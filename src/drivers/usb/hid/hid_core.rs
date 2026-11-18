/*
 * Created by Antoni Kuczyński
 * 28/07/2026
 */
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use crate::drivers::input::kbd::keyboard::{GlobalKeyboard, GLOBAL_KEYBOARD};
use crate::drivers::input::mouse::mouse::{GlobalMouse, GLOBAL_MOUSE};
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_core::usb_clear_endpoint_halt;
use crate::drivers::usb::core::usb_descriptors::{UsbInterfaceTree};
use crate::drivers::usb::core::usb_requests::{hid_request_set_boot_protocol, hid_request_set_idle_indefinitely};
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbDeviceId, UsbSetupPacket, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferStatus::{Completed, Disconnected, Error, Stalled};
use crate::drivers::usb::hid::hid_keyboard::{HidKeyboard};
use crate::drivers::usb::hid::hid_mouse::HidMouse;
use crate::{kprintln_hid_core, kprintln_hid_core_debug};

pub const HID_SUBCLASS_NONE: u8 = 0x00;
pub const HID_SUBCLASS_BOOT: u8 = 0x01;

pub const HID_PROTOCOL_NONE: u8 = 0x00;
pub const HID_PROTOCOL_KEYBOARD: u8 = 0x01;
pub const HID_PROTOCOL_MOUSE: u8 = 0x02;


pub struct RawHidReport<'a> {
    pub data: &'a [u8],
}

impl<'a> RawHidReport<'a> {
    fn new(data: &'a [u8]) -> Self {
        RawHidReport {
            data
        }
    }
}

pub trait HidDriver: Send {
    fn handle_hid_report(&mut self, report: RawHidReport);

    fn start_listening(&mut self);
}

pub struct HidCore {
    devices: BTreeMap<(UsbDeviceId, u8), Arc<IrqMutex<Box<dyn HidDriver>>>> //(system_id, endpoint_address)
}

impl HidCore {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }
}

pub fn hid_on_cleared_endpoint_halt(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (status, system_id, endpoint_address) = {
        let req = request.lock();
        let Some(setup_packet) = req.setup_packet else {
            kprintln_hid_core!(Error, "ClearFeature request has no setup packet.");
            return;
        };

        (req.status, req.target_device.system_id, setup_packet.w_index)
    };

    if status != Completed {
        kprintln_hid_core!(Error, "Cannot execute ClearFeature, the device is probably broken.");
        return;
    }

    let hid_device = HID_CORE.lock()
        .devices
        .get(&(system_id, endpoint_address as u8))
        .cloned();

    let Some(hid_device) = hid_device else { return; };
    hid_device.lock().start_listening();
}

fn hid_handle_device_disconnect(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (system_id, endpoint_address) = {
        let req = request.lock();
        let dev = req.target_device.clone();
        (dev.system_id, req.endpoint_address)
    };

    let mut hid_core = HID_CORE.lock();

    let _ = match hid_core.devices.remove(&(system_id, endpoint_address)) {
        None => {
            kprintln_hid_core_debug!("Device with system_id {} was already unregistered from HID.", system_id);
        },
        Some(x) => {
            kprintln_hid_core!(Info, "Device with system_id {} was successfully unregistered from HID.", system_id);
        }
    };
}

pub fn on_interrupt_in(request: Arc<IrqMutex<UsbTransferRequest>>) {
    //the condition temporary drops the request lock before the call
    if request.lock().status == Disconnected {
        hid_handle_device_disconnect(request);
        return;
    }

    let (device, endpoint_address) = {
        let mut req = request.lock();

        if req.status == Stalled || req.status == Error {
            let endpoint = req.endpoint_address;
            let dev = req.target_device.clone();

            if usb_clear_endpoint_halt(dev, endpoint, hid_on_cleared_endpoint_halt).is_err() {
                kprintln_hid_core!(Error, "Clearing halted endpoint failed.");
            }
            return;
        }

        if req.status != Completed {
            kprintln_hid_core!(Error, "Transfer request failed to complete - status {:?}", req.status);
            return;
        }

        let endpoint_address = req.endpoint_address;

        if let Some(data) = req.received_bytes() {
            let mut hid = HID_CORE.lock();
            let Some(hid_device) = hid.devices.get_mut(&(req.target_device.system_id, endpoint_address)) else {
                kprintln_hid_core!(Error, "Report for HID device {} that is not registered.", req.target_device.system_id);
                return;
            };

            hid_device.lock().handle_hid_report(RawHidReport::new(data));
        }

        req.status = UsbTransferStatus::Pending;
        req.bytes_transferred = 0;

        (req.target_device.clone(), endpoint_address)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(request);
    }
}

fn find_in_endpoint_address(interface_tree: &UsbInterfaceTree) -> Option<u8> {
    let mut interrupt_in_address = None;

    for ep in &interface_tree.endpoints {
        let is_in = (ep.b_endpoint_address & 0x80) != 0;
        let is_interrupt = (ep.bm_attributes & 0b11) == 3; //3 = interrupt

        if is_in && is_interrupt {
            interrupt_in_address = Some(ep.b_endpoint_address);
            break;
        }
    }

    let ep_address = match interrupt_in_address {
        Some(addr) => addr,
        None => {
            return None;
        }
    };

    Some(ep_address)
}

pub fn hid_register_usb_device(dev: Arc<UsbDevice>, usb_interface_tree: &UsbInterfaceTree) {
    let interface = &usb_interface_tree.interface;
    let endpoint_in_address = match find_in_endpoint_address(usb_interface_tree) {
        None => {
            kprintln_hid_core!(Error, "IN endpoint for HID device was not found.");
            return;
        },
        Some(x) => x
    };
    let system_id = dev.system_id;

    match (interface.b_interface_sub_class, interface.b_interface_protocol) {
        (HID_SUBCLASS_BOOT, HID_PROTOCOL_KEYBOARD) => {
            kprintln_hid_core_debug!( "Found USB keyboard with boot mode support.");

            if GLOBAL_KEYBOARD.get().is_none() {
                GlobalKeyboard::init();
            }

            let hid_kbd = HidKeyboard::new(dev.clone(), endpoint_in_address);
            HID_CORE.lock().devices.insert((system_id, endpoint_in_address), Arc::new(IrqMutex::new(Box::new(hid_kbd))));
        },
        (HID_SUBCLASS_BOOT, HID_PROTOCOL_MOUSE) => {
            kprintln_hid_core_debug!( "Found USB mouse with boot mode support.");

            if GLOBAL_MOUSE.get().is_none() {
                GlobalMouse::init();
            }

            let hid_mouse = HidMouse::new(dev.clone(), endpoint_in_address);
            HID_CORE.lock().devices.insert((system_id, endpoint_in_address), Arc::new(IrqMutex::new(Box::new(hid_mouse))));
        },
        (HID_SUBCLASS_NONE, _) => {
            kprintln_hid_core_debug!( "Usb HID interfaces without boot mode are currently not supported!");
            return;
        },
        _ => {
            kprintln_hid_core!(Error, "Invalid HID interface found.");
            return;
        }
    }

    hid_set_protocol(dev, interface.b_interface_number as u16);
}

fn hid_set_protocol(device: Arc<UsbDevice>, interface_num: u16) {
    let set_protocol_request = hid_request_set_boot_protocol(device.clone(), interface_num, hid_on_set_protocol_done);

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(set_protocol_request);
    }
    kprintln_hid_core_debug!( "Issued SET_PROTOCOL request for HID.");
}

pub fn hid_on_set_protocol_done(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (status, device, interface_number) = {
        let req = request.lock();
        let Some(setup_packet) = req.setup_packet else {
            kprintln_hid_core!(Error, "SET_PROTOCOL request has no setup packet.");
            return;
        };

        //interface number is w_index
        (req.status, req.target_device.clone(), setup_packet.w_index)
    };

    if status != Completed {
        kprintln_hid_core!(Error, "SET_PROTOCOL request failed for HID.");
        return;
    }

    let set_idle_request = hid_request_set_idle_indefinitely(device.clone(), interface_number, hid_on_set_idle_done);

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(set_idle_request);
    }
    kprintln_hid_core_debug!( "Issued SET_IDLE request for HID.");
}

pub fn hid_on_set_idle_done(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (usb_device, interface_number) = {
        let req = request.lock();
        let Some(setup_packet) = req.setup_packet else {
            kprintln_hid_core!(Error, "SET_IDLE request has no setup packet.");
            return;
        };
        (req.target_device.clone(), setup_packet.w_index as u8)
    };

    //find enpoint address in config tree
    let endpoint_address = usb_device
        .configuration_tree
        .get()
        .and_then(|tree| {
            tree.interfaces
                .iter()
                .find(|i| i.interface.b_interface_number == interface_number)
        })
        .and_then(find_in_endpoint_address);

    let Some(endpoint_address) = endpoint_address else {
        kprintln_hid_core!(Error, "No IN endpoint for interface {} after SET_IDLE.", interface_number);
        return;
    };

    let dev = HID_CORE.lock()
        .devices
        .get(&(usb_device.system_id, endpoint_address))
        .cloned();

    if let Some(driver) = dev {
        driver.lock().start_listening();
    }
}

pub static HID_CORE: IrqMutex<HidCore> = IrqMutex::new(HidCore::new());

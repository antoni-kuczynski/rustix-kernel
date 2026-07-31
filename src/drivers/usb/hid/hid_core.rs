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
use crate::drivers::usb::core::usb_descriptors::{UsbInterfaceTree};
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbSetupPacket, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::{HostToDevice};
use crate::drivers::usb::core::usb_transfers::UsbTransferStatus::{Completed};
use crate::drivers::usb::hid::hid_keyboard::{HidKeyboard};
use crate::drivers::usb::hid::hid_mouse::HidMouse;
use crate::kprintln;

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
    devices: BTreeMap<u64, Arc<IrqMutex<Box<dyn HidDriver>>>>
}

impl HidCore {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }
}

pub fn on_interrupt_in(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (device, endpoint_address) = {
        let mut req = request.lock();

        if let Some(buffer) = &req.dma_buffer {
            let data = unsafe { core::slice::from_raw_parts(buffer.virt.as_ptr::<u8>(), req.bytes_transferred) };

            let mut hid = HID_CORE.lock();
            let mut dev = hid.devices
                .get_mut(&req.target_device.system_id)
                .expect("No HID device present inside hid core to handle data packet.")
                .lock();

            dev.handle_hid_report(RawHidReport::new(data));
        }

        req.status = UsbTransferStatus::Pending;
        req.bytes_transferred = 0;

        (req.target_device.clone(), req.endpoint_address)
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
    let a = dev.configuration_tree.get().unwrap();
    let interface = &usb_interface_tree.interface;
    let endpoint_in_address = find_in_endpoint_address(usb_interface_tree);
    let system_id = dev.system_id;

    if endpoint_in_address.is_none() {
        kprintln!(Error, "IN endpoint for HID device was not found.");
        return;
    }


    match (interface.b_interface_sub_class, interface.b_interface_protocol) {
        (HID_SUBCLASS_BOOT, HID_PROTOCOL_KEYBOARD) => {
            kprintln!("Found USB keyboard with boot mode support.");

            if GLOBAL_KEYBOARD.get().is_none() {
                GlobalKeyboard::init();
            }

            let hid_kbd = HidKeyboard::new(dev.clone(), endpoint_in_address.unwrap());
            HID_CORE.lock().devices.insert(system_id, Arc::new(IrqMutex::new(Box::new(hid_kbd))));
        },
        (HID_SUBCLASS_BOOT, HID_PROTOCOL_MOUSE) => {
            kprintln!("Found USB mouse with boot mode support.");

            if GLOBAL_MOUSE.get().is_none() {
                GlobalMouse::init();
            }

            let hid_mouse = HidMouse::new(dev.clone(), endpoint_in_address.unwrap());
            HID_CORE.lock().devices.insert(system_id, Arc::new(IrqMutex::new(Box::new(hid_mouse))));
        },
        (HID_SUBCLASS_NONE, _) => {
            kprintln!("Usb HID interfaces without boot mode are currently not supported!");
            return;
        },
        _ => {
            kprintln!(Error, "Invalid HID interface found.");
            return;
        }
    }

    hid_set_protocol(dev, interface.b_interface_number as u16);
}

fn hid_set_protocol(device: Arc<UsbDevice>, interface_num: u16) {
    let set_protocol_request = Arc::new(IrqMutex::new(UsbTransferRequest {
        target_device: device.clone(),
        endpoint_address: 0,
        transfer_direction: HostToDevice,
        transfer_type: UsbTransferType::Control,

        setup_packet: Some(UsbSetupPacket {
            bm_request_type: 0x21,
            b_request: 0x0B,       //SET_PROTOCOL
            w_value: 0x0000,       //0 = Boot Protocol
            w_index: interface_num,
            w_length: 0,
        }),

        dma_buffer: None,
        data_buffer_length: 0,
        status: UsbTransferStatus::Pending,
        bytes_transferred: 0,
        completion_callback: Some(hid_on_set_protocol_done),
    }));

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(set_protocol_request);
    }
    kprintln!(Debug, "Issued SET_PROTOCOL request for HID.");
}

pub fn hid_on_set_protocol_done(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (status, device, interface_number) = {
        let req = request.lock();
        let setup_packet = req.setup_packet.unwrap();

        //interface number is w_index
        (req.status, req.target_device.clone(), setup_packet.w_index)
    };

    if status != Completed {
        kprintln!(Error, "SET_PROTOCOL request failed for HID.");
        return;
    }

    let set_idle_request = Arc::new(IrqMutex::new(UsbTransferRequest {
        target_device: device.clone(),
        endpoint_address: 0,
        transfer_direction: HostToDevice,
        transfer_type: UsbTransferType::Control,

        setup_packet: Some(UsbSetupPacket {
            bm_request_type: 0x21,
            b_request: 0x0A,       //SET_IDLE
            w_value: 0x0000,       //high byte (Duration) = 0, low byte (Report id) = 0
            w_index: interface_number,
            w_length: 0,
        }),

        dma_buffer: None,
        data_buffer_length: 0,
        status: UsbTransferStatus::Pending,
        bytes_transferred: 0,
        completion_callback: Some(hid_on_set_idle_done),
    }));

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(set_idle_request);
    }
    kprintln!(Debug, "Issued SET_IDLE request for HID.");
}

pub fn hid_on_set_idle_done(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let usb_device = {
        let req = request.lock();
        req.target_device.clone()
    };

    let mut hid_core = HID_CORE.lock();
    let mut dev = hid_core.devices.get_mut(&usb_device.system_id)
        .unwrap()
        .lock();
    dev.start_listening();
}

pub static HID_CORE: IrqMutex<HidCore> = IrqMutex::new(HidCore::new());

/*
 * Created by Antoni Kuczyński
 * 23/07/2026
 */
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;
use crate::drivers::usb::core::usb_descriptors::*;
use crate::drivers::usb::core::irq_mutex::*;
use crate::drivers::usb::core::usb_hub::{usb_hub_register_port_device, usb_issue_hub_descriptor};
use crate::drivers::usb::core::usb_requests::{
    usb_request_clear_endpoint_halt, usb_request_clear_tt_buffer, usb_request_get_configuration_descriptor,
    usb_request_get_device_descriptor, usb_request_set_configuration,
};
use crate::drivers::usb::core::usb_transfers::*;
use crate::drivers::usb::core::UsbSpeed;
use crate::drivers::usb::hid::hid_core::hid_register_usb_device;
use crate::{kprintln_usb_core, kprintln_usb_core_debug};

const USB_CLASS_HID: u8 = 0x03;
const USB_CLASS_HUB: u8 = 0x09;
const CONFIGURATION_DESCRIPTOR_TOTAL_LENGTH_PREFIX: u16 = 8;
pub const USB_ENDPOINT_TRANSFER_TYPE_MASK: u8 = 0x03;
pub const USB_ENDPOINT_TRANSFER_TYPE_CONTROL: u8 = 0;
pub const USB_ENDPOINT_TRANSFER_TYPE_BULK: u8 = 2;
pub const USB_ENDPOINT_TRANSFER_TYPE_INTERRUPT: u8 = 3;
pub const USB_ENDPOINT_DIRECTION_IN: u8 = 0x80;
const USB_HUB_PROTOCOL_MULTI_TT: u8 = 2;

pub struct UsbCore {
    devices: Vec<Arc<UsbDevice>>,
    next_system_id: u64
}

impl UsbCore {
    const fn new() -> Self {
        UsbCore {
            devices: Vec::new(),
            next_system_id: 1
        }
    }

    fn new_system_id(&mut self) -> UsbDeviceId {
        let id = self.next_system_id;
        self.next_system_id += 1;
        UsbDeviceId(id)
    }

    fn add_device(
        &mut self,
        hardware_id: u8,
        host_controller: Weak<dyn UsbHostController>,
        port_on_parent: u8,
        speed: UsbSpeed,
        parent_hub: Option<Arc<UsbDevice>>
    ) -> (Arc<UsbDevice>, Option<Arc<IrqMutex<UsbTransferRequest>>>) {
        let device = Arc::new(UsbDevice {
            system_id: self.new_system_id(),
            hardware_id,
            host_controller,
            configuration_tree: Once::new(),
            device_descriptor: Once::new(),
            port_on_parent,
            speed,
            parent_hub
        });

        self.devices.push(device.clone());

        let request = usb_request_get_device_descriptor(device.clone(), on_device_descriptor_received);

        (device, request)
    }
}

pub fn usb_clear_endpoint_halt(
    device: Arc<UsbDevice>,
    endpoint_address: u8,
    callback: UsbTransferCallback,
) -> Result<(), &'static str> {
    let controller = device
        .host_controller
        .upgrade()
        .ok_or("Host controller is gone.")?;

    //high speed hub's tt buffers also have to be cleared for low/full speed devices in
    //control/bulk endpoints
    if let Some(hub) = &device.parent_hub {
        let endpoint_transfer_type = device
            .configuration_tree
            .get()
            .and_then(|tree| tree.find_endpoint(endpoint_address))
            .map(|endpoint| endpoint.bm_attributes & USB_ENDPOINT_TRANSFER_TYPE_MASK);

        let is_control_or_bulk = matches!(
            endpoint_transfer_type,
            Some(USB_ENDPOINT_TRANSFER_TYPE_CONTROL) | Some(USB_ENDPOINT_TRANSFER_TYPE_BULK)
        );
        let is_behind_tt = matches!(device.speed, UsbSpeed::Low | UsbSpeed::Full)
            && hub.speed == UsbSpeed::High;

        if let Some(endpoint_transfer_type) = endpoint_transfer_type.filter(|_| is_behind_tt && is_control_or_bulk) {
            let is_multi_tt = hub
                .device_descriptor
                .get()
                .is_some_and(|descriptor| descriptor.b_device_protocol() == USB_HUB_PROTOCOL_MULTI_TT);
            let tt_port_number = if is_multi_tt { device.port_on_parent as u16 } else { 1 };

            match controller.device_address(device.hardware_id) {
                Some(device_address) => {
                    let clear_tt_request = usb_request_clear_tt_buffer(
                        hub.clone(),
                        tt_port_number,
                        device_address,
                        endpoint_address,
                        endpoint_transfer_type,
                        on_clear_tt_buffer_complete,
                    );
                    match clear_tt_request {
                        Some(clear_tt_request) => {
                            if let Err(error) = controller.submit_request(clear_tt_request) {
                                kprintln_usb_core!(Error, "Failed to submit CLEAR_TT_BUFFER: {}", error);
                            }
                        }
                        None => {
                            kprintln_usb_core!(Error, "Failed to allocate CLEAR_TT_BUFFER request.");
                        }
                    }
                }
                None => {
                    kprintln_usb_core!(Error, "No USB address for device {}, skipping CLEAR_TT_BUFFER.", device.system_id);
                }
            }
        }
    }

    let request = usb_request_clear_endpoint_halt(device, endpoint_address, callback)
        .ok_or("Failed to allocate ClearFeature(ENDPOINT_HALT) request.")?;
    controller.submit_request(request)
}

fn on_clear_tt_buffer_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let status = request.lock().status;
    if status != UsbTransferStatus::Completed {
        kprintln_usb_core!(Error, "CLEAR_TT_BUFFER failed with status {:?}.", status);
    }
}

pub fn usb_find_device(host_controller: &Weak<dyn UsbHostController>, hardware_id: u8) -> Option<Arc<UsbDevice>> {
    let controller_address = host_controller.as_ptr() as *const ();

    USB_CORE.lock()
        .devices
        .iter()
        .rev()
        .find(|device| {
            device.hardware_id == hardware_id && device.host_controller.as_ptr() as *const () == controller_address
        })
        .cloned()
}

pub fn usb_register_device(
    hardware_id: u8, host_controller: Weak<dyn UsbHostController>, 
    parent_port: u8, speed: UsbSpeed, parent_hub: Option<Arc<UsbDevice>>) {
    let (device, request) = {
        let mut usb_core = USB_CORE.lock();
        usb_core.add_device(hardware_id, host_controller, parent_port, speed, parent_hub)
    };

    //registered before any driver binds them so disconnect of the port can always find it
    if let Some(parent_hub) = &device.parent_hub {
        usb_hub_register_port_device(parent_hub, device.port_on_parent as u16, device.clone());
    }

    let Some(request) = request else {
        kprintln_usb_core!(Error, "Failed to allocate GET_DESCRIPTOR request for device {}.", device.system_id);
        return;
    };

    let Some(controller) = device.host_controller.upgrade() else {
        kprintln_usb_core!(Warn, "Host controller went away before the device could be registered.");
        return;
    };

    if let Err(error) = controller.submit_request(request) {
        kprintln_usb_core!(Error, "Failed to request the device descriptor: {}", error);
        return;
    }
}


fn on_device_descriptor_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln_usb_core!(Error, "Failed to complete GET_DESCRIPTOR request for device descriptor.");
        return;
    }

    let (dev, next_request) = {
        let req = request.lock();
        kprintln_usb_core_debug!(
            "Got device descriptor: ({:?}, {} bytes).",
            req.status,
            req.bytes_transferred
        );

        let Some(desc) = req.read_received::<UsbDeviceDescriptor>() else {
            kprintln_usb_core!(Error, "No full device descriptor received ({} bytes).", req.bytes_transferred);
            return;
        };

        let dev = req.target_device.clone();
        dev.device_descriptor.call_once(|| desc);

        let Some(config_req) = usb_request_get_configuration_descriptor(
            dev.clone(), CONFIGURATION_DESCRIPTOR_TOTAL_LENGTH_PREFIX, on_config_header_received
        ) else {
            kprintln_usb_core!(Error, "Failed to allocate configuration descriptor header request.");
            return;
        };
        (dev, config_req)
    };

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(next_request);
    }
}

pub fn on_config_header_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln_usb_core!(Error, "Failed to complete GET_DESCRIPTOR request for configuration descriptor header.");
        return;
    }

    let (device, full_config_request) = {
        let req = request.lock();
        let dev = req.target_device.clone();

        let Some(header) = req.received_bytes().and_then(|bytes| bytes.get(2..4)) else {
            kprintln_usb_core!(Error, "Configuration descriptor header too short ({} bytes).", req.bytes_transferred);
            return;
        };
        let w_total_length = u16::from_le_bytes([header[0], header[1]]);

        kprintln_usb_core_debug!( "Detected Configuration Descriptor size: {} bytes.", w_total_length);

        //now, it's time for the full configuration descriptor
        let Some(full_req) = usb_request_get_configuration_descriptor(dev.clone(), w_total_length, on_full_config_received) else {
            kprintln_usb_core!(Error, "Failed to allocate full configuration descriptor request ({} bytes).", w_total_length);
            return;
        };

        (dev, full_req)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(full_config_request);
    }
}

pub fn on_full_config_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln_usb_core!(Error, "Failed to complete GET_DESCRIPTOR request for configuration descriptor.");
        return;
    }

    let (device, full_config_request) = {
        let req = request.lock();
        let dev = req.target_device.clone();

        let Some(received_bytes) = req.received_bytes() else {
            kprintln_usb_core!(Error, "No dma buffer found for full configuration descriptor.");
            return;
        };

        let Some(config_tree) = UsbConfigurationTree::parse(received_bytes) else {
            kprintln_usb_core!(Error, "Failed to parse full USB configuration tree of device {}.", dev.system_id);
            return;
        };

        kprintln_usb_core_debug!( "Successfully fully enumerated device ID {}!", dev.system_id);

        let config_value = config_tree.configuration.b_configuration_value;

        dev.configuration_tree.call_once(|| config_tree);

        let Some(full_req) = usb_request_set_configuration(dev.clone(), config_value, on_set_configuration_complete) else {
            kprintln_usb_core!(Error, "Failed to allocate SET_CONFIGURATION request.");
            return;
        };
        (dev, full_req)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(full_config_request);
    }
}

pub fn on_set_configuration_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln_usb_core!(Error, "Failed to complete SET_CONFIGURATION request.");
        return;
    }

    kprintln_usb_core_debug!( "Completed SET_CONFIGURATION request.");

    let dev = {
        let req = request.lock();
        let dev = req.target_device.clone();
        dev
    };

    let (Some(device_descriptor), Some(configuration_tree)) =
        (dev.device_descriptor.get(), dev.configuration_tree.get()) else {
        kprintln_usb_core!(Error, "Device {} configured without its descriptors.", dev.system_id);
        return;
    };

    if device_descriptor.b_device_class() == USB_CLASS_HUB {
        usb_issue_hub_descriptor(dev);
        return;
    }

    for interface_tree in &configuration_tree.interfaces {
        if interface_tree.interface.b_interface_class == USB_CLASS_HID {
            hid_register_usb_device(dev.clone(), interface_tree);
        }
    }
}

pub static USB_CORE: IrqMutex<UsbCore> = IrqMutex::new(UsbCore::new());

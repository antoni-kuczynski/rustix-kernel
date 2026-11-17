/*
 * Created by Antoni Kuczyński
 * 23/07/2026
 */
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;
use crate::drivers::usb::core::usb_descriptors::*;
use crate::drivers::usb::core::irq_mutex::*;
use crate::drivers::usb::core::usb_hub::{usb_issue_hub_descriptor};
use crate::drivers::usb::core::usb_requests::{
    usb_request_clear_endpoint_halt, usb_request_get_configuration_descriptor, usb_request_get_device_descriptor,
    usb_request_set_configuration,
};
use crate::drivers::usb::core::usb_transfers::*;
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::{DeviceToHost, HostToDevice};
use crate::drivers::usb::core::UsbSpeed;
use crate::drivers::usb::hid::hid_core::hid_register_usb_device;
use crate::kprintln;
use crate::memory::dma::dma_alloc_zeroed;


const USB_CLASS_HID: u8 = 0x03;
const USB_CLASS_HUB: u8 = 0x09;
const CONFIGURATION_DESCRIPTOR_TOTAL_LENGTH_PREFIX: u16 = 8;

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
        parent_hub: Option<Arc<IrqMutex<UsbDevice>>>
    ) -> (Arc<UsbDevice>, Arc<IrqMutex<UsbTransferRequest>>) {
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

    let request = usb_request_clear_endpoint_halt(device, endpoint_address, callback);
    controller.submit_request(request)
}

/// Registers a newly addressed device and asks it for its device descriptor.
pub fn usb_register_device(
    hardware_id: u8, host_controller: Weak<dyn UsbHostController>, 
    parent_port: u8, speed: UsbSpeed, parent_hub: Option<Arc<IrqMutex<UsbDevice>>>) {
    let (device, request) = {
        let mut usb_core = USB_CORE.lock();
        usb_core.add_device(hardware_id, host_controller, parent_port, speed, parent_hub)
    };

    let Some(controller) = device.host_controller.upgrade() else {
        kprintln!(Warn, "[USB CORE] Host controller went away before the device could be registered.");
        return;
    };

    if let Err(error) = controller.submit_request(request) {
        kprintln!(Error, "[USB CORE] Failed to request the device descriptor: {}", error);
        return;
    }
}


fn on_device_descriptor_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln!(Error, "[USB CORE] Failed to complete GET_DESCRIPTOR request for device descriptor.");
        return;
    }

    let (dev, next_request) = {
        let req = request.lock();
        kprintln!(
            Debug,
            "[USB CORE] Got device descriptor: ({:?}, {} bytes).",
            req.status,
            req.bytes_transferred
        );

        let desc = unsafe { &*req.dma_buffer
            .as_ref()
            .expect("No dma buffer found for configuration descriptor header.")
            .virt.as_ptr::<UsbDeviceDescriptor>()
        };

        let dev = req.target_device.clone();
        dev.device_descriptor.call_once(|| *desc);

        let config_req = usb_request_get_configuration_descriptor(
            dev.clone(), CONFIGURATION_DESCRIPTOR_TOTAL_LENGTH_PREFIX, on_config_header_received
        );
        (dev, config_req)
    };

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(next_request);
    }
}

pub fn on_config_header_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln!(Error, "[USB CORE] Failed to complete GET_DESCRIPTOR request for configuration descriptor header.");
        return;
    }

    let (device, full_config_request) = {
        let req = request.lock();
        let dev = req.target_device.clone();

        let dma_ptr = req.dma_buffer
            .as_ref()
            .expect("No dma buffer found for configuration descriptor header.")
            .virt
            .as_ptr::<u64>();

        let first_8_bytes = unsafe { core::ptr::read(dma_ptr) };

        let bytes = first_8_bytes.to_le_bytes();
        let w_total_length = u16::from_le_bytes([bytes[2], bytes[3]]);

        kprintln!(Debug, "[USB CORE] Detected Configuration Descriptor size: {} bytes.", w_total_length);

        //now, it's time for the full configuration descriptor
        let full_req = usb_request_get_configuration_descriptor(dev.clone(), w_total_length, on_full_config_received);

        (dev, full_req)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(full_config_request);
    }
}

pub fn on_full_config_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln!(Error, "[USB CORE] Failed to complete GET_DESCRIPTOR request for configuration descriptor.");
        return;
    }

    let (device, full_config_request) = {
        let req = request.lock();
        let dev = req.target_device.clone();

        let ptr = req.dma_buffer
            .as_ref()
            .expect("No dma buffer found for full configuration descriptor.")
            .virt.as_ptr::<u8>();

        let config_tree = unsafe {
            UsbConfigurationTree::from_ptr(ptr, req.data_buffer_length)
        }.expect("Failed to parse full USB configuration tree!");

        kprintln!(Debug, "[USB CORE] Successfully fully enumerated device ID {}!", dev.system_id);

        let config_value = config_tree.configuration.b_configuration_value;

        dev.configuration_tree.call_once(|| config_tree);

        let full_req = usb_request_set_configuration(dev.clone(), config_value, on_set_configuration_complete);
        (dev, full_req)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(full_config_request);
    }
}

pub fn on_set_configuration_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status != UsbTransferStatus::Completed {
        kprintln!(Error, "[USB CORE] Failed to complete SET_CONFIGURATION request.");
        return;
    }

    kprintln!(Debug, "[USB CORE] Completed SET_CONFIGURATION request.");

    let dev = {
        let req = request.lock();
        let dev = req.target_device.clone();
        dev
    };

    if dev.device_descriptor.get().unwrap().b_device_class() == USB_CLASS_HUB {
        usb_issue_hub_descriptor(dev);
        return;
    }
    
    let a = dev.configuration_tree.get().unwrap();
    for interface_tree in  &a.interfaces {
        if interface_tree.interface.b_interface_class == USB_CLASS_HID {
            hid_register_usb_device(dev.clone(), interface_tree);
        }
    }
}

pub static USB_CORE: IrqMutex<UsbCore> = IrqMutex::new(UsbCore::new());

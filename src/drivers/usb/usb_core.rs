/*
 * Created by Antoni Kuczyński
 * 23/07/2026
 */
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;
use crate::drivers::usb::descriptors::{UsbConfigurationTree, UsbDeviceDescriptor};
use crate::drivers::usb::irq_mutex::IrqMutex;
use crate::drivers::usb::usb_transfers::{UsbDevice, UsbHostController, UsbSetupPacket, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::usb_transfers::UsbTransferDirection::DeviceToHost;
use crate::kprintln;
use crate::memory::dma::dma_alloc_zeroed;

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

    fn new_system_id(&mut self) -> u64 {
        let id = self.next_system_id;
        self.next_system_id += 1;
        id
    }

    fn add_device(
        &mut self,
        hardware_id: u8,
        host_controller: Weak<dyn UsbHostController>,
    ) -> (Arc<UsbDevice>, Arc<IrqMutex<UsbTransferRequest>>) {
        let device = Arc::new(UsbDevice {
            system_id: self.new_system_id(),
            hardware_id,
            host_controller,
            configuration_tree: Once::new(),
            device_descriptor: Once::new()
        });

        self.devices.push(device.clone());

        let dma_buffer = dma_alloc_zeroed(size_of::<UsbDeviceDescriptor>(), 1)
            .expect("Failed to allocate memory for GET_DESCRIPTOR request!");

        let setup = UsbSetupPacket::new(
            0x80,
            0x06,
            0x0100,
            0x0000,
            18
        );

        let request = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: device.clone(),
            endpoint_address: 0,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Control,
            setup_packet: Some(setup),
            dma_buffer,
            data_buffer_length: 18,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(on_device_descriptor_received),
        }));

        (device, request)
    }
}

/// Registers a newly addressed device and asks it for its device descriptor.
pub fn usb_register_device(hardware_id: u8, host_controller: Weak<dyn UsbHostController>) {
    let (device, request) = {
        let mut usb_core = USB_CORE.lock();
        usb_core.add_device(hardware_id, host_controller)
    };

    let Some(controller) = device.host_controller.upgrade() else {
        kprintln!(Warn, "Host controller went away before the device could be registered.");
        return;
    };

    kprintln!("Prepared request.");
    if let Err(error) = controller.submit_request(request) {
        kprintln!(Error, "Failed to request the device descriptor: {}", error);
        return;
    }
    kprintln!("Issued request.");
}


fn on_device_descriptor_received(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (dev, next_request) = {
        let req = request.lock();
        kprintln!(
            Debug,
            "Got device descriptor in layer2 ({:?}, {} bytes).",
            req.status,
            req.bytes_transferred
        );
        
        let desc = unsafe { &*req.dma_buffer.virt.as_ptr::<UsbDeviceDescriptor>() };
        let dev = req.target_device.clone();
        dev.device_descriptor.call_once(|| *desc);

        let next_dma = dma_alloc_zeroed(8, 1).unwrap();
        let setup = UsbSetupPacket::new(
            0x80,
            0x06,
            0x0200,
            0x0000,
            8
        );

        let config_req = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: dev.clone(),
            endpoint_address: 0,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Control,
            setup_packet: Some(setup),
            dma_buffer: next_dma,
            data_buffer_length: 8, //8bytes to get the total length. we dont need the 9th byte now
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(on_config_header_received),
        }));
        (dev, config_req)
    };

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(next_request);
    }
}

pub fn on_config_header_received(request_arc: Arc<IrqMutex<UsbTransferRequest>>) {
    let (device, full_config_request) = {
        let req = request_arc.lock();
        let dev = req.target_device.clone();

        let dma_ptr = req.dma_buffer.virt.as_ptr::<u64>();
        let first_8_bytes = unsafe { core::ptr::read(dma_ptr) };

        let bytes = first_8_bytes.to_le_bytes();
        let w_total_length = u16::from_le_bytes([bytes[2], bytes[3]]);

        kprintln!(Debug, "[USB CORE] Detected Configuration Descriptor size: {} bytes", w_total_length);

        //now, it's time for the full configuration descriptor
        let next_dma = dma_alloc_zeroed(w_total_length as usize, 1).unwrap();
        let setup = UsbSetupPacket::new(0x80, 0x06, 0x0200, 0x0000, w_total_length);

        let full_req = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: dev.clone(),
            endpoint_address: 0,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Control,
            setup_packet: Some(setup),
            dma_buffer: next_dma,
            data_buffer_length: w_total_length as usize,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(on_full_config_received),
        }));

        (dev, full_req)
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(full_config_request);
    }
}

pub fn on_full_config_received(request_arc: Arc<IrqMutex<UsbTransferRequest>>) {
    let req = request_arc.lock();
    let dev = req.target_device.clone();

    let ptr = req.dma_buffer.virt.as_ptr::<u8>();

    let config_tree = unsafe {
        UsbConfigurationTree::from_ptr(ptr, req.data_buffer_length)
    }.expect("Failed to parse full USB configuration tree!");

    kprintln!(Info, "[USB CORE] Successfully fully enumerated device ID {}!", dev.system_id);
    kprintln!(Debug, "{}", config_tree);

    dev.configuration_tree.call_once(|| config_tree);



}

pub static USB_CORE: IrqMutex<UsbCore> = IrqMutex::new(UsbCore::new());

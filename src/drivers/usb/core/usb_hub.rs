/*
 * Created by Antoni Kuczyński
 * 04/08/2026
 */
use alloc::sync::Arc;
use core::ptr::slice_from_raw_parts;
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbSetupPacket, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::{DeviceToHost, HostToDevice};
use crate::kprintln;
use crate::memory::dma::dma_alloc_zeroed;

pub fn usb_issue_hub_descriptor(dev: Arc<UsbDevice>) {
    let is_superspeed = dev.speed.is_superspeed_or_higher();
    let descriptor_type: u16 = if is_superspeed { 0x2A } else { 0x29 };
    let w_value = (descriptor_type << 8) | 0x00;
    let w_length: u16 = 128;

    let (dev, full_req) = {
        let get_hub_desc_setup = UsbSetupPacket::new(
            0xA0,
            0x06,
            w_value,
            0x0000,
            w_length
        );

        let dma_buf = dma_alloc_zeroed(w_length as usize, 64)
            .expect("DMA allocation for hub descriptor failed");

        let hub_desc_request = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: dev.clone(),
            endpoint_address: 0,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Control,
            setup_packet: Some(get_hub_desc_setup),
            dma_buffer: Some(dma_buf),
            data_buffer_length: w_length as usize,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(on_get_hub_descriptor_complete),
        }));

        (dev, hub_desc_request)
    };

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(full_req);
    }
}

fn on_get_hub_descriptor_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    //TODO
    // let (dma, desc_type, dev) = {
    //     let req = request.lock();
    //     let desc_type = req.setup_packet.unwrap().w_value >> 8;
    //     let dma = req.dma_buffer.as_ref().unwrap();
    //
    //     (dma, desc_type, req.target_device.clone())
    // };
    //
    // let num_ports = if desc_type == 0x2A {
    //     let desc = unsafe { dma.as_mut::<Usb3HubDescriptor>() };
    //     desc.b_nbr_ports
    // } else {
    //     let slice = unsafe { &*slice_from_raw_parts(
    //         dma.virt.as_mut_ptr::<u8>(), dma.layout.size()
    //     ) };
    //
    //     let desc = Usb2HubDescriptor::parse_from_dma(slice)
    //         .expect("Failed to parse usb2 hub descriptor.");
    //     desc.b_nbr_ports
    // };
    //
    // for port_number in 1..num_ports {
    //     let set_port_power_setup = UsbSetupPacket::new(
    //         0x23,
    //         0x03,
    //         8,
    //         port_number as u16,
    //         0
    //     );
    //
    //     let port_power_request = Arc::new(IrqMutex::new(UsbTransferRequest {
    //         target_device: dev.clone(),
    //         endpoint_address: 0,
    //         transfer_direction: HostToDevice,
    //         transfer_type: UsbTransferType::Control,
    //         setup_packet: Some(set_port_power_setup),
    //         dma_buffer: None, // Brak bufora (wLength = 0)
    //         data_buffer_length: 0,
    //         status: UsbTransferStatus::Pending,
    //         bytes_transferred: 0,
    //         completion_callback: Some(on_set_port_power_complete),
    //     }));
    // }

}


fn on_set_port_power_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {

}

#[derive(Debug)]
pub struct Usb2HubDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_nbr_ports: u8,
    pub w_hub_characteristics: u16,
    pub b_pwr_on_to_pwr_good: u8,
    pub b_hub_contr_current: u8,
    device_removable_mask: [u8; 4],
}

impl Usb2HubDescriptor {
    pub fn parse_from_dma(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < 7 {
            return None;
        }

        let b_length = buffer[0];
        let b_descriptor_type = buffer[1];
        let b_nbr_ports = buffer[2];
        let w_hub_characteristics = u16::from_le_bytes([buffer[3], buffer[4]]);
        let b_pwr_on_to_pwr_good = buffer[5];
        let b_hub_contr_current = buffer[6];

        let mask_bytes_len = ((b_nbr_ports / 8) + 1) as usize;
        let mask_start_offset = 7;

        if buffer.len() < mask_start_offset + (mask_bytes_len * 2) {
            return None;
        }

        let mut device_removable_mask = [0u8; 4];

        let copy_len = mask_bytes_len.min(4);
        device_removable_mask[..copy_len].copy_from_slice(
            &buffer[mask_start_offset .. mask_start_offset + copy_len]
        );

        Some(Self {
            b_length,
            b_descriptor_type,
            b_nbr_ports,
            w_hub_characteristics,
            b_pwr_on_to_pwr_good,
            b_hub_contr_current,
            device_removable_mask,
        })
    }

    pub fn is_port_removable(&self, port_number: u8) -> bool {
        if port_number == 0 || port_number > self.b_nbr_ports {
            return true;
        }

        let byte_index = (port_number / 8) as usize;
        let bit_index = port_number % 8;

        if byte_index >= self.device_removable_mask.len() {
            return true;
        }

        let bit_value = self.device_removable_mask[byte_index] & (1 << bit_index);
        bit_value == 0
    }
}

#[repr(C, packed)]
pub struct Usb3HubDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_nbr_ports: u8,
    pub w_hub_characteristics: u16,
    pub b_pwr_on_to_pwr_good: u8,
    pub b_hub_contr_current: u8,
    pub b_hub_hdr_dec_lat: u8,
    pub w_hub_delay: u16,
    pub device_removable: u16,
}
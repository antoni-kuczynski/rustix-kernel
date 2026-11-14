/*
 * Created by Antoni Kuczyński
 * 04/08/2026
 */
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::cmp::min;
use core::ptr::slice_from_raw_parts;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_core::on_set_configuration_complete;
use crate::drivers::usb::core::usb_descriptors::UsbConfigurationTree;
use crate::drivers::usb::core::usb_requests::{usb_request_get_hub_descriptor, usb_request_set_hub_port_feature};
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbSetupPacket, UsbTransferDirection, UsbTransferRecipient, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::{DeviceToHost, HostToDevice};
use crate::drivers::usb::{UsbBRequest, UsbBmRequestType, WValue};
use crate::drivers::usb::core::usb_transfers::UsbTransferStatus::{Completed, Error};
use crate::drivers::usb::hid::hid_core::{HidCore, HidDriver};
use crate::kprintln;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};

//the device removable mask only covers 31 ports
const HUB_MAX_PORTS: u16 = 31;

pub struct UsbHub {
    device: Arc<UsbDevice>,
    system_id: u64,
    number_of_ports: u16,
    current_port_index: u16,
    is_super_speed: bool,
    hub_descriptor: DmaAlloc,
}

impl UsbHub {
    /// Builds SET_FEATURE(PORT_POWER) for the next unpowered port and advances
    /// `current_port_index`. Returns `None` once every port has been covered.
    fn next_port_power_request(&mut self) -> Option<Arc<IrqMutex<UsbTransferRequest>>> {
        if self.current_port_index > self.number_of_ports {
            return None;
        }

        let port_number = self.current_port_index;
        self.current_port_index += 1;

        Some(usb_request_set_hub_port_feature(
            self.device.clone(), port_number, WValue::PORT_POWER, on_set_port_power_complete
        ))
    }
}

pub struct UsbHubCore {
    /// Keyed by `UsbDevice::system_id` of the hub, so completion callbacks can find the hub
    /// straight from `request.target_device`.
    devices: BTreeMap<u64, Arc<IrqMutex<Box<UsbHub>>>>
}

impl UsbHubCore {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    fn find_by_device(&self, device_system_id: u64) -> Option<Arc<IrqMutex<Box<UsbHub>>>> {
        self.devices.get(&device_system_id).cloned()
    }
}

/// Returns `None` if the descriptor is shorter than it claims or than the hub sent.
fn usb_hub_get_number_of_ports(is_super_speed_hub: bool, dma: &DmaAlloc, bytes_transferred: usize) -> Option<u16> {
    //the buffer is zeroed, so reading past what the hub actually sent would look like valid data
    let received_length = min(bytes_transferred, dma.layout.size());
    let buffer = unsafe {
        core::slice::from_raw_parts(dma.virt.as_ptr::<u8>(), received_length)
    };

    let number_of_ports = if !is_super_speed_hub {
        Usb2HubDescriptor::parse_from_dma(buffer)?.b_nbr_ports
    } else {
        if buffer.len() < size_of::<Usb3HubDescriptor>() {
            return None;
        }

        let hub_descriptor = unsafe { &*(buffer.as_ptr() as *const Usb3HubDescriptor) };
        hub_descriptor.b_nbr_ports
    } as u16;

    let b_length = buffer[0] as usize;
    if b_length > buffer.len() {
        return None;
    }

    //cap to the max size of 4 byte bitmask
    Some(min(number_of_ports, HUB_MAX_PORTS))
}

pub fn usb_issue_hub_descriptor(dev: Arc<UsbDevice>) {
    let is_superspeed = dev.speed.is_superspeed_or_higher();
    let full_req = usb_request_get_hub_descriptor(dev.clone(), is_superspeed, on_get_hub_descriptor_complete);

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(full_req);
    }
}

fn on_get_hub_descriptor_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (hub_descriptor, is_super_speed, amount_of_ports, device) = {
        let mut req = request.lock();
        let dev = req.target_device.clone();

        if req.status != Completed {
            kprintln!(Error, "[xHCI] Get hub descriptor request failed with status {:?}.", req.status);
            return;
        }

        let speed_type = req
            .setup_packet
            .unwrap()
            .w_value >> 8;

        let is_super_speed_hub = (speed_type & 0xFF) as u8 == WValue::DESC_SUPERSPEED_HUB;
        let descriptor_dma = req
            .dma_buffer
            .take()
            .unwrap();

        let Some(amount_of_ports) = usb_hub_get_number_of_ports(
            is_super_speed_hub, &descriptor_dma, req.bytes_transferred
        ) else {
            kprintln!(Error, "[USB HUB] Malformed hub descriptor ({} bytes received).", req.bytes_transferred);
            return;
        };

        (descriptor_dma, is_super_speed_hub, amount_of_ports, dev)
    };

    let system_id = USB_HUB_SYSTEM_ID.fetch_add(1, Ordering::AcqRel);
    let mut hub_device = UsbHub {
        device: device.clone(),
        system_id,
        number_of_ports: amount_of_ports,
        current_port_index: 1,
        is_super_speed,
        hub_descriptor,
    };

    //now, set the PORT_POWER for all ports of the hub, one at a time
    let port_power_req = hub_device.next_port_power_request();

    USB_HUB_CORE.lock().devices.insert(device.system_id, Arc::new(IrqMutex::new(Box::new(hub_device))));

    let Some(port_power_req) = port_power_req else {
        kprintln!(Error, "[USB HUB] Hub reported 0 ports, nothing to power.");
        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(port_power_req);
    }
}

//callback is called for each port completion
fn on_set_port_power_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let req = {
        let req1 = request.lock();
        if req1.status != Completed {
            kprintln!(Error, "[xHCI] Setting port power on hub failed with status {:?}.", req1.status);
            return;
        }
        req1
    };
    let device = req.target_device.clone();

    let Some(hub) = USB_HUB_CORE.lock().find_by_device(device.system_id) else {
        kprintln!(Error, "[USB HUB] PORT_POWER completed for a device that is not a registered hub.");
        return;
    };

    let port_power_req = hub.lock().next_port_power_request();

    let Some(port_power_req) = port_power_req else {
        on_all_ports_powered();
        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(port_power_req);
    }
}

fn on_all_ports_powered() {

}

#[derive(Debug)]
#[repr(C, packed)]
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

pub static USB_HUB_CORE: IrqMutex<UsbHubCore> = IrqMutex::new(UsbHubCore::new());
static USB_HUB_SYSTEM_ID: AtomicU64 = AtomicU64::new(1);

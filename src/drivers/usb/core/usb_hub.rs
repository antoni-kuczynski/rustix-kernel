/*
 * Created by Antoni Kuczyński
 * 04/08/2026
 */
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::cmp::{max, min};
use core::sync::atomic::{AtomicU64, Ordering};
use crate::drivers::apic::apic::set_timeout;
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_requests::{usb_request_get_hub_descriptor, usb_request_get_hub_port_status, usb_request_set_hub_port_feature};
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbDeviceId, UsbSetupPacket, UsbTransferDirection, UsbTransferRecipient, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::{WValue};
use crate::drivers::usb::core::UsbSpeed;
use crate::drivers::usb::core::usb_transfers::UsbTransferStatus::{Completed, Disconnected, Error};
use crate::{kprintln_hub_core, kprintln_hub_core_debug};
use crate::memory::dma::{DmaAlloc};

//the device removable mask only covers 31 ports
const HUB_MAX_PORTS: u16 = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsbHubId(pub u64);

impl core::fmt::Display for UsbHubId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub struct UsbHub {
    device: Arc<UsbDevice>,
    system_id: UsbHubId,
    number_of_ports: u16,
    current_port_index: u16,
    is_super_speed: bool,
    hub_descriptor: DmaAlloc,
}

impl UsbHub {
    /// Builds SET_FEATURE(PORT_POWER) for the next unpowered port and advances
    /// `current_port_index`. Returns `None` once every port has been covered.
    fn next_set_port_power_request(&mut self) -> Option<Arc<IrqMutex<UsbTransferRequest>>> {
        let port_number = self.validate_and_set_next_port()?;

        Some(usb_request_set_hub_port_feature(
            self.device.clone(), port_number, WValue::PORT_POWER, on_set_port_power_complete
        ))
    }

    /// Builds GET_HUB_PORT_STATUS for next hub port
    fn next_get_port_status_request(&mut self) -> Option<Arc<IrqMutex<UsbTransferRequest>>> {
        let port_number = self.validate_and_set_next_port()?;

        Some(usb_request_get_hub_port_status(
            self.device.clone(), port_number, on_get_port_status
        ))
    }

    /// Returns current port
    fn validate_and_set_next_port(&mut self) -> Option<u16> {
        if self.current_port_index > self.number_of_ports {
            return None;
        }

        let port_number = self.current_port_index;
        self.current_port_index += 1;
        Some(port_number)
    }
}

pub struct UsbHubCore {
    /// Keyed by `UsbDevice::system_id` of the hub, so completion callbacks can find the hub
    /// straight from `request.target_device`.
    devices: BTreeMap<UsbDeviceId, Arc<IrqMutex<Box<UsbHub>>>>
}

impl UsbHubCore {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    pub fn find_by_system_id(&self, device_system_id: UsbDeviceId) -> Option<Arc<IrqMutex<Box<UsbHub>>>> {
        self.devices.get(&device_system_id).cloned()
    }
}

/// Returns `None` if the descriptor is shorter than it claims or than the hub sent.
fn usb_hub_get_number_of_ports(is_super_speed: bool, dma: &DmaAlloc, bytes_transferred: usize) -> Option<u16> {
    let number_of_ports = UsbHubDescriptor::from_dma(is_super_speed, dma)?.number_of_ports() as u16;

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

fn usb_hub_handle_device_disconnect(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let system_id = request.lock().target_device.system_id;

    let mut hub_core = USB_HUB_CORE.lock();

    match hub_core.devices.remove(&system_id) {
        None => {
            kprintln_hub_core_debug!("Device with system_id {} was already unregistered from hub core.", system_id);
        },
        Some(_) => {
            kprintln_hub_core!(Info, "Device with system_id {} was successfully unregistered from hub core.", system_id);
        }
    };
}

fn on_get_hub_descriptor_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (hub_descriptor, is_super_speed, amount_of_ports, device) = {
        let mut req = request.lock();
        let dev = req.target_device.clone();

        if req.status != Completed {
            kprintln_hub_core!(Error, "Get hub descriptor request failed with status {:?}.", req.status);
            return;
        }

        let Some(setup_packet) = req.setup_packet else {
            kprintln_hub_core!(Error, "Hub descriptor request has no setup packet.");
            return;
        };
        let speed_type = setup_packet.w_value >> 8;

        let is_super_speed_hub = (speed_type & 0xFF) as u8 == WValue::DESC_SUPERSPEED_HUB;
        let Some(descriptor_dma) = req.dma_buffer.take() else {
            kprintln_hub_core!(Error, "Hub descriptor request has no dma buffer.");
            return;
        };

        let Some(amount_of_ports) = usb_hub_get_number_of_ports(
            is_super_speed_hub, &descriptor_dma, req.bytes_transferred
        ) else {
            kprintln_hub_core!(Error, "Malformed hub descriptor ({} bytes received).", req.bytes_transferred);
            return;
        };

        (descriptor_dma, is_super_speed_hub, amount_of_ports, dev)
    };

    let system_id = UsbHubId(USB_HUB_SYSTEM_ID.fetch_add(1, Ordering::AcqRel));
    let mut hub_device = UsbHub {
        device: device.clone(),
        system_id,
        number_of_ports: amount_of_ports,
        current_port_index: 1,
        is_super_speed,
        hub_descriptor,
    };

    //now, set the PORT_POWER for all ports of the hub, one at a time
    let port_power_req = hub_device.next_set_port_power_request();

    USB_HUB_CORE.lock().devices.insert(device.system_id, Arc::new(IrqMutex::new(Box::new(hub_device))));

    let Some(port_power_req) = port_power_req else {
        kprintln_hub_core!(Error, "Hub reported 0 ports, nothing to power.");
        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(port_power_req);
    }
}

//callback is called for each port completion
fn on_set_port_power_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (req, device) = {
        let req1 = request.lock();
        if req1.status != Completed {
            kprintln_hub_core!(Error, "Setting port power on hub failed with status {:?}.", req1.status);
            return;
        }
        let device = req1.target_device.clone();
        (req1, device)
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(device.system_id) else {
        kprintln_hub_core!(Error, "PORT_POWER completed for a device that is not a registered hub.");
        return;
    };

    let port_power_req = hub.lock().next_set_port_power_request();

    let Some(port_power_req) = port_power_req else {
        let (b_pwr_on_to_pwr_good_ms, system_id) = {
            let hub_locked = hub.lock();
            let Some(desc) = UsbHubDescriptor::from_dma(
                hub_locked.is_super_speed,
                &hub_locked.hub_descriptor
            ) else {
                kprintln_hub_core!(Error, "Stored hub descriptor of hub {} is malformed.", hub_locked.system_id);
                return;
            };
            (desc.power_on_to_power_good_in_ms(), hub_locked.system_id)
        };

        //wait until the hub is ready to be powered
        let timeout = max(b_pwr_on_to_pwr_good_ms, 100);
        let device_system_id = device.system_id;
        set_timeout(timeout, move || {
           on_all_ports_powered(device_system_id);
        });

        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(port_power_req);
    }
}

fn on_all_ports_powered(device_system_id: UsbDeviceId) {
    let Some(hub) = USB_HUB_CORE
        .lock()
        .find_by_system_id(device_system_id)
        else {
            kprintln_hub_core!(Info, "Hub with system ID={} disappeared.", device_system_id);
            return;
        };

    //now, we need to start checking for port status using GetPortStatus
    //doing that the same way as SetPortPower - callback for each port checked
   let (get_port_power_req, usb_device) = {
       let mut lock = hub.lock();
       let hub = lock.as_mut();
       let device = hub.device.clone();
       lock.current_port_index = 1;
       (lock.next_get_port_status_request(), device)
   };

    let Some(get_port_power_req) = get_port_power_req else {
        kprintln_hub_core!(Error, "USB hub reports 0 ports on checking port power.");
        return;
    };

    if let Some(controller) = usb_device.host_controller.upgrade() {
        let _ = controller.submit_request(get_port_power_req);
    }
}

fn on_get_port_status(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (get_port_pwr_request, device) = {
        let device = request.lock().target_device.clone();
        let system_id = device.system_id;
        let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(system_id) else {
            kprintln_hub_core!(Info, "Hub with system id {} disappeared.", system_id);
            return;
        };

        (hub.lock().next_get_port_status_request(), device)
    };

    //TODO: check port status and check if port needs to be reset

    let Some(get_port_pwr_request) = get_port_pwr_request else {
        //no more ports to check, so we finished
        on_all_ports_status_checked();
        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(get_port_pwr_request);
    }
}

fn on_all_ports_status_checked() {

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

pub enum UsbHubDescriptor {
    Usb2(Usb2HubDescriptor),
    Usb3(Usb3HubDescriptor),
}

impl UsbHubDescriptor {
    pub fn from_dma(is_super_speed: bool, dma: &DmaAlloc) -> Option<Self> {
        //the buffer is zeroed, so reading past what the hub actually sent would look like valid data
        let received_length = dma.layout.size();
        let buffer = &dma.as_bytes()[..received_length];

        let b_length = *buffer.first()? as usize;
        if b_length > buffer.len() {
            return None;
        }

        if is_super_speed {
            if buffer.len() < size_of::<Usb3HubDescriptor>() {
                return None;
            }

            let descriptor = unsafe {
                core::ptr::read_unaligned(buffer.as_ptr() as *const Usb3HubDescriptor)
            };
            Some(Self::Usb3(descriptor))
        } else {
            Usb2HubDescriptor::parse_from_dma(buffer).map(Self::Usb2)
        }
    }

    pub fn number_of_ports(&self) -> u8 {
        match self {
            Self::Usb2(descriptor) => descriptor.b_nbr_ports,
            Self::Usb3(descriptor) => descriptor.b_nbr_ports,
        }
    }

    pub fn hub_characteristics(&self) -> u16 {
        match self {
            Self::Usb2(descriptor) => descriptor.w_hub_characteristics,
            Self::Usb3(descriptor) => descriptor.w_hub_characteristics,
        }
    }

    /// Returns value in ms, not 2*1ms!
    pub fn power_on_to_power_good_in_ms(&self) -> u64 {
        let power_on_to_power_good_2ms_units = match self {
            Self::Usb2(descriptor) => descriptor.b_pwr_on_to_pwr_good,
            Self::Usb3(descriptor) => descriptor.b_pwr_on_to_pwr_good,
        };

        power_on_to_power_good_2ms_units as u64 * 2
    }
}

pub const HUB_PORT_STATUS_LENGTH: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct Usb2HubPortStatus {
    port_status: u16,
    port_change: u16,
}

impl Usb2HubPortStatus {
    pub fn from_bytes(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < HUB_PORT_STATUS_LENGTH {
            return None;
        }

        Some(Self {
            port_status: u16::from_le_bytes([buffer[0], buffer[1]]),
            port_change: u16::from_le_bytes([buffer[2], buffer[3]]),
        })
    }

    fn status_bit(&self, bit: u8) -> bool {
        self.port_status & (1 << bit) != 0
    }

    fn change_bit(&self, bit: u8) -> bool {
        self.port_change & (1 << bit) != 0
    }

    pub fn raw_port_status(&self) -> u16 { self.port_status }
    pub fn raw_port_change(&self) -> u16 { self.port_change }

    pub fn is_connected(&self) -> bool { self.status_bit(0) }
    pub fn is_enabled(&self) -> bool { self.status_bit(1) }
    pub fn is_suspended(&self) -> bool { self.status_bit(2) }
    pub fn is_over_current(&self) -> bool { self.status_bit(3) }
    pub fn is_resetting(&self) -> bool { self.status_bit(4) }
    pub fn is_powered(&self) -> bool { self.status_bit(8) }
    pub fn is_low_speed(&self) -> bool { self.status_bit(9) }
    pub fn is_high_speed(&self) -> bool { self.status_bit(10) }
    pub fn is_in_test_mode(&self) -> bool { self.status_bit(11) }
    pub fn is_indicator_controlled(&self) -> bool { self.status_bit(12) }

    pub fn speed(&self) -> UsbSpeed {
        if self.is_low_speed() {
            UsbSpeed::Low
        } else if self.is_high_speed() {
            UsbSpeed::High
        } else {
            UsbSpeed::Full
        }
    }

    pub fn connection_changed(&self) -> bool { self.change_bit(0) }
    pub fn enable_changed(&self) -> bool { self.change_bit(1) }
    pub fn suspend_changed(&self) -> bool { self.change_bit(2) }
    pub fn over_current_changed(&self) -> bool { self.change_bit(3) }
    pub fn reset_changed(&self) -> bool { self.change_bit(4) }
}

#[derive(Debug, Clone, Copy)]
pub struct Usb3HubPortStatus {
    port_status: u16,
    port_change: u16,
}

impl Usb3HubPortStatus {
    pub fn from_bytes(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < HUB_PORT_STATUS_LENGTH {
            return None;
        }

        Some(Self {
            port_status: u16::from_le_bytes([buffer[0], buffer[1]]),
            port_change: u16::from_le_bytes([buffer[2], buffer[3]]),
        })
    }

    fn status_bit(&self, bit: u8) -> bool {
        self.port_status & (1 << bit) != 0
    }

    fn change_bit(&self, bit: u8) -> bool {
        self.port_change & (1 << bit) != 0
    }

    pub fn raw_port_status(&self) -> u16 { self.port_status }
    pub fn raw_port_change(&self) -> u16 { self.port_change }

    pub fn is_connected(&self) -> bool { self.status_bit(0) }
    pub fn is_enabled(&self) -> bool { self.status_bit(1) }
    pub fn is_over_current(&self) -> bool { self.status_bit(3) }
    pub fn is_resetting(&self) -> bool { self.status_bit(4) }
    pub fn link_state(&self) -> u8 { ((self.port_status >> 5) & 0xF) as u8 }
    pub fn is_powered(&self) -> bool { self.status_bit(9) }
    pub fn raw_speed(&self) -> u8 { ((self.port_status >> 10) & 0x7) as u8 }

    pub fn connection_changed(&self) -> bool { self.change_bit(0) }
    pub fn over_current_changed(&self) -> bool { self.change_bit(3) }
    pub fn reset_changed(&self) -> bool { self.change_bit(4) }
    pub fn warm_reset_changed(&self) -> bool { self.change_bit(5) }
    pub fn link_state_changed(&self) -> bool { self.change_bit(6) }
    pub fn config_error_changed(&self) -> bool { self.change_bit(7) }
}

pub static USB_HUB_CORE: IrqMutex<UsbHubCore> = IrqMutex::new(UsbHubCore::new());
static USB_HUB_SYSTEM_ID: AtomicU64 = AtomicU64::new(1);

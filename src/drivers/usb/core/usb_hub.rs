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
use crate::drivers::usb::core::usb_requests::{usb_request_clear_hub_port_connection_change, usb_request_clear_hub_port_feature, usb_request_get_hub_descriptor, usb_request_get_hub_port_status, usb_request_set_hub_depth, usb_request_set_hub_port_feature};
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbDeviceId, UsbSetupPacket, UsbTransferDirection, UsbTransferRecipient, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::{WValue};
use crate::drivers::usb::core::UsbSpeed;
use crate::drivers::usb::core::usb_transfers::UsbTransferStatus::{Completed, Disconnected, Error, Stalled};
use crate::drivers::usb::core::usb_core::{
    usb_clear_endpoint_halt, USB_ENDPOINT_DIRECTION_IN, USB_ENDPOINT_TRANSFER_TYPE_INTERRUPT, USB_ENDPOINT_TRANSFER_TYPE_MASK,
};
use crate::drivers::usb::core::usb_requests::usb_request_interrupt_in;
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
    port_states_by_port_number: BTreeMap<u16, UsbHubPortState>,
    port_devices_by_port_number: BTreeMap<u16, Arc<UsbDevice>>,
}

#[derive(Debug, Default, Clone, Copy)]
struct UsbHubPortState {
    debounce_attempts: u8,
    reset_poll_attempts: u8,
    speed_after_reset: Option<UsbSpeed>,
}

impl UsbHub {
    /// Builds SET_FEATURE(PORT_POWER) for the next unpowered port and advances
    /// `current_port_index`. Returns `Ok(None)` once every port has been covered,
    /// `Err` if the request could not be allocated.
    fn next_set_port_power_request(&mut self) -> Result<Option<Arc<IrqMutex<UsbTransferRequest>>>, ()> {
        let Some(port_number) = self.validate_and_set_next_port() else {
            return Ok(None);
        };

        let Some(request) = usb_request_set_hub_port_feature(
            self.device.clone(), port_number, WValue::PORT_POWER, on_set_port_power_complete
        ) else {
            kprintln_hub_core!(Error, "Failed to allocate PORT_POWER request for port {} of hub {}.", port_number, self.system_id);
            return Err(());
        };

        Ok(Some(request))
    }

    /// Builds GET_HUB_PORT_STATUS for next hub port
    fn next_get_port_status_request(&mut self) -> Result<Option<Arc<IrqMutex<UsbTransferRequest>>>, ()> {
        let Some(port_number) = self.validate_and_set_next_port() else {
            return Ok(None);
        };

        let Some(request) = usb_request_get_hub_port_status(
            self.device.clone(), port_number, on_get_port_status
        ) else {
            kprintln_hub_core!(Error, "Failed to allocate GetPortStatus request for port {} of hub {}.", port_number, self.system_id);
            return Err(());
        };

        Ok(Some(request))
    }

    fn port_state_mut(&mut self, port_number: u16) -> &mut UsbHubPortState {
        self.port_states_by_port_number.entry(port_number).or_default()
    }

    fn clear_port_state(&mut self, port_number: u16) {
        self.port_states_by_port_number.remove(&port_number);
    }

    fn record_debounce_bounce(&mut self, port_number: u16) -> u8 {
        let port_state = self.port_state_mut(port_number);
        port_state.debounce_attempts = port_state.debounce_attempts.saturating_add(1);
        port_state.debounce_attempts
    }

    fn clear_debounce_attempts(&mut self, port_number: u16) {
        self.port_state_mut(port_number).debounce_attempts = 0;
    }

    fn record_reset_poll(&mut self, port_number: u16) -> u8 {
        let port_state = self.port_state_mut(port_number);
        port_state.reset_poll_attempts = port_state.reset_poll_attempts.saturating_add(1);
        port_state.reset_poll_attempts
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
    let Some(full_req) = usb_request_get_hub_descriptor(dev.clone(), is_superspeed, on_get_hub_descriptor_complete) else {
        kprintln_hub_core!(Error, "Failed to allocate hub descriptor request for device {}.", dev.system_id);
        return;
    };

    if let Some(controller) = dev.host_controller.upgrade() {
        let _ = controller.submit_request(full_req);
    }
}

/// The hub was disconnected by the controller so only the devices behind it are left.
fn usb_hub_handle_device_disconnect(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let system_id = request.lock().target_device.system_id;

    let hub = USB_HUB_CORE.lock().devices.remove(&system_id);

    match hub {
        None => {
            kprintln_hub_core_debug!("Device with system_id {} was already unregistered from hub core.", system_id);
        },
        Some(hub) => {
            kprintln_hub_core!(Info, "Device with system_id {} was successfully unregistered from hub core.", system_id);
            usb_hub_disconnect_port_devices(&hub);
        }
    };
}

/// Called by usb core for every device registered with a parent hub.
pub fn usb_hub_register_port_device(hub_device: &Arc<UsbDevice>, port_number: u16, device: Arc<UsbDevice>) {
    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core!(Warn, "Device {} registered behind hub device {}, which is not a registered hub.", device.system_id, hub_device.system_id);
        return;
    };

    let previous = hub.lock().port_devices_by_port_number.insert(port_number, device.clone());
    if let Some(previous) = previous {
        //the old device on that port was never disconnected, do it now so its slot is released
        kprintln_hub_core!(Warn, "Port {} of hub device {} still had device {}, disconnecting it.", port_number, hub_device.system_id, previous.system_id);
        usb_hub_disconnect_device_tree(previous);
    }

    kprintln_hub_core_debug!("Device {} is on port {} of hub device {}.", device.system_id, port_number, hub_device.system_id);
}

/// Disconnects `device` and, if it is a hub, everything behind it first (depth first).
fn usb_hub_disconnect_device_tree(device: Arc<UsbDevice>) {
    let hub = USB_HUB_CORE.lock().devices.remove(&device.system_id);
    if let Some(hub) = hub {
        usb_hub_disconnect_port_devices(&hub);
    }

    let Some(controller) = device.host_controller.upgrade() else {
        kprintln_hub_core!(Warn, "Host controller of device {} is gone, cannot disconnect it.", device.system_id);
        return;
    };

    if let Err(error) = controller.disconnect_device(device.hardware_id) {
        kprintln_hub_core!(Warn, "Disconnecting device {} failed: {}", device.system_id, error);
    }
}

fn usb_hub_disconnect_port_devices(hub: &Arc<IrqMutex<Box<UsbHub>>>) {
    //taken out first, the recursion locks other hubs and the hub core
    let port_devices = core::mem::take(&mut hub.lock().port_devices_by_port_number);
    for (_, port_device) in port_devices {
        usb_hub_disconnect_device_tree(port_device);
    }
}

fn usb_hub_depth(hub_device: &UsbDevice) -> u16 {
    let mut depth = 0;
    let mut parent = hub_device.parent_hub.clone();
    while let Some(hub) = parent {
        depth += 1;
        parent = hub.parent_hub.clone();
    }
    depth
}

fn issue_set_hub_depth_usb3(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let device = {
        let req = request.lock();
        let usb_dev = req.target_device.clone();
        usb_dev
    };

    let hub_depth = usb_hub_depth(&device);
    let Some(set_hub_depth_req) =
        usb_request_set_hub_depth(device.clone(), hub_depth, on_set_hub_depth_completion)
        else {
            kprintln_hub_core!(Error, "Building SET_HUB_DEPTH request failed.");
            return;
        };


    if let Some(controller) = device.host_controller.upgrade() {
        let Ok(a) = controller.submit_request(set_hub_depth_req) else {
            kprintln_hub_core!(Error, "Submitting SET_HUB_DEPTH req failed.");
            return;
        };
    }
}

fn on_set_hub_depth_completion(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let status = {request.lock().status};
    if status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let device = {
        let req = request.lock();
        let usb_dev = req.target_device.clone();
        usb_dev
    };

    if status != Completed {
        kprintln_hub_core!(Error, "SET_HUB_DEPTH for hub device {} failed with status {:?}.", device.system_id, status);
        return;
    }

    //start the port power chain after setting hub depth
    issue_set_port_power_for_first_port(device);
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
        port_states_by_port_number: BTreeMap::new(),
        port_devices_by_port_number: BTreeMap::new(),
    };
    USB_HUB_CORE.lock().devices.insert(device.system_id, Arc::new(IrqMutex::new(Box::new(hub_device))));

    //for super speed hubs we need to configure its depth, so that route string can be set properly
    if is_super_speed {
        issue_set_hub_depth_usb3(request.clone());
        return;
    }

    issue_set_port_power_for_first_port(device);
}

fn issue_set_port_power_for_first_port(device: Arc<UsbDevice>) {
    //now, set the PORT_POWER for all ports of the hub, one at a time
    let port_power_req = {
        let Some(hub_dev_lock) = USB_HUB_CORE
            .lock()
            .find_by_system_id(device.system_id)
            else {
                kprintln_hub_core_debug!("Hub with system_id={} disappeared.", device.system_id);
                return;
            };
        let mut hub_dev = hub_dev_lock.lock();

        let port_power_req = hub_dev.next_set_port_power_request();
        port_power_req
    };

    let Ok(port_power_req) = port_power_req else {
        return;
    };

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

    let Ok(port_power_req) = hub.lock().next_set_port_power_request() else {
        return;
    };

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

    let Ok(get_port_power_req) = get_port_power_req else {
        return;
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

    let (status, port_number, received_bytes, device) = {
        let req = request.lock();
        let port_number = req.setup_packet.map(|setup_packet| setup_packet.w_index);
        let received_bytes = req.received_bytes().and_then(|bytes| bytes.get(..HUB_PORT_STATUS_LENGTH)).map(|bytes| [bytes[0], bytes[1], bytes[2], bytes[3]]);
        (req.status, port_number, received_bytes, req.target_device.clone())
    };

    let system_id = device.system_id;
    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(system_id) else {
        kprintln_hub_core!(Info, "Hub with system id {} disappeared.", system_id);
        return;
    };
    let is_super_speed = hub.lock().is_super_speed;

    //a failed port is skipped, the remaining ports are still checked
    match (status, port_number, received_bytes) {
        (Completed, Some(port_number), Some(bytes)) => {
            if usb_hub_port_needs_reset(is_super_speed, port_number, &bytes) {
                hub.lock().clear_port_state(port_number);
                usb_hub_reset_port(device.clone(), port_number);
            }
        }
        (Completed, port_number, _) => {
            kprintln_hub_core!(Error, "GetPortStatus of hub {} port {:?} returned too few bytes.", system_id, port_number);
        }
        (status, port_number, _) => {
            kprintln_hub_core!(Error, "GetPortStatus of hub {} port {:?} failed with status {:?}.", system_id, port_number, status);
        }
    }

    let get_port_pwr_request = hub.lock().next_get_port_status_request();

    let Ok(get_port_pwr_request) = get_port_pwr_request else {
        return;
    };

    let Some(get_port_pwr_request) = get_port_pwr_request else {
        //no more ports to check, so we finished
        on_all_ports_status_checked(device);
        return;
    };

    if let Some(controller) = device.host_controller.upgrade() {
        let _ = controller.submit_request(get_port_pwr_request);
    }
}

fn on_all_ports_status_checked(hub_device: Arc<UsbDevice>) {
    usb_hub_start_status_change_listening(hub_device);
}

fn usb_hub_status_change_endpoint_address(hub_device: &UsbDevice) -> Option<u8> {
    hub_device
        .configuration_tree
        .get()?
        .interfaces
        .iter()
        .flat_map(|interface| interface.endpoints.iter())
        .find(|endpoint| {
            let is_in = endpoint.b_endpoint_address & USB_ENDPOINT_DIRECTION_IN != 0;
            let is_interrupt = endpoint.bm_attributes & USB_ENDPOINT_TRANSFER_TYPE_MASK == USB_ENDPOINT_TRANSFER_TYPE_INTERRUPT;
            is_in && is_interrupt
        })
        .map(|endpoint| endpoint.b_endpoint_address)
}

fn usb_hub_start_status_change_listening(hub_device: Arc<UsbDevice>) {
    let Some(endpoint_address) = usb_hub_status_change_endpoint_address(&hub_device) else {
        kprintln_hub_core!(Error, "Hub device {} has no status change endpoint.", hub_device.system_id);
        return;
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared before listening for status changes.", hub_device.system_id);
        return;
    };

    //bit 0 is the hub, bit N is port N
    let bitmap_length_bytes = (hub.lock().number_of_ports as usize + 1).div_ceil(8);

    let Some(request) = usb_request_interrupt_in(
        hub_device.clone(), endpoint_address, bitmap_length_bytes, on_hub_status_change
    ) else {
        kprintln_hub_core!(Error, "Failed to allocate the status change request of hub device {}.", hub_device.system_id);
        return;
    };

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to submit the status change request of hub device {}: {}", hub_device.system_id, error);
        }
    }
}

fn on_hub_status_change(request: Arc<IrqMutex<UsbTransferRequest>>) {
    //the condition temporary drops the request lock before the call
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, hub_device, endpoint_address, changed_bitmap) = {
        let req = request.lock();
        let mut changed_bitmap = [0u8; 4];
        if let Some(bytes) = req.received_bytes() {
            let length = bytes.len().min(changed_bitmap.len());
            changed_bitmap[..length].copy_from_slice(&bytes[..length]);
        }
        (req.status, req.target_device.clone(), req.endpoint_address, changed_bitmap)
    };

    if status == Stalled || status == Error {
        //listening starts again with a fresh request once the halt is cleared
        if let Err(error) = usb_clear_endpoint_halt(hub_device.clone(), endpoint_address, on_hub_status_change_endpoint_cleared) {
            kprintln_hub_core!(Error, "Clearing the halted status change endpoint of hub device {} failed: {}", hub_device.system_id, error);
        }
        return;
    }

    if status != Completed {
        kprintln_hub_core!(Error, "Status change request of hub device {} failed with status {:?}.", hub_device.system_id, status);
        return;
    }

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Status change for hub device {} that is not registered.", hub_device.system_id);
        return;
    };
    let number_of_ports = hub.lock().number_of_ports;

    kprintln_hub_core_debug!("Hub device {} status change bitmap {:02x?}.", hub_device.system_id, changed_bitmap);

    if changed_bitmap[0] & 1 != 0 {
        usb_hub_handle_hub_change(hub_device.clone());
    }

    for port_number in 1..=number_of_ports {
        let bit_index = port_number as usize;
        if changed_bitmap[bit_index / 8] & (1 << (bit_index % 8)) == 0 {
            continue;
        }

        //a port in the middle of its debounce or reset is handled by that sequence
        if hub.lock().port_states_by_port_number.contains_key(&port_number) {
            continue;
        }

        usb_hub_handle_port_change(hub_device.clone(), port_number);
    }

    {
        let mut req = request.lock();
        req.status = UsbTransferStatus::Pending;
        req.bytes_transferred = 0;
    }

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to resubmit the status change request of hub device {}: {}", hub_device.system_id, error);
        }
    }
}

fn on_hub_status_change_endpoint_cleared(request: Arc<IrqMutex<UsbTransferRequest>>) {
    let (status, hub_device) = {
        let req = request.lock();
        (req.status, req.target_device.clone())
    };

    if status != Completed {
        kprintln_hub_core!(Error, "ClearFeature(ENDPOINT_HALT) on the status change endpoint of hub device {} failed with status {:?}.", hub_device.system_id, status);
        return;
    }

    usb_hub_start_status_change_listening(hub_device);
}

fn usb_hub_handle_hub_change(hub_device: Arc<UsbDevice>) {
    //TODO: GetHubStatus, clear C_HUB_LOCAL_POWER / C_HUB_OVER_CURRENT
}

fn usb_hub_handle_port_change(hub_device: Arc<UsbDevice>, port_number: u16) {
    let request = usb_request_get_hub_port_status(hub_device.clone(), port_number, on_port_change_status);
    usb_hub_submit_port_request(&hub_device, port_number, "GetPortStatus for a port change", request);
}

fn on_port_change_status(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, snapshot, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), UsbHubPortSnapshot::from_request(&req), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "GetPortStatus request has no setup packet.");
        return;
    };

    if status != Completed {
        kprintln_hub_core!(Error, "GetPortStatus for a change on port {} of hub device {} failed with status {:?}.", port_number, hub_device.system_id, status);
        return;
    }

    let Some(snapshot) = snapshot else {
        kprintln_hub_core!(Error, "GetPortStatus for a change on port {} of hub device {} is malformed or the hub is gone.", port_number, hub_device.system_id);
        return;
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared during a change on port {}.", hub_device.system_id, port_number);
        return;
    };
    let is_super_speed = hub.lock().is_super_speed;

    kprintln_hub_core_debug!(
        "Port {} of hub device {} changed: wPortChange={:#06x} connected={} enabled={}.",
        port_number, hub_device.system_id, snapshot.raw_port_change, snapshot.is_connected, snapshot.is_enabled
    );

    //the status change request may have been sent before a debounce or reset of this port started
    if hub.lock().port_states_by_port_number.contains_key(&port_number) {
        return;
    }

    let mut connection_change_cleared_by_reset = false;

    if snapshot.connection_changed {
        //a quick replug reports a single change, the old device is gone either way
        let previous_device = hub.lock().port_devices_by_port_number.remove(&port_number);
        if let Some(previous_device) = previous_device {
            kprintln_hub_core!(Info, "Device {} on port {} of hub device {} was disconnected.", previous_device.system_id, port_number, hub_device.system_id);
            usb_hub_disconnect_device_tree(previous_device);
        }

        if snapshot.is_connected {
            kprintln_hub_core!(Info, "Device connected on port {} of hub device {}.", port_number, hub_device.system_id);
            hub.lock().clear_port_state(port_number);
            //starts with ClearPortFeature(C_PORT_CONNECTION) itself
            usb_hub_reset_port(hub_device.clone(), port_number);
            connection_change_cleared_by_reset = true;
        }
    }

    for feature_selector in usb_hub_port_change_feature_selectors(is_super_speed, snapshot.raw_port_change) {
        if feature_selector == WValue::C_PORT_CONNECTION && connection_change_cleared_by_reset {
            continue;
        }

        if feature_selector != WValue::C_PORT_CONNECTION {
            kprintln_hub_core!(Warn, "Port {} of hub device {} reported change feature {}, clearing it.", port_number, hub_device.system_id, feature_selector);
        }

        let request = usb_request_clear_hub_port_feature(
            hub_device.clone(), port_number, feature_selector, on_port_change_cleared
        );
        usb_hub_submit_port_request(&hub_device, port_number, "ClearPortFeature for a port change", request);
    }
}

fn on_port_change_cleared(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let req = request.lock();
    if req.status != Completed {
        kprintln_hub_core!(
            Error,
            "ClearPortFeature({:?}) for port {:?} of hub device {} failed with status {:?}.",
            req.setup_packet.map(|setup_packet| setup_packet.w_value),
            req.setup_packet.map(|setup_packet| setup_packet.w_index),
            req.target_device.system_id,
            req.status
        );
    }
}

fn usb_hub_port_change_feature_selectors(is_super_speed: bool, raw_port_change: u16) -> impl Iterator<Item = u16> {
    const USB2_CHANGE_BITS: [(u8, u16); 5] = [
        (0, WValue::C_PORT_CONNECTION),
        (1, WValue::C_PORT_ENABLE),
        (2, WValue::C_PORT_SUSPEND),
        (3, WValue::C_PORT_OVER_CURRENT),
        (4, WValue::C_PORT_RESET),
    ];
    const USB3_CHANGE_BITS: [(u8, u16); 6] = [
        (0, WValue::C_PORT_CONNECTION),
        (3, WValue::C_PORT_OVER_CURRENT),
        (4, WValue::C_PORT_RESET),
        (5, WValue::C_BH_PORT_RESET),
        (6, WValue::C_PORT_LINK_STATE),
        (7, WValue::C_PORT_CONFIG_ERROR),
    ];

    let change_bits: &'static [(u8, u16)] = if is_super_speed { &USB3_CHANGE_BITS } else { &USB2_CHANGE_BITS };
    change_bits
        .iter()
        .filter(move |(bit, _)| raw_port_change & (1 << bit) != 0)
        .map(|(_, feature_selector)| *feature_selector)
}

fn usb_hub_port_needs_reset(is_super_speed: bool, port_number: u16, bytes: &[u8; HUB_PORT_STATUS_LENGTH]) -> bool {
    let (is_connected, is_enabled, is_resetting, connection_changed, raw_status, raw_change) = if is_super_speed {
        let Some(port_status) = Usb3HubPortStatus::from_bytes(bytes) else { return false; };
        (
            port_status.is_connected(), port_status.is_enabled(), port_status.is_resetting(),
            port_status.connection_changed(), port_status.raw_port_status(), port_status.raw_port_change(),
        )
    } else {
        let Some(port_status) = Usb2HubPortStatus::from_bytes(bytes) else { return false; };
        (
            port_status.is_connected(), port_status.is_enabled(), port_status.is_resetting(),
            port_status.connection_changed(), port_status.raw_port_status(), port_status.raw_port_change(),
        )
    };

    kprintln_hub_core_debug!(
        "Port {}: wPortStatus={:#06x} wPortChange={:#06x} connected={} enabled={} resetting={}.",
        port_number, raw_status, raw_change, is_connected, is_enabled, is_resetting
    );

    is_connected && !is_resetting && (connection_changed || !is_enabled)
}

const HUB_PORT_CONNECT_DEBOUNCE_MS: u64 = 100;
const HUB_PORT_MAX_DEBOUNCE_ATTEMPTS: u8 = 15;

fn usb_hub_reset_port(hub_device: Arc<UsbDevice>, port_number: u16) {
    //the entry marks the port as busy, so the status change handler leaves it alone
    if let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) {
        hub.lock().port_state_mut(port_number);
    }

    let Some(request) = usb_request_clear_hub_port_connection_change(
        hub_device.clone(), port_number, on_clear_port_connection_change_complete
    ) else {
        kprintln_hub_core!(Error, "Failed to allocate ClearPortFeature(C_PORT_CONNECTION) for port {} of hub device {}.", port_number, hub_device.system_id);
        usb_hub_abort_port_sequence(&hub_device, port_number);
        return;
    };

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to submit ClearPortFeature(C_PORT_CONNECTION) for port {}: {}", port_number, error);
            usb_hub_abort_port_sequence(&hub_device, port_number);
        }
    }
}

fn usb_hub_abort_port_sequence(hub_device: &Arc<UsbDevice>, port_number: u16) {
    if let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) {
        hub.lock().clear_port_state(port_number);
    }
}

fn on_clear_port_connection_change_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "ClearPortFeature(C_PORT_CONNECTION) request has no setup packet.");
        return;
    };

    if status != Completed {
        kprintln_hub_core!(Error, "ClearPortFeature(C_PORT_CONNECTION) for port {} of hub device {} failed with status {:?}.", port_number, hub_device.system_id, status);
        usb_hub_abort_port_sequence(&hub_device, port_number);
        return;
    }

    set_timeout(HUB_PORT_CONNECT_DEBOUNCE_MS, move || {
        on_port_connection_debounced(hub_device, port_number);
    });
}

fn on_port_connection_debounced(hub_device: Arc<UsbDevice>, port_number: u16) {
    if USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id).is_none() {
        kprintln_hub_core_debug!("Hub device {} disappeared during the debounce of port {}.", hub_device.system_id, port_number);
        return;
    }

    //the connection has to be stable for the whole debounce, so the port is checked again
    let Some(request) = usb_request_get_hub_port_status(
        hub_device.clone(), port_number, on_debounced_port_status
    ) else {
        kprintln_hub_core!(Error, "Failed to allocate GetPortStatus after the debounce of port {}.", port_number);
        return;
    };

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to submit GetPortStatus after the debounce of port {}: {}", port_number, error);
        }
    }
}

fn on_debounced_port_status(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, snapshot, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), UsbHubPortSnapshot::from_request(&req), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "GetPortStatus request has no setup packet.");
        return;
    };

    if status != Completed {
        kprintln_hub_core!(Error, "GetPortStatus after the debounce of port {} failed with status {:?}.", port_number, status);
        usb_hub_abort_port_sequence(&hub_device, port_number);
        return;
    }

    let Some(UsbHubPortSnapshot { is_connected, connection_changed, .. }) = snapshot else {
        kprintln_hub_core!(Error, "GetPortStatus after the debounce of port {} is malformed or the hub is gone.", port_number);
        usb_hub_abort_port_sequence(&hub_device, port_number);
        return;
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared after the debounce of port {}.", hub_device.system_id, port_number);
        return;
    };

    if !is_connected {
        hub.lock().clear_port_state(port_number);
        kprintln_hub_core_debug!("Device on port {} of hub device {} left during the debounce.", port_number, hub_device.system_id);
        return;
    }

    if connection_changed {
        let attempts = hub.lock().record_debounce_bounce(port_number);
        if attempts >= HUB_PORT_MAX_DEBOUNCE_ATTEMPTS {
            hub.lock().clear_port_state(port_number);
            kprintln_hub_core!(
                Warn,
                "Connection on port {} of hub device {} kept bouncing for {} ms, giving up.",
                port_number,
                hub_device.system_id,
                attempts as u64 * HUB_PORT_CONNECT_DEBOUNCE_MS
            );
            return;
        }

        kprintln_hub_core_debug!(
            "Connection on port {} of hub device {} bounced ({}/{}), debouncing again.",
            port_number, hub_device.system_id, attempts, HUB_PORT_MAX_DEBOUNCE_ATTEMPTS
        );
        usb_hub_reset_port(hub_device, port_number);
        return;
    }

    hub.lock().clear_debounce_attempts(port_number);

    kprintln_hub_core_debug!("Port {} of hub device {} debounced, resetting.", port_number, hub_device.system_id);

    let Some(request) = usb_request_set_hub_port_feature(
        hub_device.clone(), port_number, WValue::PORT_RESET, on_port_reset_started
    ) else {
        kprintln_hub_core!(Error, "Failed to allocate SetPortFeature(PORT_RESET) for port {}.", port_number);
        return;
    };

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to submit SetPortFeature(PORT_RESET) for port {}: {}", port_number, error);
        }
    }
}

fn on_port_reset_started(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "SetPortFeature(PORT_RESET) request has no setup packet.");
        return;
    };

    if status != Completed {
        kprintln_hub_core!(Error, "SetPortFeature(PORT_RESET) for port {} failed with status {:?}.", port_number, status);
        usb_hub_abort_port_sequence(&hub_device, port_number);
        return;
    }

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared after resetting port {}.", hub_device.system_id, port_number);
        return;
    };
    hub.lock().port_state_mut(port_number).reset_poll_attempts = 0;

    schedule_port_reset_poll(hub_device, port_number);
}

const HUB_PORT_RESET_POLL_INTERVAL_MS: u64 = 10;
const HUB_PORT_RESET_TIMEOUT_MS: u64 = 800;
const HUB_PORT_RESET_RECOVERY_MS: u64 = 10;

fn schedule_port_reset_poll(hub_device: Arc<UsbDevice>, port_number: u16) {
    set_timeout(HUB_PORT_RESET_POLL_INTERVAL_MS, move || {
        let request = usb_request_get_hub_port_status(hub_device.clone(), port_number, on_port_reset_poll_status);
        usb_hub_submit_port_request(&hub_device, port_number, "GetPortStatus during reset", request);
    });
}

fn on_port_reset_poll_status(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, snapshot, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), UsbHubPortSnapshot::from_request(&req), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "GetPortStatus request has no setup packet.");
        return;
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared during the reset of port {}.", hub_device.system_id, port_number);
        return;
    };

    if status != Completed {
        hub.lock().clear_port_state(port_number);
        kprintln_hub_core!(Error, "GetPortStatus during the reset of port {} failed with status {:?}.", port_number, status);
        return;
    }

    let Some(snapshot) = snapshot else {
        hub.lock().clear_port_state(port_number);
        kprintln_hub_core!(Error, "GetPortStatus during the reset of port {} is malformed.", port_number);
        return;
    };

    if !snapshot.is_connected {
        hub.lock().clear_port_state(port_number);
        kprintln_hub_core_debug!("Device on port {} of hub device {} left during the reset.", port_number, hub_device.system_id);
        return;
    }

    if !snapshot.reset_changed {
        let attempts = hub.lock().record_reset_poll(port_number);
        let waited_ms = attempts as u64 * HUB_PORT_RESET_POLL_INTERVAL_MS;

        if waited_ms >= HUB_PORT_RESET_TIMEOUT_MS {
            hub.lock().clear_port_state(port_number);
            kprintln_hub_core!(Warn, "Reset of port {} of hub device {} did not finish in {} ms, giving up.", port_number, hub_device.system_id, waited_ms);
            return;
        }

        schedule_port_reset_poll(hub_device, port_number);
        return;
    }

    //a reset that leaves the port disabled failed, C_PORT_RESET still has to be cleared
    hub.lock().port_state_mut(port_number).speed_after_reset = snapshot.is_enabled.then_some(snapshot.speed);

    let request = usb_request_clear_hub_port_feature(
        hub_device.clone(), port_number, WValue::C_PORT_RESET, on_clear_port_reset_change_complete
    );
    usb_hub_submit_port_request(&hub_device, port_number, "ClearPortFeature(C_PORT_RESET)", request);
}

fn on_clear_port_reset_change_complete(request: Arc<IrqMutex<UsbTransferRequest>>) {
    if request.lock().status == Disconnected {
        usb_hub_handle_device_disconnect(request);
        return;
    }

    let (status, port_number, hub_device) = {
        let req = request.lock();
        (req.status, req.setup_packet.map(|setup_packet| setup_packet.w_index), req.target_device.clone())
    };

    let Some(port_number) = port_number else {
        kprintln_hub_core!(Error, "ClearPortFeature(C_PORT_RESET) request has no setup packet.");
        return;
    };

    let Some(hub) = USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id) else {
        kprintln_hub_core_debug!("Hub device {} disappeared during the reset of port {}.", hub_device.system_id, port_number);
        return;
    };

    let speed_after_reset = {
        let mut hub = hub.lock();
        let speed_after_reset = hub.port_state_mut(port_number).speed_after_reset;
        hub.clear_port_state(port_number);
        speed_after_reset
    };

    if status != Completed {
        kprintln_hub_core!(Error, "ClearPortFeature(C_PORT_RESET) for port {} failed with status {:?}.", port_number, status);
        return;
    }

    let Some(speed) = speed_after_reset else {
        kprintln_hub_core!(Error, "Port {} of hub device {} is not enabled after the reset.", port_number, hub_device.system_id);
        return;
    };

    set_timeout(HUB_PORT_RESET_RECOVERY_MS, move || {
        on_port_reset_recovered(hub_device, port_number, speed);
    });
}

fn on_port_reset_recovered(hub_device: Arc<UsbDevice>, port_number: u16, speed: UsbSpeed) {
    if USB_HUB_CORE.lock().find_by_system_id(hub_device.system_id).is_none() {
        kprintln_hub_core_debug!("Hub device {} disappeared during the reset recovery of port {}.", hub_device.system_id, port_number);
        return;
    }

    kprintln_hub_core_debug!("Port {} of hub device {} is reset and enabled, speed {:?}.", port_number, hub_device.system_id, speed);

    let Some(controller) = hub_device.host_controller.upgrade() else {
        kprintln_hub_core!(Warn, "Host controller of hub device {} is gone, port {} is not enumerated.", hub_device.system_id, port_number);
        return;
    };

    //hub ports go up to 255 (bNbrPorts is a byte), HUB_MAX_PORTS caps them lower already
    if let Err(error) = controller.enumerate_device_behind_hub(&hub_device, port_number as u8, speed) {
        kprintln_hub_core!(Error, "Failed to enumerate the device on port {} of hub device {}: {}", port_number, hub_device.system_id, error);
    }
}

#[derive(Debug, Clone, Copy)]
struct UsbHubPortSnapshot {
    is_connected: bool,
    is_enabled: bool,
    connection_changed: bool,
    reset_changed: bool,
    speed: UsbSpeed,
    raw_port_change: u16,
}

impl UsbHubPortSnapshot {
    fn parse(is_super_speed: bool, bytes: &[u8]) -> Option<Self> {
        if is_super_speed {
            let port_status = Usb3HubPortStatus::from_bytes(bytes)?;
            Some(Self {
                is_connected: port_status.is_connected(),
                is_enabled: port_status.is_enabled(),
                connection_changed: port_status.connection_changed(),
                reset_changed: port_status.reset_changed(),
                raw_port_change: port_status.raw_port_change(),
                //a USB3 hub downstream port only runs SuperSpeed, raw speed 0 = 5 Gb/s
                speed: UsbSpeed::SuperSpeed,
            })
        } else {
            let port_status = Usb2HubPortStatus::from_bytes(bytes)?;
            Some(Self {
                is_connected: port_status.is_connected(),
                is_enabled: port_status.is_enabled(),
                connection_changed: port_status.connection_changed(),
                reset_changed: port_status.reset_changed(),
                raw_port_change: port_status.raw_port_change(),
                speed: port_status.speed(),
            })
        }
    }

    fn from_request(request: &UsbTransferRequest) -> Option<Self> {
        let is_super_speed = USB_HUB_CORE
            .lock()
            .find_by_system_id(request.target_device.system_id)
            .map(|hub| hub.lock().is_super_speed)?;
        Self::parse(is_super_speed, request.received_bytes()?)
    }
}

fn usb_hub_submit_port_request(hub_device: &Arc<UsbDevice>, port_number: u16, request_name: &str, request: Option<Arc<IrqMutex<UsbTransferRequest>>>) {
    let Some(request) = request else {
        kprintln_hub_core!(Error, "Failed to allocate {} for port {} of hub device {}.", request_name, port_number, hub_device.system_id);
        return;
    };

    if let Some(controller) = hub_device.host_controller.upgrade() {
        if let Err(error) = controller.submit_request(request) {
            kprintln_hub_core!(Error, "Failed to submit {} for port {} of hub device {}: {}", request_name, port_number, hub_device.system_id, error);
        }
    }
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

/*
 * Created by Antoni Kuczyński
 * 22/07/2026
 */
use alloc::sync::{Arc, Weak};
use spin::Once;
use crate::drivers::usb::core::usb_descriptors::{UsbConfigurationTree, UsbDeviceDescriptor};
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::UsbSpeed;
use crate::memory::dma::DmaAlloc;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum UsbTransferType {
    Control = 0,
    Isochronous = 1,
    Bulk = 2,
    Interrupt = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum UsbTransferStatus {
    Pending = 0,
    Completed = 1,
    Stalled = 2,
    Error = 3,
    Disconnected = 4
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum UsbTransferDirection {
    HostToDevice = 0,
    DeviceToHost = 1
}

impl UsbTransferDirection {
    pub const HOST_TO_DEVICE: u8 = 0;
    pub const DEVICE_TO_HOST: u8 = 1;
}

/// Recipient field of bmRequestType (bits 4:0).
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum UsbTransferRecipient {
    Device = 0,
    Interface = 1,
    Endpoint = 2,
    Other = 3,
}

impl UsbTransferRecipient {
    pub const DEVICE: u8 = 0;
    pub const INTERFACE: u8 = 1;
    pub const ENDPOINT: u8 = 2;
    /// Used by hub class requests to target a hub port (wIndex = port number).
    pub const OTHER: u8 = 3;
}

/// A host controller driver.
pub trait UsbHostController: Send + Sync {
    fn submit_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str>;
    
    fn cancel_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str>;

    /// USB address the controller assigned to the device
    fn device_address(&self, hardware_id: u8) -> Option<u8>;

    /// Enumerates a device on hub_port of hub
    fn enumerate_device_behind_hub(&self, hub: &Arc<UsbDevice>, hub_port: u8, speed: UsbSpeed) -> Result<(), &'static str>;

    /// Fails every request of the device with `Disconnected` and releases it in the controller
    fn disconnect_device(&self, hardware_id: u8) -> Result<(), &'static str>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsbDeviceId(pub u64);

impl core::fmt::Display for UsbDeviceId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub struct UsbDevice {
    pub system_id: UsbDeviceId,
    pub hardware_id: u8,
    pub host_controller: Weak<dyn UsbHostController>, //weak to prevent controller reference count loop
    pub configuration_tree: Once<UsbConfigurationTree>,
    pub device_descriptor: Once<UsbDeviceDescriptor>,

    pub parent_hub: Option<Arc<UsbDevice>>,
    pub port_on_parent: u8,
    pub speed: UsbSpeed
}

pub type UsbTransferCallback = fn(Arc<IrqMutex<UsbTransferRequest>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct UsbSetupPacket {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl UsbSetupPacket {
    pub const fn new(
        bm_request_type: u8,
        b_request: u8,
        w_value: u16,
        w_index: u16,
        w_length: u16,
    ) -> Self {
        Self {
            bm_request_type,
            b_request,
            w_value,
            w_index,
            w_length,
        }
    }

    pub fn as_bytes(&self) -> [u8; 8] {
        let val = self.w_value.to_le_bytes();
        let idx = self.w_index.to_le_bytes();
        let len = self.w_length.to_le_bytes();
        [
            self.bm_request_type,
            self.b_request,
            val[0], val[1],
            idx[0], idx[1],
            len[0], len[1],
        ]
    }
}

pub struct UsbTransferRequest {
    pub target_device: Arc<UsbDevice>,
    pub endpoint_address: u8,
    pub transfer_direction: UsbTransferDirection,
    pub transfer_type: UsbTransferType,
    pub setup_packet: Option<UsbSetupPacket>,
    pub dma_buffer: Option<DmaAlloc>,
    pub data_buffer_length: usize,
    pub status: UsbTransferStatus,
    pub bytes_transferred: usize,
    pub completion_callback: Option<UsbTransferCallback>,
}

unsafe impl Send for UsbTransferRequest {}
unsafe impl Sync for UsbTransferRequest {}

impl UsbTransferRequest {
    /// Only the bytes the device actually sent, not the whole buffer.
    pub fn received_bytes(&self) -> Option<&[u8]> {
        self.dma_buffer.as_ref()?.as_bytes().get(..self.bytes_transferred)
    }

    pub fn read_received<T: Copy>(&self) -> Option<T> {
        let bytes = self.received_bytes()?;
        if bytes.len() < size_of::<T>() {
            return None;
        }
        Some(unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const T) })
    }

    pub fn new_interrupt_in(
        device: Arc<UsbDevice>,
        endpoint_num: u8,
        dma_buffer: DmaAlloc,
        buffer_length: usize,
        callback: UsbTransferCallback,
    ) -> Self {
        Self {
            target_device: device,
            endpoint_address: endpoint_num,
            transfer_direction: UsbTransferDirection::HostToDevice,
            transfer_type: UsbTransferType::Interrupt,
            setup_packet: None,
            dma_buffer: Some(dma_buffer),
            data_buffer_length: buffer_length,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(callback),
        }
    }
}
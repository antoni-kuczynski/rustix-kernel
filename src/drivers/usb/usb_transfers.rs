/*
 * Created by Antoni Kuczyński
 * 22/07/2026
 */
use alloc::sync::{Arc, Weak};
use spin::Once;
use crate::drivers::usb::descriptors::{UsbConfigurationTree, UsbDeviceDescriptor};
use crate::drivers::usb::irq_mutex::IrqMutex;
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum UsbTransferDirection {
    HostToDevice = 0,
    DeviceToHost = 1
}

/// A host controller driver.
///
/// The methods take `&self` because a controller is expected to guard its rings and devices
/// individually - requiring `&mut self` would force every submission to take a lock covering the
/// whole controller and block its interrupt handlers.
pub trait UsbHostController: Send + Sync {
    fn submit_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str>;

    fn cancel_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str>;
}

pub struct UsbDevice {
    pub system_id: u64,
    pub hardware_id: u8,
    pub host_controller: Weak<dyn UsbHostController>, //weak to prevent controller reference count loop
    pub configuration_tree: Once<UsbConfigurationTree>,
    pub device_descriptor: Once<UsbDeviceDescriptor>
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
    pub dma_buffer: DmaAlloc,
    pub data_buffer_length: usize,
    pub status: UsbTransferStatus,
    pub bytes_transferred: usize,
    pub completion_callback: Option<UsbTransferCallback>,
}

unsafe impl Send for UsbTransferRequest {}
unsafe impl Sync for UsbTransferRequest {}

impl UsbTransferRequest {
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
            dma_buffer: dma_buffer,
            data_buffer_length: buffer_length,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(callback),
        }
    }
}
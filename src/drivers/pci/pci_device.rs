#![allow(dead_code)]
/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */
use crate::kprintln;

const PCI_COMMAND_REGISTER: u32 = 0x04;
const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

#[repr(C, packed)]
pub struct PciDevice {
    vendor_id: u16,
    device_id: u16,
    class_code: u8,
    sub_class: u8,
    prog_info_byte: u8,
    header_type: u8,
    base_id: u32,
}

#[derive(Debug)]
pub enum PciDeviceInitError {
    NotImplemeted,
    InvalidBarType,
    InitializationFailure,
    TimeoutError,
    NoMSIXCapabilities,
    DmaAllocationFailure,
    XhciControllerStopTimeout,
    XhciControllerResetTimeout,
    XhciControllerNotReadyTimeout,
    XhciControllerStartTimeout,
    XhciCommandRingInitFailure,
    XhciMsixCapabilityNotFound,
    XhciMsiCapabilityNotFound,
    XhciNoSupportedInterruptMode,
    InsufficientMsixVectors,
    InsufficientMsiVectors,
    MsixTableBarInvalid,
    MsixPbaBarInvalid,
}

pub trait PciDeviceInitializer {
    fn initialize(pci_device: PciDevice) -> Result<(), PciDeviceInitError>;
}

impl PciDeviceInitializer for PciDevice {
    fn initialize(pci_device: PciDevice) -> Result<(), PciDeviceInitError> {
        Err(PciDeviceInitError::NotImplemeted)
    }
}
impl PciDevice {
    pub fn get_pci_id(pci_bus: u32, pci_device: u32, pci_function: u32) -> u32 {
        let val: u32 = (pci_bus << 16) | (pci_device << 11) | (pci_function << 8);
        val
    }

    pub fn new(
        vendor_id: u16,
        device_id: u16,
        class_code: u8,
        sub_class: u8,
        prog_info_byte: u8,
        header_type: u8,
        base_id: u32,
    ) -> Self {
        PciDevice {
            vendor_id,
            device_id,
            class_code,
            sub_class,
            prog_info_byte,
            header_type,
            base_id,
        }
    }

    /// Early init for PCI device to be able to use pci_read / write functions early
    pub fn new_empty(base_id: u32) -> Self {
        PciDevice {
            vendor_id: 0,
            device_id: 0,
            class_code: 0,
            sub_class: 0,
            prog_info_byte: 0,
            header_type: 0,
            base_id,
        }
    }

    pub fn print(&self) {
        let vendor_id = self.vendor_id;
        let device_id = self.device_id;
        let class_code = self.class_code;
        let sub_class = self.sub_class;
        let prog_if = self.prog_info_byte;
        let header_type = self.header_type;
        let base_id = self.base_id;

        kprintln!(Debug, "PCI Device Header:");
        kprintln!(Debug, "  Vendor ID   : 0x{:04x}", vendor_id);
        kprintln!(Debug, "  Device ID   : 0x{:04x}", device_id);
        kprintln!(Debug, "  Class Code  : 0x{:02x}", class_code);
        kprintln!(Debug, "  Subclass    : 0x{:02x}", sub_class);
        kprintln!(Debug, "  Prog IF     : 0x{:02x}", prog_if);
        kprintln!(Debug, "  Header Type : 0x{:02x}", header_type);
        kprintln!(Debug, "  Base ID : 0x{:08x}", base_id);
    }

    pub fn enable_pci_mmio_and_bus_mastering(&self) {
        let command = self.pci_read16(PCI_COMMAND_REGISTER);
        self.pci_write16(
            PCI_COMMAND_REGISTER,
            command | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER,
        );
    }

    pub fn prog_info_byte(&self) -> u8 {
        self.prog_info_byte
    }

    pub fn class_code(&self) -> u8 {
        self.class_code
    }

    pub fn sub_class(&self) -> u8 {
        self.sub_class
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    pub fn header_type(&self) -> u8 {
        self.header_type
    }

    pub fn base_id(&self) -> u32 {
        self.base_id
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        self.pci_read16(0x2C)
    }

    pub fn subsystem_device_id(&self) -> u16 {
        self.pci_read16(0x2E)
    }
}

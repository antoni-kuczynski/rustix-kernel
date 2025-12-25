/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */
use core::fmt::Error;
use crate::vgaprintln;

#[repr(C, packed)]
pub struct PciDeviceHeader {
    vendor_id: u16,
    device_id: u16,
    class_code: u8,
    sub_class: u8,
    prog_info_byte: u8,
    header_type: u8,
}

pub trait PciDeviceInitializer {
    fn initialize(&self) -> Result<(), Error>;
}


impl PciDeviceInitializer for PciDeviceHeader {
    fn initialize(&self) -> Result<(), Error> {
        Err(Error)
    }
}
impl PciDeviceHeader {
    pub fn get_pci_id(pci_bus: u32, pci_device: u32, pci_function: u32) -> u32 {
        let val: u32 = (pci_bus << 16) | (pci_device << 11) | (pci_function << 8);
        val
    }

    pub fn new(vendor_id: u16, device_id: u16, class_code: u8, sub_class: u8, prog_info_byte: u8, header_type: u8) -> Self {
        PciDeviceHeader {
            vendor_id,device_id,class_code,sub_class,prog_info_byte,header_type
        }
    }



    pub fn print(&self) {
        let vendor_id = self.vendor_id;
        let device_id = self.device_id;
        let class_code = self.class_code;
        let sub_class = self.sub_class;
        let prog_if = self.prog_info_byte;
        let header_type = self.header_type;

        vgaprintln!("PCI Device Header:");
        vgaprintln!("  Vendor ID   : 0x{:04x}", vendor_id);
        vgaprintln!("  Device ID   : 0x{:04x}", device_id);
        vgaprintln!("  Class Code  : 0x{:02x}", class_code);
        vgaprintln!("  Subclass    : 0x{:02x}", sub_class);
        vgaprintln!("  Prog IF     : 0x{:02x}", prog_if);
        vgaprintln!("  Header Type : 0x{:02x}", header_type);
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
}


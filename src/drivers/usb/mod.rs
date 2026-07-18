use crate::{kprintln};
use core::fmt::Error;
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitializer};
use crate::drivers::pci::pci_bar::PciBAR;
use crate::drivers::usb::xhci::xhci::XHCI;

pub mod uhci;
pub mod xhci;
pub mod ehci;

const PIF_UHCI_CONTROLLER: u8 = 0x00;
const PIF_OHCI_CONTROLLER: u8 = 0x10;
pub(crate) const PIF_EHCI_CONTROLLER: u8 = 0x20;
const PIF_XHCI_CONTROLLER: u8 = 0x30;

pub trait UsbControllerInitializer {
    fn initialize(&self) -> Result<(), Error>;
}

pub fn init_usb_controller(pci_dev: PciDevice) {
    let dev_id = pci_dev.device_id();
    match pci_dev.prog_info_byte() {
        PIF_UHCI_CONTROLLER => {
            kprintln!(Info, "Found UHCI controller with id {:#06x}.", dev_id);
            let bar = PciBAR::get(&pci_dev, 4);
            // bar.print();
        },
        PIF_OHCI_CONTROLLER => {
            kprintln!(Info, "Found OHCI controller with id {:#06x}.", dev_id);
        },
        PIF_EHCI_CONTROLLER => {
            kprintln!(Info, "Found EHCI controller with id {:#06x}.", dev_id);
            let bar = PciBAR::get(&pci_dev, 0);
            // bar.print();
        },
        PIF_XHCI_CONTROLLER => {
            kprintln!(Info, "Found XHCI controller with id {:#06x}.", dev_id);
            XHCI::initialize(pci_dev).expect("XHCI init failed!");
        },
        _ => todo!()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct UsbDeviceDescriptor {
    b_length: u8,
    b_descriptor_type: u8,
    bcd_usb: u16,
    b_device_class: u8,
    b_device_subclass: u8,
    b_device_protocol: u8,
    b_max_packet_size0: u8,
    id_vendor: u16,
    id_product: u16,
    bcd_device: u16,
    i_manufacturer: u8,
    i_product: u8,
    i_serial_number: u8,
    b_num_configurations: u8,
}

impl UsbDeviceDescriptor {
    pub fn b_length(&self) -> u8 {
        self.b_length
    }

    pub fn b_descriptor_type(&self) -> u8 {
        self.b_descriptor_type
    }

    pub fn bcd_usb(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.bcd_usb)) }
    }

    pub fn b_device_class(&self) -> u8 {
        self.b_device_class
    }

    pub fn b_device_subclass(&self) -> u8 {
        self.b_device_subclass
    }

    pub fn b_device_protocol(&self) -> u8 {
        self.b_device_protocol
    }

    pub fn b_max_packet_size0(&self) -> u8 {
        self.b_max_packet_size0
    }

    pub fn id_vendor(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.id_vendor)) }
    }

    pub fn id_product(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.id_product)) }
    }

    pub fn bcd_device(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.bcd_device)) }
    }

    pub fn i_manufacturer(&self) -> u8 {
        self.i_manufacturer
    }

    pub fn i_product(&self) -> u8 {
        self.i_product
    }

    pub fn i_serial_number(&self) -> u8 {
        self.i_serial_number
    }

    pub fn b_num_configurations(&self) -> u8 {
        self.b_num_configurations
    }
}

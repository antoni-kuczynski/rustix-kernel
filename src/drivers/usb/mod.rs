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
    id_vendor: UsbVendor,
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

    pub fn id_vendor(&self) -> UsbVendor {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbVendor(pub u16);

impl UsbVendor {
    pub const ADOMAX: Self = Self(0x0627);
    pub const QEMU: Self = Self(0x46F4);
    pub const VIRTUALBOX: Self = Self(0x80EE);
    pub const VMWARE: Self = Self(0x0E0F);

    pub const MICROSOFT: Self = Self(0x045E);
    pub const LOGITECH: Self = Self(0x046D);
    pub const APPLE: Self = Self(0x05AC);
    pub const RAZER: Self = Self(0x1532);
    pub const CORSAIR: Self = Self(0x1B1C);
    pub const STEELSERIES: Self = Self(0x1038);
    pub const ROCCAT: Self = Self(0x1E7D);
    pub const CHERRY: Self = Self(0x046A);
    pub const ASUS: Self = Self(0x0B05);
    pub const COOLER_MASTER: Self = Self(0x2516);
    pub const KEYCHRON: Self = Self(0x3434);
    pub const BISON_ELECTRONICS: Self = Self(0x5986);
    pub const SONIX_TECHNOLOGY: Self = Self(0x0C45);

    pub const DELL: Self = Self(0x413C);
    pub const HP: Self = Self(0x03F0);
    pub const LENOVO: Self = Self(0x17EF);
    pub const CHICONY: Self = Self(0x04F2);
    pub const LITEON: Self = Self(0x04CA);

    pub const INTEL: Self = Self(0x8086);
    pub const INTEL_ALT: Self = Self(0x8087);

    pub fn name(self) -> &'static str {
        match self {
            Self::ADOMAX => "Adomax Technology",
            Self::QEMU => "QEMU Virtual Device",
            Self::VIRTUALBOX => "VirtualBox",
            Self::VMWARE => "VMware",
            Self::MICROSOFT => "Microsoft",
            Self::LOGITECH => "Logitech",
            Self::APPLE => "Apple",
            Self::RAZER => "Razer USA",
            Self::CORSAIR => "Corsair",
            Self::STEELSERIES => "SteelSeries",
            Self::ROCCAT => "Roccat",
            Self::CHERRY => "Cherry GmbH",
            Self::ASUS => "ASUSTek",
            Self::COOLER_MASTER => "Cooler Master",
            Self::KEYCHRON => "Keychron",
            Self::BISON_ELECTRONICS => "Bison Electronics Inc.",
            Self::SONIX_TECHNOLOGY => "Sonix Technology Co., Ltd.",
            Self::DELL => "Dell Computer Corp.",
            Self::HP => "Hewlett-Packard",
            Self::LENOVO => "Lenovo",
            Self::CHICONY => "Chicony Electronics Co., Ltd.",
            Self::LITEON => "Lite-On Technology Corp.",
            Self::INTEL | Self::INTEL_ALT => "Intel Corporation",
            _ => "Unknown Vendor",
        }
    }
}

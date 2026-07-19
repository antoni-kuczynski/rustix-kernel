use crate::{kprintln};
use core::fmt::Error;
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitializer};
use crate::drivers::pci::pci_bar::PciBAR;
use crate::drivers::usb::xhci::xhci::XHCI;

pub mod uhci;
pub mod xhci;
pub mod ehci;
mod descriptors;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbId {
    pub vendor: u16,
    pub product: u16,
}

impl UsbId {
    pub const VID_ADOMAX: u16 = 0x0627;
    pub const VID_QEMU: u16 = 0x46F4;
    pub const VID_VIRTUALBOX: u16 = 0x80EE;
    pub const VID_VMWARE: u16 = 0x0E0F;

    pub const VID_MICROSOFT: u16 = 0x045E;
    pub const VID_LOGITECH: u16 = 0x046D;
    pub const VID_APPLE: u16 = 0x05AC;
    pub const VID_RAZER: u16 = 0x1532;
    pub const VID_CORSAIR: u16 = 0x1B1C;
    pub const VID_STEELSERIES: u16 = 0x1038;
    pub const VID_ROCCAT: u16 = 0x1E7D;
    pub const VID_A_FOUR_TECH: u16 = 0x09DA;
    pub const VID_CHERRY: u16 = 0x046A;
    pub const VID_ASUS: u16 = 0x0B05;
    pub const VID_COOLER_MASTER: u16 = 0x2516;
    pub const VID_KEYCHRON: u16 = 0x3434;
    pub const VID_BISON_ELECTRONICS: u16 = 0x5986;
    pub const VID_SONIX_TECHNOLOGY: u16 = 0x0C45;

    pub const VID_DELL: u16 = 0x413C;
    pub const VID_HP: u16 = 0x03F0;
    pub const VID_LENOVO: u16 = 0x17EF;
    pub const VID_CHICONY: u16 = 0x04F2;
    pub const VID_LITEON: u16 = 0x04CA;

    pub const VID_INTEL: u16 = 0x8086;
    pub const VID_INTEL_ALT: u16 = 0x8087;

    pub const fn new(vendor: u16, product: u16) -> Self {
        Self { vendor, product }
    }

    pub fn vendor_name(&self) -> &'static str {
        match self.vendor {
            Self::VID_ADOMAX => "Adomax Technology",
            Self::VID_QEMU => "QEMU Virtual Device",
            Self::VID_VIRTUALBOX => "VirtualBox",
            Self::VID_VMWARE => "VMware",
            Self::VID_MICROSOFT => "Microsoft",
            Self::VID_LOGITECH => "Logitech",
            Self::VID_APPLE => "Apple",
            Self::VID_RAZER => "Razer USA",
            Self::VID_CORSAIR => "Corsair",
            Self::VID_STEELSERIES => "SteelSeries",
            Self::VID_ROCCAT => "Roccat",
            Self::VID_A_FOUR_TECH => "A4Tech Co., Ltd",
            Self::VID_CHERRY => "Cherry GmbH",
            Self::VID_ASUS => "ASUSTek",
            Self::VID_COOLER_MASTER => "Cooler Master",
            Self::VID_KEYCHRON => "Keychron",
            Self::VID_BISON_ELECTRONICS => "Bison Electronics Inc.",
            Self::VID_SONIX_TECHNOLOGY => "Sonix Technology Co., Ltd.",
            Self::VID_DELL => "Dell Computer Corp.",
            Self::VID_HP => "Hewlett-Packard",
            Self::VID_LENOVO => "Lenovo",
            Self::VID_CHICONY => "Chicony Electronics Co., Ltd.",
            Self::VID_LITEON => "Lite-On Technology Corp.",
            Self::VID_INTEL | Self::VID_INTEL_ALT => "Intel Corporation",
            _ => "Unknown Vendor",
        }
    }

    //I only included the products i own - for easier debugging / clarity
    pub fn product_name(&self) -> &'static str {
        match (self.vendor, self.product) {
            (Self::VID_QEMU, 0x0001) => "QEMU USB Keyboard",
            (Self::VID_QEMU, 0x0002) => "QEMU USB Mouse",
            (Self::VID_QEMU, 0x0003) => "QEMU USB Tablet",

            (Self::VID_INTEL_ALT, 0x07dc) => "Bluetooth wireless interface",

            (Self::VID_LOGITECH, 0xc33a) => "G413 Gaming Keyboard",
            (Self::VID_LOGITECH, 0xc34a) => "G413 SE Gaming Keyboard",

            (Self::VID_RAZER, 0x006c) => "Mamba Elite (Wired)",

            (Self::VID_A_FOUR_TECH, 0x3263) => "Bloody A60 Mouse",

            (Self::VID_BISON_ELECTRONICS, 0x0268) => "SunplusIT INC. Integrated Camera",

            _ => "Unknown Device",
        }
    }
}

pub struct UsbBRequest;

impl UsbBRequest {
    pub const GET_STATUS: u8 = 0x00;
    pub const CLEAR_FEATURE: u8 = 0x01;
    pub const SET_FEATURE: u8 = 0x03;
    pub const SET_ADDRESS: u8 = 0x05;
    pub const GET_DESCRIPTOR: u8 = 0x06;
    pub const SET_DESCRIPTOR: u8 = 0x07;
    pub const GET_CONFIGURATION: u8 = 0x08;
    pub const SET_CONFIGURATION: u8 = 0x09;
    pub const GET_INTERFACE: u8 = 0x0A;
    pub const SET_INTERFACE: u8 = 0x0B;
    pub const SYNCH_FRAME: u8 = 0x0C;

    pub const HID_GET_REPORT: u8 = 0x01;
    pub const HID_GET_IDLE: u8 = 0x02;
    pub const HID_GET_PROTOCOL: u8 = 0x03;
    pub const HID_SET_REPORT: u8 = 0x09;
    pub const HID_SET_IDLE: u8 = 0x0A;
    pub const HID_SET_PROTOCOL: u8 = 0x0B;
}

pub struct UsbBmRequestType;

impl UsbBmRequestType {
    pub const DIR_HOST_TO_DEVICE: u8 = 0x00;
    pub const DIR_DEVICE_TO_HOST: u8 = 0x80;

    pub const TYPE_STANDARD: u8 = 0x00;
    pub const TYPE_CLASS: u8 = 0x20;
    pub const TYPE_VENDOR: u8 = 0x40;

    pub const REC_DEVICE: u8 = 0x00;
    pub const REC_INTERFACE: u8 = 0x01;
    pub const REC_ENDPOINT: u8 = 0x02;
    pub const REC_OTHER: u8 = 0x03;

    #[inline]
    pub const fn new(direction: u8, req_type: u8, recipient: u8) -> u8 {
        direction | req_type | recipient
    }
}

pub struct WValue;

impl WValue {
    pub const DESC_DEVICE: u8 = 0x01;
    pub const DESC_CONFIGURATION: u8 = 0x02;
    pub const DESC_STRING: u8 = 0x03;
    pub const DESC_INTERFACE: u8 = 0x04;
    pub const DESC_ENDPOINT: u8 = 0x05;
    pub const DESC_DEVICE_QUALIFIER: u8 = 0x06;
    pub const DESC_OTHER_SPEED: u8 = 0x07;

    pub const DESC_HID: u8 = 0x21;
    pub const DESC_HID_REPORT: u8 = 0x22;
    pub const DESC_HID_PHYSICAL: u8 = 0x23;

    pub const HID_REPORT_INPUT: u8 = 0x01;
    pub const HID_REPORT_OUTPUT: u8 = 0x02;
    pub const HID_REPORT_FEATURE: u8 = 0x03;

    pub const INDEX_ZERO: u8 = 0x00;

    pub const FEATURE_ENDPOINT_HALT: u16 = 0x0000;
    pub const FEATURE_DEVICE_REMOTE_WAKEUP: u16 = 0x0001;
    pub const FEATURE_TEST_MODE: u16 = 0x0002;

    #[inline]
    pub const fn new(high_byte: u8, low_byte: u8) -> u16 {
        ((high_byte as u16) << 8) | (low_byte as u16)
    }
}

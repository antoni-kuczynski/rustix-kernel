use crate::{vgaprint, VGAWRITER};
use crate::ColorTextMode;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Error;
use crate::drivers::pci::pci_device::PciDeviceHeader;
use crate::{print_fail_msg, vgaprintln};

pub mod uhci;


const PIF_UHCI_CONTROLLER: u8 = 0x00;
const PIF_OHCI_CONTROLLER: u8 = 0x10;
const PIF_EHCI_CONTROLLER: u8 = 0x20;
const PIF_XHCI_CONTROLLER: u8 = 0x30;

pub trait UsbControllerInitializer {
    fn initialize(&self) -> Result<(), Error>;
}

pub fn init_usb_controller(pci_dev: &PciDeviceHeader) {
    match pci_dev.prog_info_byte() {
        PIF_UHCI_CONTROLLER => {
            vgaprint!("Initializing UHCI...TODO\n");
        },
        PIF_OHCI_CONTROLLER => {
            vgaprint!("Initializing OHCI...TODO\n");
        },
        PIF_EHCI_CONTROLLER => {
            vgaprint!("Initializing EHCI...TODO\n");
        },
        PIF_XHCI_CONTROLLER => {
            vgaprint!("Initializing XHCI...TODO\n");
        },
        _ => todo!()
    }
}
use crate::drivers::pci::pci_bar::PciBAR;
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitializer};
use crate::drivers::usb::xhci::xhci::XHCI;
use crate::vgaprint;
use core::fmt::Error;

pub mod ehci;
pub mod uhci;
pub mod xhci;

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
            let bar = PciBAR::get(&pci_dev, 4);
            // bar.print();
        }
        PIF_OHCI_CONTROLLER => {
            vgaprint!("Initializing OHCI...TODO\n");
        }
        PIF_EHCI_CONTROLLER => {
            vgaprint!("Initializing EHCI...TODO\n");
            let bar = PciBAR::get(&pci_dev, 0);
            // bar.print();
        }
        PIF_XHCI_CONTROLLER => {
            vgaprint!("Initializing XHCI...TODO\n");
            XHCI::initialize(pci_dev).expect("XHCI init failed");
        }
        _ => todo!(),
    }
}

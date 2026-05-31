use crate::{kprintln};
use core::fmt::Error;
use crate::drivers::pci::pci_device::PciDeviceHeader;
use crate::drivers::pci::pci_bar::PciBAR;

pub mod uhci;
pub mod xhci;
pub mod ehci;

const PIF_UHCI_CONTROLLER: u8 = 0x00;
const PIF_OHCI_CONTROLLER: u8 = 0x10;
const PIF_EHCI_CONTROLLER: u8 = 0x20;
const PIF_XHCI_CONTROLLER: u8 = 0x30;

pub trait UsbControllerInitializer {
    fn initialize(&self) -> Result<(), Error>;
}

pub fn init_usb_controller(pci_dev: &PciDeviceHeader) {
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
        },
        _ => todo!()
    }
}
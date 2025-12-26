use crate::{VGAWRITER};
use crate::vgaprintln;
use crate::ColorTextMode;
use crate::{print_fail_msg, print_ok_msg, vgaprint};
use core::fmt::Error;
use bootloader::BootInfo;
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::usb::ehci::EHCI;
use crate::drivers::usb::uhci::UHCI;

pub mod uhci;
mod ehci;

const PIF_UHCI_CONTROLLER: u8 = 0x00;
const PIF_OHCI_CONTROLLER: u8 = 0x10;
const PIF_EHCI_CONTROLLER: u8 = 0x20;
const PIF_XHCI_CONTROLLER: u8 = 0x30;

#[allow(dead_code)]
pub trait UsbControllerInitializer {
    fn initialize(&self) -> Result<(), Error>;
}

pub fn init_usb_controller(pci_dev: &PciDeviceHeader, boot_info: &BootInfo) {
    match pci_dev.prog_info_byte() {
        PIF_UHCI_CONTROLLER => {
            vgaprint!("Initializing UHCI...");
            match UHCI::initialize(&pci_dev, &boot_info) {
                Ok(_) => {
                    print_ok_msg!();
                },
                Err(_e) => {
                    print_fail_msg!();
                }
            }
        },
        PIF_OHCI_CONTROLLER => {
            vgaprint!("Initializing OHCI...TODO\n");
        },
        PIF_EHCI_CONTROLLER => {
            vgaprint!("Initializing EHCI...");
            match EHCI::initialize(&pci_dev, &boot_info) {
                Ok(_) => {
                    print_ok_msg!();
                }
                Err(e) => {
                    print_fail_msg!();
                    vgaprintln!("{:?}", e)
                }
            }

        },
        PIF_XHCI_CONTROLLER => {
            vgaprint!("Initializing XHCI...TODO\n");
        },
        _ => todo!()
    }
}
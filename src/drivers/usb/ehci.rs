use alloc::boxed::Box;
use core::ptr;
use bootloader::BootInfo;
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_device::PciDeviceInitError::{InitializationFailure, InvalidBarType};
use crate::drivers::pci::pci_io::{pci_read32, pci_write32};
use crate::interrupts::hardware::pic8259::{get_current_time_millis, get_ticks};
use crate::vgaprintln;

pub struct EHCI {
    header: PciDeviceHeader,
    frame_list: Box<[u32; 1024]>
}

struct EhciExtendedCapabilitiesInitError();

//REGS
const EECP_USB_LEGSUP_REG: u32 = 0x00;

const HCCPARAMS_REG: u64 = 0x08;

const USBSTS_REG: u64 = 0x04;
const USBINTR_REG: u64 = 0x08;
const FRINDEX_REG: u64 = 0x0C;

//MASKS
const EECP_MASK: u32 = 0xFF << 8; //bits 15:8
const LEGSUP_HC_BIOS_OWNED_SEMAPHORE_MASK: u32 = 0x01 << 16;
const LEGSUP_HC_OS_OWNED_SEMAPHORE_MASK: u32 = 0x01 << 24;

fn handle_extended_capabilities(eecp: u32, base_id: u32) -> Result<u32, EhciExtendedCapabilitiesInitError> {
    let mut legsup = pci_read32(base_id, eecp + EECP_USB_LEGSUP_REG);

    if legsup & LEGSUP_HC_BIOS_OWNED_SEMAPHORE_MASK != 0 {
        //request ownership of EHCI controller
        pci_write32(
            base_id, eecp + EECP_USB_LEGSUP_REG,
        legsup | LEGSUP_HC_OS_OWNED_SEMAPHORE_MASK
        );

        let time = get_current_time_millis();
        let mut current_time = time;
        while current_time - time <= 50 {
            current_time = get_current_time_millis();
            legsup = pci_read32(base_id, eecp + EECP_USB_LEGSUP_REG);

            if legsup & LEGSUP_HC_BIOS_OWNED_SEMAPHORE_MASK == 0 {
                return Ok(legsup);
            }
        }
    }
    Ok(0)
}

impl PciDeviceInitializer for EHCI {
    fn initialize(pci_device: &PciDeviceHeader, boot_info: &BootInfo) -> Result<(), PciDeviceInitError> {
        let bar = PciBAR::get(pci_device, 0);

        if bar.bar_type() == &BarType::Io {
            return Err(InvalidBarType);
        }

        unsafe {
            let base = bar.base_address() + boot_info.physical_memory_offset;
            let eecp =
                (ptr::read_volatile((base + HCCPARAMS_REG) as *const u32) &
                EECP_MASK) >> 8;

            if eecp >= 0x40 {
                let a = handle_extended_capabilities(
                    eecp, pci_device.base_id()
                );

                match a {
                    Ok(x) => {
                        if x == 0 {
                            vgaprintln!("BIOS didnt own the EHCI");
                        }
                    }
                    Err(e) => {
                        return Err(InitializationFailure)
                    }
                }
            }


            //clear status
            ptr::write_volatile(
                (base + USBSTS_REG) as *mut u32,
                0x3F
            );

            //disable interrupts
            ptr::write_volatile(
                (base + USBINTR_REG) as *mut u32,
                0x00
            );

            //set frame index
            ptr::write_volatile(
                (base + FRINDEX_REG) as *mut u32,
                0x00
            );

            
            //for now leaving this unfinished as is, as i dont have a machine with enabled EHCI right now
            // (only XHCI) and cant really test it lol


        }

        Ok(())



    }
}
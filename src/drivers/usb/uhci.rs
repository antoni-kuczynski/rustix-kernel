/*
 * Created by Antoni Kuczyński
 * 25/12/2025
 */
use crate::asm::{outb, outl, outw};
use crate::drivers::apic::apic::timer_lapic_sleep;
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitError, PciDeviceInitializer};
use alloc::boxed::Box;
use x86_64::PhysAddr;
//================================================================================
//This controller is only initialized for compatibility reasons (PS/2 8042 controller)
//this is not finished at all
//================================================================================
#[allow(dead_code)]
pub struct UHCI {
    pci_header: PciDevice,
    frame_list: Box<[u32; 1024]>,
}

const UHCI_USB_LEGACY_SUPPORT: u64 = 0xC0;
const UHCI_INTERRUPT_ENABLE_REG: u64 = 0x04;
const UHCI_FRNUM_REG: u64 = 0x06;
const UHCI_FRBASEADD: u64 = 0x08;
const UHCI_USB_CMD: u64 = 0x00;

fn io_port(base: PhysAddr, offset: u64) -> u16 {
    (base.as_u64() + offset) as u16
}

impl PciDeviceInitializer for UHCI {
    fn initialize(pci_device: &PciDevice) -> Result<(), PciDeviceInitError> {
        let pci_bar = PciBAR::get(&pci_device, 4);

        if pci_bar.bar_type() != &BarType::Io {
            //only io is supported
            return Err(PciDeviceInitError::InvalidBarType);
        }

        let mut frame_list = Box::new([0u32; 1024]);

        let io_address = pci_bar.base_address();
        unsafe {
            outw(io_port(io_address, UHCI_USB_LEGACY_SUPPORT), 0x2000); //disable legacy usb support
            outw(io_port(io_address, UHCI_INTERRUPT_ENABLE_REG), 0x0000); //disable interrupts

            //global reset & host controller reset
            outb(io_port(io_address, UHCI_USB_CMD), 0b00000110);
            timer_lapic_sleep(10);
            outb(io_port(io_address, UHCI_USB_CMD), 0x00);

            outw(io_port(io_address, UHCI_FRNUM_REG), 0x0000); //reset frame number
            outl(
                io_port(io_address, UHCI_FRBASEADD),
                *frame_list.as_mut_ptr(),
            ); //set frame list address

            outb(io_port(io_address, UHCI_USB_CMD), 0b10000001); //enable controller
        }

        Ok(())
    }
}

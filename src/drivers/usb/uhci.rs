/*
 * Created by Antoni Kuczyński
 * 25/12/2025
 */
use crate::interrupts::hardware::pic8259::{sleep};
use alloc::boxed::Box;
use bootloader::BootInfo;
use crate::asm::{outb, outl, outw};
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};



//================================================================================
//This controller is only initialized for compatibility reasons (PS/2 8042 controller)
//this is not finished at all
//================================================================================
#[allow(dead_code)]
pub struct UHCI {
    pci_header: PciDeviceHeader,
    frame_list: Box<[u32; 1024]>
}

const UHCI_USB_LEGACY_SUPPORT: u64 = 0xC0;
const UHCI_INTERRUPT_ENABLE_REG: u64 = 0x04;
const UHCI_FRNUM_REG: u64 = 0x06;
const UHCI_FRBASEADD: u64 = 0x08;
const UHCI_USB_CMD: u64 = 0x00;

impl PciDeviceInitializer for UHCI {
    fn initialize(pci_device: &PciDeviceHeader, boot_info: &BootInfo) -> Result<(), PciDeviceInitError> {
        let pci_bar = PciBAR::get(&pci_device, 4);

        if pci_bar.bar_type() != &BarType::Io {
            //only io is supported
            return Err(PciDeviceInitError::InvalidBarIoType);
        }

        let mut frame_list = Box::new([0u32; 1024]);

        let io_addres = pci_bar.base_address();
        unsafe {
            outw((io_addres + UHCI_USB_LEGACY_SUPPORT) as u16, 0x2000); //disable legacy usb support
            outw((io_addres + UHCI_INTERRUPT_ENABLE_REG) as u16, 0x0000); //disable interrupts

            //global reset & host controller reset
            outb((io_addres + UHCI_USB_CMD) as u16, 0b00000110);
            sleep(10);
            outb((io_addres + UHCI_USB_CMD) as u16, 0x00);


            outw((io_addres + UHCI_FRNUM_REG) as u16, 0x0000); //reset frame number
            outl((io_addres + UHCI_FRBASEADD) as u16, *frame_list.as_mut_ptr()); //set frame list address

            outb((io_addres + UHCI_USB_CMD) as u16, 0b10000001); //enable controller
        }


        Ok(())
    }
}
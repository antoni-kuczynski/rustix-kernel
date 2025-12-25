use core::fmt::Error;
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitializer};

pub struct UHCI {
    pci_header: PciDeviceHeader
}

impl PciDeviceInitializer for UHCI {
    fn initialize(&self) -> Result<(), Error> {
        todo!()
    }
}
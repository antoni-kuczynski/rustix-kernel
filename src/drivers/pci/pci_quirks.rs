#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 19/05/2026
 */
use crate::drivers::pci::pci::for_each_pci_device;
use crate::drivers::pci::pci_device::PciDevice;
use crate::drivers::pci::pci_io::{PciVendor};
use crate::drivers::usb::PIF_EHCI_CONTROLLER;
use crate::kprintln;

const USB_INTEL_XUSB2PR: u32 = 0xD0;
const USB_INTEL_USB2PRM: u32 = 0xD4;
const USB_INTEL_USB3_PSSEN: u32 = 0xD8;
const USB_INTEL_USB3PRM: u32 = 0xDC;


impl PciDevice {
    // solution from the linux kernel
    /// Fix for Intel Panther Point chipsets - these chipsets have both XHCI and EHCI installed
    /// that share some ports. This switches the ports to XHCI.
    pub fn usb_intel_enable_xhci_ports(&self) {
        //according to linux source code, old Sony VAIO t-series laptops cant switch ports to xhci
        if self.subsystem_vendor_id() == PciVendor::SONY && self.subsystem_device_id() == 0x90A8 {
            return;
        }

        //if system doesnt have ehci dont switch the ports
        let mut has_ehci = false;
        for_each_pci_device(|dev| {
            if dev.prog_info_byte() == PIF_EHCI_CONTROLLER {
                has_ehci = true;
                return;
            }
        });

        if !has_ehci {
            return;
        }

        //get ports that can be changed to XHCI (port routing mask)
        //turn on super speed for these ports
        let available_ports = self.pci_read32(USB_INTEL_USB3PRM);
        self.pci_write32(USB_INTEL_USB3_PSSEN, available_ports);

        let switched_ports = self.pci_read32(USB_INTEL_USB3_PSSEN);
        kprintln!(Info,"USB 3.0 ports enabled under XHCI: {}", switched_ports);

        // set usb2 ports to be controller by xhci (usb 2 port routing mask)
        let ports_usb2 = self.pci_read32(USB_INTEL_USB2PRM);
        self.pci_write32(USB_INTEL_XUSB2PR, ports_usb2);
        let switched_ports_usb2 = self.pci_read32(USB_INTEL_XUSB2PR);
        kprintln!(Info,"USB 2.0 ports enabled under XHCI: {}", switched_ports_usb2);
    }
}

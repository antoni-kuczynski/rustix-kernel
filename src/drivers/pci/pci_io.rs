/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */

use crate::asm::*;

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[inline(always)]
fn pci_addr(id: u32, reg: u32) -> u32 {
    0x8000_0000 | id | (reg & 0xfc)
}

pub fn pci_read8(id: u32, reg: u32) -> u8 {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        inb(PCI_CONFIG_DATA + (reg & 0x03) as u16)
    }
}

pub fn pci_read16(id: u32, reg: u32) -> u16 {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        inw(PCI_CONFIG_DATA + (reg & 0x02) as u16)
    }
}

pub fn pci_read32(id: u32, reg: u32) -> u32 {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        inl(PCI_CONFIG_DATA)
    }
}

pub fn pci_write8(id: u32, reg: u32, data: u8) {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        outb(PCI_CONFIG_DATA + (reg & 0x03) as u16, data);
    }
}

pub fn pci_write16(id: u32, reg: u32, data: u16) {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        outw(PCI_CONFIG_DATA + (reg & 0x02) as u16, data);
    }
}

pub fn pci_write32(id: u32, reg: u32, data: u32) {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        outl(PCI_CONFIG_DATA, data);
    }
}

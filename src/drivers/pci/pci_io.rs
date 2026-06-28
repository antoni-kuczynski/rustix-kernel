/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */
use core::ops::Add;
use crate::asm::*;
use crate::drivers::pci::pci::PCI_MMIO_ALLOCS;

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[inline(always)]
fn pci_addr(id: u32, reg: u32) -> u32 {
    0x8000_0000 | id | (reg & 0xfc)
}

pub fn pci_read8(id: u32, reg: u32) -> u8 {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::read_volatile(ptr) }
    } else {
        if reg >= 256 {
            return 0xFF;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            inb(PCI_CONFIG_DATA + (reg & 0x03) as u16)
        }
    }
}

pub fn pci_read16(id: u32, reg: u32) -> u16 {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::read_volatile(ptr as *const u16) }
    } else {
        if reg >= 256 {
            return 0xFF;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            inw(PCI_CONFIG_DATA + (reg & 0x02) as u16)
        }
    }
}

pub fn pci_read32(id: u32, reg: u32) -> u32 {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::read_volatile(ptr as *const u32) }
    } else {
        if reg >= 256 {
            return 0xFF;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            inl(PCI_CONFIG_DATA)
        }
    }
}

pub fn pci_write8(id: u32, reg: u32, data: u8) {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::write_volatile(ptr, data) }
    } else {
        if reg >= 256 {
            return;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            outb(PCI_CONFIG_DATA + (reg & 0x03) as u16, data);
        }
    }
}

pub fn pci_write16(id: u32, reg: u32, data: u16) {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::write_volatile(ptr as *mut u16, data) }
    } else {
        if reg >= 256 {
            return;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            outw(PCI_CONFIG_DATA + (reg & 0x02) as u16, data);
        }
    }
}

pub fn pci_write32(id: u32, reg: u32, data: u32) {
    if let Some(ptr) = get_mmio_addr(id, reg) {
        unsafe { core::ptr::write_volatile(ptr as *mut u32, data) }
    } else {
        if reg >= 256 {
            return;
        } //legacy I/O doesnt support extended registers
        unsafe {
            outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
            outl(PCI_CONFIG_DATA, data);
        }
    }
}

fn get_mmio_addr(id: u32, reg: u32) -> Option<*mut u8> {
    let bus = ((id >> 16) & 0xFF) as u8;
    let device = ((id >> 11) & 0x1F) as u8;
    let function = ((id >> 8) & 0x07) as u8;

    if let Some(allocs) = PCI_MMIO_ALLOCS.get() {
        for info in allocs {
            if info.mcfg_alloc.pci_segment_group == 0
                && bus >= info.mcfg_alloc.start_bus_number
                && bus <= info.mcfg_alloc.end_bus_number
            {
                let bus_offset = ((bus - info.mcfg_alloc.start_bus_number) as usize) << 20;
                let dev_offset = (device as usize) << 15;
                let func_offset = (function as usize) << 12;
                let reg_offset = (reg & 0xFFF) as usize;

                let offset = bus_offset | dev_offset | func_offset | reg_offset;
                let vaddr = info.io_alloc.virt_addr.add(offset as u64);

                return Some(vaddr.as_mut_ptr());
            }
        }
    }
    None
}

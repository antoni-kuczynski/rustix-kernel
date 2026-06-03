/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */

use crate::asm::*;

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn pci_write8(id: u32, reg: u32, data: u8) {
    unsafe {
        outl(PCI_CONFIG_ADDR, pci_addr(id, reg));
        outb(PCI_CONFIG_DATA + (reg & 0x03) as u16, data);
    }
}

#[allow(dead_code)]
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

pub struct PciVendor;

impl PciVendor {
    pub const INTEL: u16 = 0x8086;
    pub const AMD: u16 = 0x1022;
    pub const NVIDIA: u16 = 0x10DE;
    pub const ATI_AMD_GPU: u16 = 0x1002;
    pub const SONY: u16 = 0x104D;
    pub const REALTEK: u16 = 0x10EC;
    pub const BROADCOM: u16 = 0x14E4;
    pub const QUALCOMM_ATHEROS: u16 = 0x168C;
    pub const MARVELL: u16 = 0x11AB;
    pub const VIA: u16 = 0x1106;
    pub const VMWARE: u16 = 0x15AD;
    pub const VIRTIO: u16 = 0x1AF4;
    pub const RED_HAT_QUMRANET: u16 = 0x1B36;
    pub const QEMU: u16 = 0x1234;
    pub const BOCHS: u16 = 0x1234;
    pub const VIRTUALBOX: u16 = 0x80EE;
    pub const MICROSOFT: u16 = 0x1414;
    pub const APPLE: u16 = 0x106B;
    pub const ASMEDIA: u16 = 0x1B21;
    pub const NEC_RENESAS: u16 = 0x1033;
    pub const TEXAS_INSTRUMENTS: u16 = 0x104C;
    pub const CREATIVE: u16 = 0x1102;
    pub const ENSONIQ: u16 = 0x1274;
    pub const CIRRUS_LOGIC: u16 = 0x1013;
    pub const MATROX: u16 = 0x102B;
    pub const LSI_BROADCOM: u16 = 0x1000;
    pub const ADAPTEC: u16 = 0x9005;
    pub const SILICON_IMAGE: u16 = 0x1095;
    pub const JMICRON: u16 = 0x197B;
    pub const MEDIATEK: u16 = 0x14C3;
    pub const RALINK: u16 = 0x1814;
    pub const CHELSIO: u16 = 0x1425;
    pub const MELLANOX_NVIDIA: u16 = 0x15B3;
    pub const AQUANTIA: u16 = 0x1D6A;
    pub const AMAZON_ANNAPURNA: u16 = 0x1D0F;
    pub const GOOGLE: u16 = 0x1AE0;
    pub const HUAWEI: u16 = 0x19E5;
    pub const XILINX: u16 = 0x10EE;
    pub const ALTERA_INTEL_FPGA: u16 = 0x1172;
    pub const SYNOPSYS: u16 = 0x16C3;
    pub const CADENCE: u16 = 0x17CD;
}

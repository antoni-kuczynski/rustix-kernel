/*
 * Created by Antoni Kuczyński
 * 25/12/2025
 */
use crate::drivers::pci::pci_device::PciDeviceHeader;
use crate::drivers::pci::pci_io::{pci_read32, pci_write32};
use crate::__vgaprintln;

#[derive(PartialEq, Eq)]

pub struct PciBAR {
    base_address: u64,
    size: u64,
    bar_type: BarType,
    prefetchable: bool
}

struct BarInfo {
    address: u32,
    mask: u32
}

#[derive(PartialEq, Eq)]
pub enum BarType {
    Mmio32,
    Mmio64,
    Io,
}

//BAR OFFSET
const BAR0_OFFSET: u8 = 0x10;

//BAR flags
const PCI_BAR_IO: u32 = 0x01;
const PCI_BAR_PREFETCH: u32 = 0x08;

//MASKS
const PCI_BAR_MEM_TYPE_MASK: u32 = 0x06;
const PCI_BAR_MEM_TYPE_64: u32 = 0x04;

impl BarInfo {
    fn get(bar_index: u8, base_id: u32) -> BarInfo {
        let bar_offset: u32 = (BAR0_OFFSET + (bar_index * 4)) as u32;

        let address = pci_read32(base_id, bar_offset);

        //get bitmask and get BARs size
        pci_write32(base_id, bar_offset, 0xFFFF_FFFF);
        let mask = pci_read32(base_id, bar_offset);

        pci_write32(base_id, bar_offset, address);

        BarInfo {
            address, mask
        }
    }
}

/*
(from osdev wiki)
 Memory Space BAR Layout Bits 31-4 	Bit 3 	Bits 2-1 	Bit 0
16-Byte Aligned Base Address 	Prefetchable 	Type 	Always 0

I/O Space BAR Layout Bits 31-2 	Bit 1 	Bit 0
4-Byte Aligned Base Address 	Reserved 	Always 1
 */

#[allow(dead_code)]
impl PciBAR {
    pub fn get(device: &PciDeviceHeader, bar_index: u8) -> Self {
        let bar = BarInfo::get(bar_index, device.base_id());
        let address_low = bar.address;
        let mask_low = bar.mask;


        if (address_low & PCI_BAR_IO) != 0 {
            let base = (address_low & !0x3) as u64;
            let size = (!(mask_low & !0x3) + 1) as u64;

            return PciBAR {
                base_address: base,
                size,
                bar_type: BarType::Io,
                prefetchable: false,
            };
        }

        //memory BAR
        let prefetchable = (address_low & PCI_BAR_PREFETCH) != 0;
        let mem_type = address_low & PCI_BAR_MEM_TYPE_MASK;

        //64bit MMIO
        if mem_type == PCI_BAR_MEM_TYPE_64 {
            let bar_high = BarInfo::get(bar_index + 1, device.base_id());

            let base =
                ((bar_high.address as u64) << 32) |
                    ((address_low & !0xF) as u64);

            let size =
                !(((bar_high.mask as u64) << 32) |
                    ((mask_low & !0xF) as u64)) + 1;

            return PciBAR {
                base_address: base,
                size,
                bar_type: BarType::Mmio64,
                prefetchable,
            };
        }

        //32bit MMIO
        let base = (address_low & !0xF) as u64;
        let size = (!(mask_low & !0xF) + 1) as u64;

        PciBAR {
            base_address: base,
            size,
            bar_type: BarType::Mmio32,
            prefetchable,
        }
    }

    pub fn print(&self) {
        let bar_type_str = match self.bar_type {
            BarType::Mmio32 => "MMIO (32-bit)",
            BarType::Mmio64 => "MMIO (64-bit)",
            BarType::Io => "I/O",
        };

        __vgaprintln!("PCI BAR:");
        __vgaprintln!("  Type         : {}", bar_type_str);
        __vgaprintln!("  Base Address : 0x{:016x}", self.base_address);
        __vgaprintln!("  Size         : 0x{:x} ({} bytes)", self.size, self.size);
        __vgaprintln!("  Prefetchable : {}", self.prefetchable);
    }

    pub fn base_address(&self) -> u64 {
        self.base_address
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn bar_type(&self) -> &BarType {
        &self.bar_type
    }

    pub fn prefetchable(&self) -> bool {
        self.prefetchable
    }
}
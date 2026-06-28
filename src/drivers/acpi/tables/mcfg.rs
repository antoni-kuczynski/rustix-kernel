/*
 * Created by Antoni Kuczyński
 * 26/06/2026
 */
use core::ptr::slice_from_raw_parts;
use core::slice;
use x86_64::VirtAddr;
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;

#[repr(C, packed)]
pub struct MCFG {
    pub header: ACPISDTHeader,
    pub __reserved: u64,
}

#[repr(C, packed)]
pub struct McfgAllocation {
    pub base_address: u64,
    pub pci_segment_group: u16,
    pub start_bus_number: u8,
    pub end_bus_number: u8,
    pub __reserved: u32,
}

impl AcpiSdtTable for MCFG {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::MCFG
    }

    fn validate(&self) -> bool {
        self.header.validate_checksum()
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

impl MCFG {
    pub fn new_from_ptr(ptr: VirtAddr) -> &'static MCFG {
        let header = ACPISDTHeader::new_from_virt_addr(ptr);
        let length = header.length as usize;
        let rsdt_ptr =
            slice_from_raw_parts(ptr.as_ptr::<u8>(), (length - size_of_val(&header)) >> 2);

        unsafe { &*(rsdt_ptr as *const MCFG) }
    }

    pub fn allocations(&self) -> &'static [McfgAllocation] {
        let header_and_reserved_size = 44;
        let allocations_byte_size = self.header.length as usize - header_and_reserved_size;
        let allocations_count = allocations_byte_size / size_of::<McfgAllocation>();

        unsafe {
            let ptr = (self as *const MCFG as *const u8).add(header_and_reserved_size) as *const McfgAllocation;
            slice::from_raw_parts(ptr, allocations_count)
        }
    }
}
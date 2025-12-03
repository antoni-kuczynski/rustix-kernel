/*
 * Created by Antoni Kuczyński
 * 05/11/2025
 */
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::slice_from_raw_parts;
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};

// ============================================================
//               **XSDT & RSDT**
//  The RSDT is used on ACPI version 1.0,
//  XSDT is used on ACPI version 2.0+
// ============================================================
#[repr(C, packed)]
pub struct RSDT {
    pub header: ACPISDTHeader,
    pub other_sdt_pointers: [u32]
}

impl AcpiSdtTable for RSDT {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::RSDT
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

impl RSDT {
    pub fn new_from_ptr(ptr: u64) -> &'static RSDT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let rsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                (length - size_of_val(&header)) >> 2,
            );

            &*(rsdt_ptr as *const RSDT)
        }
    }

    pub fn get_mapping_length(&self) -> usize {
        let header: ACPISDTHeader = self.header;
        let length = self.header.length;
        (length as usize - size_of_val(&header)) >> 2
    }

    pub fn get_pointers_to_other_sdts(&self) -> Vec<u64> {
        let mut a = vec![0; self.get_mapping_length()];
        for i in 0..self.get_mapping_length() {
            a[i] = self.other_sdt_pointers[i] as u64;
        }
        a
    }
}

#[repr(C, packed)]
pub struct XSDT {
    pub header: ACPISDTHeader,
    pub other_sdt_pointers: [u64]
}

impl AcpiSdtTable for XSDT {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::XSDT
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

impl XSDT {
    pub(crate) fn new(ptr: u64) -> &'static XSDT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let xsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                (length - size_of_val(&header)) >> 3,
            );

            &*(xsdt_ptr as *const XSDT)
        }
    }

    pub fn get_mapping_length(&self) -> usize {
        let header: ACPISDTHeader = self.header;
        let length = self.header.length;
        (length as usize - size_of_val(&header)) >> 3
    }

    pub fn get_pointers_to_other_sdts(&self) -> Vec<u64> {
        let mut sdt_ptrs = vec![0; self.get_mapping_length()];
        for i in 0..self.get_mapping_length() {
            sdt_ptrs[i] = self.other_sdt_pointers[i];
        }
        sdt_ptrs
    }
}
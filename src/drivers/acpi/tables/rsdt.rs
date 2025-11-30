/*
 * Created by Antoni Kuczyński
 * 05/11/2025
 */
use alloc::string::String;
use core::ptr::slice_from_raw_parts;
use crate::drivers::acpi::acpi_sdt::ACPISDTHeader;
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::vgaprintln;

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
    pub(crate) fn new_from_ptr(ptr: u64) -> &'static RSDT {
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

    pub fn print(&self) {
        let header: ACPISDTHeader = self.header;
        let signature = header.signature;
        let length = header.length;
        let revision = header.revision;
        let checksum = header.checksum;
        let oem_id = header.oem_id;
        let oem_table_id = header.oem_table_id;
        let oem_revision = header.oem_revision;
        let creator_id = header.creator_id;
        let creator_revision = header.creator_revision;

        vgaprintln!("RSDT:");
        vgaprintln!("  Signature: {:?}", signature);
        vgaprintln!("  Length:    {}", length);
        vgaprintln!("  Revision:  {}", revision);
        vgaprintln!("  Checksum:  {}", checksum);
        vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        vgaprintln!("  OEM Table ID: {:?}", String::from_utf8_lossy(&oem_table_id));
        vgaprintln!("  OEM Revision: {}", oem_revision);
        vgaprintln!("  Creator ID:   {:?}", creator_id);
        vgaprintln!("  Creator Rev:  {}", creator_revision);

        // let ptrs: [u32] = self.other_sdt_pointers;
        for i in 0..self.get_mapping_length() {
            let addr = self.other_sdt_pointers[i];
            vgaprintln!("    [{}] 0x{:08X}", i, addr);
        }
    }
}


#[repr(C, packed)]
pub struct XSDT {
    pub header: ACPISDTHeader,
    pub other_sdt_pointers: [u64]
}

impl XSDT {
    pub(crate) fn new_from_ptr(ptr: u64) -> &'static XSDT {
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

    pub fn print(&self) {
        let header: ACPISDTHeader = self.header;
        let signature = header.signature;
        let length = header.length;
        let revision = header.revision;
        let checksum = header.checksum;
        let oem_id = header.oem_id;
        let oem_table_id = header.oem_table_id;
        let oem_revision = header.oem_revision;
        let creator_id = header.creator_id;
        let creator_revision = header.creator_revision;

        vgaprintln!("XSDT:");
        vgaprintln!("  Signature: {:?}", signature);
        vgaprintln!("  Length:    {}", length);
        vgaprintln!("  Revision:  {}", revision);
        vgaprintln!("  Checksum:  {}", checksum);
        vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        vgaprintln!("  OEM Table ID: {:?}", String::from_utf8_lossy(&oem_table_id));
        vgaprintln!("  OEM Revision: {}", oem_revision);
        vgaprintln!("  Creator ID:   {:?}", creator_id);
        vgaprintln!("  Creator Rev:  {}", creator_revision);

        for i in 0..((length as usize - size_of_val(&header)) >> 3) {
            let addr = self.other_sdt_pointers[i];
            vgaprintln!("    [{}] 0x{:08X}", i, addr);
        }
    }
}
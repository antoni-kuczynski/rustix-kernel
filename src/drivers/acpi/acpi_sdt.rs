/*
 * Created by Antoni Kuczyński
 * 03/11/2025
 */
use crate::drivers::acpi::acpi_tables::ACPISignature;

// ============================================================
//
//               **SDT HEADER**
//  Shared by all ACPI SDT types
// ============================================================
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ACPISDTHeader {
    pub signature: ACPISignature,
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

impl ACPISDTHeader {
    fn new_from_ptr_u32(ptr: u32) -> &'static ACPISDTHeader {
        unsafe {
            &*(ptr as *const ACPISDTHeader)
        }
    }

    pub(crate) fn new_from_ptr_u64(ptr: u64) -> &'static ACPISDTHeader {
        unsafe {
            &*(ptr as *const ACPISDTHeader)
        }
    }
    pub(crate) fn validate_checksum(&self) -> bool {
        unsafe {
            let ptr = self as *const _ as *const u8;
            let mut sum: u8 = 0;
            let len = self.length as usize;
            for i in 0..len {
                sum = sum.wrapping_add(*ptr.add(i));
            }
            sum == 0
        }
    }
}
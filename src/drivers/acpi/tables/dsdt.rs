use core::ptr::slice_from_raw_parts;
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::drivers::acpi::tables::fadt::FADT;
use crate::drivers::acpi::tables::rsdt::RSDT;
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;

pub struct DSDT {
    header: ACPISDTHeader
}

impl AcpiSdtTable for DSDT {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::DSDT
    }

    fn validate(&self) -> bool {
        self.get_sdt_header().validate_checksum()
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

impl DSDT {
    pub fn new_from_ptr(ptr: u64) -> &'static DSDT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let rsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                (length - size_of_val(&header)) >> 2,
            );

            &*(rsdt_ptr as *const DSDT)
        }
    }
}
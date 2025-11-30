/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Error;
use bootloader::BootInfo;
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};
use crate::drivers::acpi::acpi_sdt::ACPISDTHeader;
use crate::drivers::acpi::tables::{rsdp, AcpiRevision};
use crate::drivers::acpi::tables::rsdp::{XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};

#[derive(Debug)]
pub struct InvalidChecksumError;
#[derive(Clone, Copy, PartialEq)]
#[derive(Debug)]
pub struct ACPISignature([u8; 4]); //all signatures are 4 chars (except rsdt)

impl ACPISignature {
    pub const RSDT: ACPISignature = ACPISignature(*b"RSDT");
    pub const XSDT: ACPISignature = ACPISignature(*b"XSDT");
    pub const FADT: ACPISignature = ACPISignature(*b"FACP");
}


pub trait AcpiSdtTable {
    fn get_signature(&self) -> ACPISignature;
    fn do_checksum(&self) -> bool {
        self.get_sdt_header().validate_checksum()
    }
    fn get_sdt_header(&self) -> ACPISDTHeader;
}

pub struct ACPITables {
    mem_physical_offset: u64,
    rsdp_or_xsdp: XSDP,
    rsdt_mappings: Vec<u64>
}

impl ACPITables {
    fn new(rsdp : XSDP, mem_physical_offset: u64) -> Self {
        ACPITables {
            mem_physical_offset,
            rsdp_or_xsdp: rsdp,
            rsdt_mappings: vec![]
        }
    }

    pub fn get_revision(&self) -> AcpiRevision {
        self.rsdp_or_xsdp.get_acpi_revision()
    }

    pub fn find_sdt_table(&self, signature: ACPISignature) -> Option<u64> {
        for i in 0..self.rsdt_mappings.len() {
            let ptr = self.rsdt_mappings[i];
            let header = ACPISDTHeader::new_from_ptr_u64(ptr + self.mem_physical_offset);
            if header.signature == signature {
                return Some(ptr + self.mem_physical_offset);
            }
        }
        None
    }

}
// ============================================================
//               **INITIALIZING THE TABLES**
// ============================================================
pub fn initialize_acpi_tables(boot_info: &BootInfo) -> Result<ACPITables, Error> {
    let rsdp_address: u64 = rsdp::get_rsdp_address(boot_info.physical_memory_offset);
    vgaprint!("Validating ACPI tables...");
    //*RSDP / XSDP*
    let rsdp = XSDP::new_rsdp_from_ptr(rsdp_address);

    if !rsdp.validate() {
        print_fail_msg!();
        return Err(Error);
    }

    let mut acpi_tables = ACPITables::new(*rsdp, boot_info.physical_memory_offset);
    if acpi_tables.get_revision() == AcpiRevision::Unknown {
        print_fail_msg!();
        return Err(Error);
    }
    print_ok_msg!();

    let rsdt_mapping = if acpi_tables.get_revision() == AcpiRevision::Acpi10 {
        get_mapping_from_rsdt(&acpi_tables)
    } else {
        acpi_tables.rsdp_or_xsdp = *XSDP::new_xsdp_from_rsd_ptr(rsdp_address);
        get_mapping_from_xsdt(&acpi_tables)
    };

    match rsdt_mapping {
        Ok(_) => {
            acpi_tables.rsdt_mappings = rsdt_mapping.unwrap();
            Ok(acpi_tables)
        }
        Err(_) => {
            Err(Error)
        }
    }
}

fn get_mapping_from_rsdt(acpi_tables: &ACPITables) -> Result<Vec<u64>, InvalidChecksumError> {
    let rsdt = RSDT::new_from_ptr(
        acpi_tables.rsdp_or_xsdp.get_rsdt_address() + acpi_tables.mem_physical_offset
    );

    if !rsdt.header.validate_checksum() {
        return Err(InvalidChecksumError);
    }

    let mut a = vec![];
    for i in 0..rsdt.get_mapping_length() {
        a[i] = rsdt.other_sdt_pointers[i] as u64;
    }
    Ok(a)
}

fn get_mapping_from_xsdt(acpi_tables: &ACPITables) -> Result<Vec<u64>, InvalidChecksumError> {
    let xsdp = XSDP::new_xsdp_from_rsd_ptr(acpi_tables.rsdp_or_xsdp.get_xsdt_address());
    if !xsdp.validate() {
        return Err(InvalidChecksumError);
    }

    let xsdt = XSDT::new_from_ptr(
        xsdp.xsdt_address + acpi_tables.mem_physical_offset
    );
    if !xsdt.header.validate_checksum() {
        return Err(InvalidChecksumError);
    }

    let mut a = vec![];
    for i in 0..xsdt.get_mapping_length() {
        a[i] = xsdt.other_sdt_pointers[i];
    }
    Ok(a)
}

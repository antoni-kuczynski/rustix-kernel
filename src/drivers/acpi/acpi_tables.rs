/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use alloc::vec;
use alloc::vec::Vec;
use crate::BootInfo;
use crate::drivers::acpi::tables::{rsdp, AcpiRevision};
use crate::drivers::acpi::tables::rsdp::{DescriptionPointerTable, RSDP, XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ACPISignature([u8; 4]); //all signatures are 4 chars (except rsdp)

#[allow(dead_code)]
impl ACPISignature {
    pub const RSDT: ACPISignature = ACPISignature(*b"RSDT");
    pub const XSDT: ACPISignature = ACPISignature(*b"XSDT");
    pub const FADT: ACPISignature = ACPISignature(*b"FACP");
    pub const DSDT: ACPISignature = ACPISignature(*b"DSDT");

    pub fn as_str<'a>(&self) -> &'a str {
        match self { 
            &ACPISignature::RSDT => "RSDT",
            &ACPISignature::XSDT => "XSDT",
            &ACPISignature::FADT => "FADT",
            &ACPISignature::DSDT => "DSDT",
            _ => "----"
        }
    }
}

#[derive(Debug)]
pub enum AcpiError {
    InvalidRevisionError,
    InvalidSdpChecksumError(),
    InvalidChecksumError(ACPISignature)
}

#[allow(dead_code)]
pub trait AcpiSdtTable {
    fn get_signature(&self) -> ACPISignature;
    fn validate(&self) -> bool;
    fn get_sdt_header(&self) -> ACPISDTHeader;
}

pub struct ACPITables<'a> {
    mem_physical_offset: u64,
    rsdp: Option<&'a RSDP>,
    xsdp: Option<&'a XSDP>,
    rsdt_mappings: Vec<u64>
}

#[allow(dead_code)]
impl<'a> ACPITables<'a> {
    fn new_from_xsdp(xsdp: &'a XSDP, mem_physical_offset: u64) -> Self {
        ACPITables {
            mem_physical_offset,
            xsdp: Some(xsdp),
            rsdp: None,
            rsdt_mappings: vec![]
        }
    }

    fn new_from_rsdp(rsdp : &'a RSDP, mem_physical_offset: u64) -> Self {
        ACPITables {
            mem_physical_offset,
            xsdp: None,
            rsdp: Some(rsdp),
            rsdt_mappings: vec![]
        }
    }

    pub fn get_revision(&self) -> AcpiRevision {
        match self.xsdp {
            Some(_) => AcpiRevision::Acpi20,
            None => {
                match self.rsdp {
                    None => { AcpiRevision::Unknown}
                    Some(_) => { AcpiRevision::Acpi10}
                }

            }
        }
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

    pub fn get_memory_offset(&self) -> u64 {
        self.mem_physical_offset
    }

}
// ============================================================
//               **INITIALIZING THE TABLES**
// ============================================================
pub fn get_acpi_tables(boot_info: &'_ BootInfo) -> Result<ACPITables<'_>, AcpiError> {
    let logical_rsdp_address: u64 = rsdp::get_rsdp_address(boot_info.physical_memory_offset);
    //*RSDP / XSDP*
    let rsdp = RSDP::new_from_rsd_ptr(logical_rsdp_address);
    if !rsdp.validate() {
        return Err(AcpiError::InvalidSdpChecksumError());
    }

    let mut acpi_tables;
    match rsdp.get_revision() {
        AcpiRevision::Unknown => {
            return Err(AcpiError::InvalidRevisionError);
        }
        AcpiRevision::Acpi10 => {
            //acpi tables from rsdp
            acpi_tables = ACPITables::new_from_rsdp(rsdp, boot_info.physical_memory_offset);

            //rsdt
            let rsdt = RSDT::new_from_ptr(
                rsdp.get_sdt_address() + acpi_tables.mem_physical_offset
            );

            if !rsdt.validate() {
                return Err(AcpiError::InvalidChecksumError(ACPISignature::RSDT));
            }

            acpi_tables.rsdt_mappings = rsdt.get_pointers_to_other_sdts();
        }
        AcpiRevision::Acpi20 => {
            let xsdp = XSDP::new_xsdp_from_rsd_ptr(logical_rsdp_address);
            if !xsdp.validate() {
                return Err(AcpiError::InvalidSdpChecksumError());
            }

            acpi_tables = ACPITables::new_from_xsdp(xsdp, boot_info.physical_memory_offset);

            let xsdt = XSDT::new(
                xsdp.get_sdt_address() + acpi_tables.mem_physical_offset
            );
            if !xsdt.validate() {
                return Err(AcpiError::InvalidChecksumError(ACPISignature::XSDT));
            }

            acpi_tables.rsdt_mappings = xsdt.get_pointers_to_other_sdts();
        }
    }
    Ok(acpi_tables)
}

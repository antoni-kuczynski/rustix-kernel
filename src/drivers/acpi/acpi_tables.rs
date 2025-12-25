/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use crate::boot::multiboot::{multiboot2_new_rsdp, multiboot2_old_rsdp};
use crate::drivers::acpi::acpi::enable_acpi;
use crate::drivers::acpi::tables::rsdp::{
    DescriptionPointerTable, RSDP, XSDP, rsdp_fallback_search_in_bios,
};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use crate::drivers::acpi::tables::{AcpiRevision};
use crate::memory::dir_mapping::physical_to_virtual;
use alloc::vec;
use alloc::vec::Vec;
use spin::Once;
use x86_64::{PhysAddr, VirtAddr};
use crate::kprintln;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ACPISignature([u8; 4]); //all signatures are 4 chars (except rsdp)

#[allow(dead_code)]
impl ACPISignature {
    pub const RSDT: ACPISignature = ACPISignature(*b"RSDT");
    pub const XSDT: ACPISignature = ACPISignature(*b"XSDT");
    pub const FADT: ACPISignature = ACPISignature(*b"FACP");
    pub const DSDT: ACPISignature = ACPISignature(*b"DSDT");
    pub const MADT: ACPISignature = ACPISignature(*b"APIC");
    pub const MCFG: ACPISignature = ACPISignature(*b"MCFG");

    pub fn as_str<'a>(&self) -> &'a str {
        match self {
            &ACPISignature::RSDT => "RSDT",
            &ACPISignature::XSDT => "XSDT",
            &ACPISignature::FADT => "FADT",
            &ACPISignature::DSDT => "DSDT",
            &ACPISignature::MADT => "APIC",
            &ACPISignature::MCFG => "MCFG",
            _ => "----",
        }
    }

    pub fn table_name<'a>(&self) -> &'a str {
        match self {
            &ACPISignature::RSDT => "RSDT",
            &ACPISignature::XSDT => "XSDT",
            &ACPISignature::FADT => "FADT",
            &ACPISignature::DSDT => "DSDT",
            &ACPISignature::MADT => "MADT",
            &ACPISignature::MCFG => "MCFG",
            _ => "",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum AcpiError {
    InvalidRevisionError,
    InvalidSdpChecksumError(),
    InvalidChecksumError(ACPISignature),
    RsdpNotFoundError,
}

#[allow(dead_code)]
pub trait AcpiSdtTable {
    fn get_signature(&self) -> ACPISignature;
    fn validate(&self) -> bool;
    fn get_sdt_header(&self) -> ACPISDTHeader;
}

pub struct ACPITables {
    rsdp: Option<&'static RSDP>,
    xsdp: Option<&'static XSDP>,
    rsdt_mappings: Vec<u64>,
}

#[allow(dead_code)]
impl ACPITables {
    fn new_from_xsdp(xsdp: &'static XSDP) -> Self {
        ACPITables {
            xsdp: Some(xsdp),
            rsdp: None,
            rsdt_mappings: vec![],
        }
    }

    fn new_from_rsdp(rsdp: &'static RSDP) -> Self {
        ACPITables {
            xsdp: None,
            rsdp: Some(rsdp),
            rsdt_mappings: vec![],
        }
    }

    fn get_revision(&self) -> AcpiRevision {
        match self.xsdp {
            Some(_) => AcpiRevision::Acpi20,
            None => match self.rsdp {
                None => AcpiRevision::Unknown,
                Some(_) => AcpiRevision::Acpi10,
            },
        }
    }

    fn find_sdt_table(&self, signature: ACPISignature) -> Option<VirtAddr> {
        for i in 0..self.rsdt_mappings.len() {
            let ptr = PhysAddr::new(self.rsdt_mappings[i]);
            let header = ACPISDTHeader::new_from_virt_addr(physical_to_virtual(ptr));
            if header.signature == signature {
                return Some(physical_to_virtual(ptr));
            }
        }
        None
    }
}
// ============================================================
//               **INITIALIZING THE TABLES**
// ============================================================
pub fn get_acpi_tables() -> Result<ACPITables, AcpiError> {
    let xsdp = multiboot2_new_rsdp();
    let mut rsdp: Option<&RSDP> = None;

    if xsdp.is_none() {
        kprintln!(Info, "Using old rsdp for ACPI.");
        rsdp = multiboot2_old_rsdp();
    } else if !xsdp.unwrap().validate() {
        return Err(AcpiError::InvalidSdpChecksumError());
    }

    if rsdp.is_none() && xsdp.is_none() {
        let addr = rsdp_fallback_search_in_bios();
        kprintln!(Info, "Using legacy bios search method for getting rsdp.");
        if addr.is_none() {
            return Err(AcpiError::RsdpNotFoundError);
        }
        rsdp = unsafe { Some(&*(addr.unwrap().as_u64() as *const RSDP)) };
        if !rsdp.unwrap().validate() {
            return Err(AcpiError::InvalidSdpChecksumError());
        }
    }

    let revision = if xsdp.is_some() {
        xsdp.unwrap().get_revision()
    } else {
        rsdp.unwrap().get_revision()
    };
    kprintln!(Info, "Detected ACPI revision: {}.", revision.as_u8());

    let mut acpi_tables;
    match revision {
        AcpiRevision::Unknown => {
            return Err(AcpiError::InvalidRevisionError);
        }
        AcpiRevision::Acpi10 => {
            //acpi tables from rsdp
            acpi_tables = ACPITables::new_from_rsdp(rsdp.unwrap());

            //rsdt
            let rsdt = RSDT::new_from_ptr(physical_to_virtual(PhysAddr::new(
                rsdp.unwrap().get_sdt_address(),
            )));

            if !rsdt.validate() {
                return Err(AcpiError::InvalidChecksumError(ACPISignature::RSDT));
            }

            acpi_tables.rsdt_mappings = rsdt.get_pointers_to_other_sdts();
        }
        AcpiRevision::Acpi20 => {
            if xsdp.is_none() {
                panic!("Acpi revision is 2.0 but no xsdp is present!");
            }
            acpi_tables = ACPITables::new_from_xsdp(xsdp.unwrap());

            let xsdt = XSDT::new(physical_to_virtual(PhysAddr::new(
                xsdp.unwrap().get_sdt_address(),
            )));
            if !xsdt.validate() {
                return Err(AcpiError::InvalidChecksumError(ACPISignature::XSDT));
            }

            acpi_tables.rsdt_mappings = xsdt.get_pointers_to_other_sdts();
        }
    }
    Ok(acpi_tables)
}

pub static ACPI_TABLES: Once<ACPITables> = Once::new();

pub fn acpi_init() {
    let tables = get_acpi_tables().expect("Acpi tables init failed!");
    ACPI_TABLES.call_once(|| tables);
    enable_acpi().expect("acpi enabling failed");
}

pub fn acpi_get_sdt_table(signature: ACPISignature) -> Option<VirtAddr> {
    ACPI_TABLES.get()?.find_sdt_table(signature)
}

pub fn acpi_get_revision() -> AcpiRevision {
    ACPI_TABLES.get().expect("ACPI not initialized!").get_revision()
}



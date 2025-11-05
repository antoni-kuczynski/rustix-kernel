/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::PartialEq;
use core::ptr::slice_from_raw_parts;
use bootloader::BootInfo;
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};
use crate::drivers::acpi::tables::fadt::*;
use crate::drivers::acpi::acpi_sdt::ACPISDTHeader;
use crate::drivers::acpi::tables::AcpiRevision;
use crate::drivers::acpi::tables::fadt::{find_FADT_address_from_rsdt, find_FADT_address_from_xsdt, FADT};
use crate::drivers::acpi::tables::rsdp::{RSDP, XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};

pub struct InvalidChecksumError;
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
    rsdp: XSDP,
    rsdt_mappings: Vec<u64>
}

impl ACPITables {
    fn new(rsdp : RSDP, mem_physical_offset: u64) -> Self {
        ACPITables {
            mem_physical_offset,
            rsdp,
            rsdt_mappings: vec![]
        }
    }

    pub fn get_revision(&self) -> AcpiRevision {
        self.rsdp.get_acpi_revision()
    }

}

// ============================================================
//               **INITIALIZING THE TABLES**
// ============================================================
pub fn initialize_acpi_tables(boot_info: &BootInfo) -> Option<ACPITables> {
    let rsdp_address: u64 = get_rsdp_address(boot_info.physical_memory_offset);
    vgaprint!("Validating ACPI tables...");
    //*RSDP / XSDP*
    let mut rsdp = XSDP::new_rsdp_from_ptr(rsdp_address);

    if !rsdp.validate_checksum() {
        print_fail_msg!();
        return None;
    }

    let mut acpi_tables = ACPITables::new(*rsdp, boot_info.physical_memory_offset);
    if acpi_tables.get_revision() == AcpiRevision::Unknown {
        print_fail_msg!();
        return None;
    }
    print_ok_msg!();



    let mut rsdt_mapping = if acpi_tables.get_revision() == AcpiRevision::Acpi10 {
        get_mapping_from_rsdt(&acpi_tables)
    } else {
        acpi_tables.rsdp = *XSDP::new_xsdp_from_rsd_ptr(rsdp_address);
        initialize_acpi_v2_and_newer(&acpi_tables)
    }
}

fn get_mapping_from_rsdt(acpi_tables: &ACPITables) -> Result<Vec<u64>, InvalidChecksumError> {
    let rsdt = RSDT::new_from_ptr(
        acpi_tables.rsdp.rsdt_address as u64 + acpi_tables.mem_physical_offset
    );

    if !rsdt.header.validate_checksum() {
        print_fail_msg!();
        return Err(InvalidChecksumError);
    }

    let mut a = vec![];
    for i in 0..rsdt.get_mapping_length() {
        a[i] = rsdt.other_sdt_pointers[i] as u64;
    }
    Ok(a)
}

fn initialize_acpi_v2_and_newer(acpi_tables: &ACPITables) -> Result<Vec<u64>, InvalidChecksumError> {
    let mut xsdp = XSDP::new_xsdp_from_rsd_ptr(acpi_tables.rsdp.);
    if !xsdp.validate_extended_checksum() {
        print_fail_msg!();
        return None;
    }

    let mut xsdt = XSDT::new_from_ptr(
        xsdp.xsdt_address + physical_mem_offset
    );
    if !xsdt.header.validate_checksum() {
        print_fail_msg!();
        return None;
    }

    let fadt_ptr = find_FADT_address_from_xsdt(&xsdt, physical_mem_offset);
    let fadt = match fadt_ptr {
        None => {
            print_fail_msg!();
            return None;
        }
        Some(_) => {
            FADT::new_from_ptr(fadt_ptr.unwrap())
        }
    };
    print_ok_msg!();

    Some(ACPITables {
        fadt
    })
}

// ============================================================
//              **SERCHING THE MEMORY FOR RSDP**
// ============================================================
const BIOS_START: u64 = 0x000E0000;
const BIOS_END: u64   = 0x000FFFFF;
const RSD_EXPECTED_SIGNATURE: &[u8] = b"RSD PTR ";

//scanning the BIOS in region 0x000E0000 - 0x000FFFFF for "RSD PTR" signature
fn get_rsdp_address(physical_memory_offset: u64) -> u64 {
    vgaprint!("Searching for ACPI tables...");
    unsafe {
        let mut addr = BIOS_START;
        while addr <= BIOS_END {
            let vaddr = (addr + physical_memory_offset) as *const u8;
            let slice = core::slice::from_raw_parts(vaddr, 8);
            if slice == RSD_EXPECTED_SIGNATURE {
                print_ok_msg!();
                return addr + physical_memory_offset;
            }
            addr += 16;
        }

        print_fail_msg!();
        BIOS_START
    }
}


use alloc::boxed::Box;
use alloc::string::String;
use crate::drivers::acpi::tables::AcpiRevision;
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};

/*
 * Created by Antoni Kuczyński
 * 05/11/2025
 */
const BIOS_START: u64 = 0x000E0000;
const BIOS_END: u64   = 0x000FFFFF;
const RSD_EXPECTED_SIGNATURE: &[u8] = b"RSD PTR ";


// ============================================================
//               **XSDP & RSDP**
//  The RSDP is used on ACPI version 1.0,
//  XSDP is used on ACPI version 2.0+
// ============================================================
#[repr(C, packed)]
struct RSDP {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct XSDP {
    pub rsdp: &'static RSDP,
    //XSDP fields - ACPI 2.0+
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3]
}

impl RSDP {
    pub fn new_from_rsd_ptr(ptr: u64) -> &'static RSDP {
        unsafe {
            &*(ptr as *const RSDP)
        }
    }

    pub fn validate_checksum(&self) -> bool {
        unsafe {
            let ptr = self as *const _ as *const u8;
            let mut sum: u8 = 0;
            for i in 0..20 {
                sum = sum.wrapping_add(*ptr.add(i));
            }
            sum == 0
        }
    }

    pub fn print(&self) {
        let signature = self.signature;
        let checksum = self.checksum;
        let oem_id = self.oem_id;
        let revision = self.revision;
        let rsdt_address = self.rsdt_address;

        vgaprintln!("==== RSDP Table Descriptor) ====");
        vgaprintln!("Signature          : {}", String::from_utf8_lossy(&signature));
        vgaprintln!("Checksum           : {:#04x}", checksum);
        vgaprintln!("OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        vgaprintln!("Revision           : {}", revision);
        vgaprintln!("RSDT Address       : {:#010x}", rsdt_address);
        vgaprintln!("====================================");
    }
}

impl XSDP {
    pub fn new_xsdp_from_rsd_ptr(ptr: u64) -> &'static XSDP {
        unsafe {
            &*(ptr as *const XSDP)
        }
    }



    pub fn new_rsdp_from_ptr(ptr: u64) -> Box<XSDP> {
        Box::new(XSDP {
            rsdp: RSDP::new_from_rsd_ptr(ptr),
            length: 0,
            xsdt_address: 0,
            extended_checksum: 0,
            reserved: [0,0,0]
        })
    }

    fn validate_extended_checksum(&self) -> bool {
        unsafe {
            let ptr = self as *const _ as *const u8;
            let mut sum: u8 = 0;
            let length = self.length as usize;

            for i in 0..length {
                sum = sum.wrapping_add(*ptr.add(i));
            }

            sum == 0
        }
    }

    pub fn get_acpi_revision(&self) -> AcpiRevision {
        match self.rsdp.revision {
            1 => AcpiRevision::Acpi10,
            2 => AcpiRevision::Acpi20,
            _ => AcpiRevision::Unknown
        }
    }

    pub fn get_rsdt_address(&self) -> u64 {
        self.rsdp.rsdt_address as u64
    }

    pub fn get_xsdt_address(&self) -> u64 {
        self.xsdt_address
    }

    pub fn validate(&self) -> bool {
        self.rsdp.validate_checksum() && self.validate_extended_checksum()
    }

    pub fn print(&self) {
        let signature = self.rsdp.signature;
        let checksum = self.rsdp.checksum;
        let oem_id = self.rsdp.oem_id;
        let revision = self.rsdp.revision;
        let rsdt_address = self.rsdp.rsdt_address;
        let length = self.length;
        let xsdt_address = self.xsdt_address;
        let extended_checksum = self.extended_checksum;
        let reserved = self.reserved;

        vgaprintln!("==== XSDP Table Descriptor) ====");
        vgaprintln!("Signature          : {}", String::from_utf8_lossy(&signature));
        vgaprintln!("Checksum           : {:#04x}", checksum);
        vgaprintln!("OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        vgaprintln!("Revision           : {}", revision);
        vgaprintln!("RSDT Address       : {:#010x}", rsdt_address);
        vgaprintln!("Length             : {}", length);
        vgaprintln!("XSDT Address       : {:#018x}", xsdt_address);
        vgaprintln!("Extended Checksum  : {:#04x}", extended_checksum);
        vgaprintln!("Reserved           : {:?}", reserved);
        vgaprintln!("====================================");
    }
}

// ============================================================
//              **SERCHING THE MEMORY FOR RSDP**
// ============================================================
pub fn get_rsdp_address(physical_memory_offset: u64) -> u64 {
    unsafe {
        let mut addr = BIOS_START;
        while addr <= BIOS_END {
            let vaddr = (addr + physical_memory_offset) as *const u8;
            let slice = core::slice::from_raw_parts(vaddr, 8);
            if slice == RSD_EXPECTED_SIGNATURE {
                return addr + physical_memory_offset;
            }
            addr += 16;
        }
        BIOS_START
    }
}
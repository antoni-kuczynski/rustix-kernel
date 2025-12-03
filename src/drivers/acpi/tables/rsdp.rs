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
pub struct RSDP {
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

pub trait DesciptionPointerTable {
    fn get_signature(&self) -> [u8; 8];
    fn validate(&self) -> bool;
    fn get_oem_id(&self) -> [u8; 6];
    fn get_revision(&self) -> AcpiRevision;
    fn get_sdt_address(&self) -> u64;
}

impl DesciptionPointerTable for RSDP {
    fn get_signature(&self) -> [u8; 8] {
        self.signature
    }

    fn validate(&self) -> bool {
        unsafe {
            let ptr = self as *const _ as *const u8;
            let mut sum: u8 = 0;
            for i in 0..20 {
                sum = sum.wrapping_add(*ptr.add(i));
            }
            sum == 0
        }
    }

    fn get_oem_id(&self) -> [u8; 6] {
        self.oem_id
    }

    fn get_revision(&self) -> AcpiRevision {
        match self.revision {
            0 => AcpiRevision::Acpi10,
            2 => AcpiRevision::Acpi20,
            _ => AcpiRevision::Unknown
        }
    }

    fn get_sdt_address(&self) -> u64 {
        self.rsdt_address as u64
    }
}


impl RSDP {
    pub fn new_from_rsd_ptr(ptr: u64) -> &'static RSDP {
        unsafe {
            &*(ptr as *const RSDP)
        }
    }
}


impl DesciptionPointerTable for XSDP {
    fn get_signature(&self) -> [u8; 8] {
        self.rsdp.signature
    }

    fn validate(&self) -> bool {
        if !self.rsdp.validate() {
            return false;
        }

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

    fn get_oem_id(&self) -> [u8; 6] {
        self.rsdp.oem_id
    }

    fn get_revision(&self) -> AcpiRevision {
        match self.rsdp.revision {
            0 => AcpiRevision::Acpi10,
            2 => AcpiRevision::Acpi20,
            _ => AcpiRevision::Unknown
        }
    }

    fn get_sdt_address(&self) -> u64 {
        self.xsdt_address
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
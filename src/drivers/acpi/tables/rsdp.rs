use crate::drivers::acpi::tables::AcpiRevision;
use crate::memory::dir_mapping::physical_to_virtual;
use x86_64::{PhysAddr, VirtAddr};
/*
 * Created by Antoni Kuczyński
 * 05/11/2025
 */
const BIOS_START: PhysAddr = PhysAddr::new(0x000E0000);
const BIOS_END: PhysAddr = PhysAddr::new(0x000FFFFF);
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
pub struct XSDP {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
    //XSDP fields - ACPI 2.0+
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

#[allow(dead_code)]
pub trait DescriptionPointerTable {
    fn get_signature(&self) -> [u8; 8];
    fn validate(&self) -> bool;
    fn get_oem_id(&self) -> [u8; 6];
    fn get_revision(&self) -> AcpiRevision;
    fn get_sdt_address(&self) -> u64;
}

impl DescriptionPointerTable for RSDP {
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
        AcpiRevision::from_u8(self.revision)
    }

    fn get_sdt_address(&self) -> u64 {
        self.rsdt_address as u64
    }
}

impl RSDP {
    pub fn new_from_rsd_ptr<'a>(ptr: u64) -> &'a RSDP {
        unsafe { &*(ptr as *const RSDP) }
    }
}

impl DescriptionPointerTable for XSDP {
    fn get_signature(&self) -> [u8; 8] {
        self.signature
    }

    fn validate(&self) -> bool {
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
        self.oem_id
    }

    fn get_revision(&self) -> AcpiRevision {
        AcpiRevision::from_u8(self.revision)
    }

    fn get_sdt_address(&self) -> u64 {
        self.xsdt_address
    }
}

impl XSDP {
    pub fn new_xsdp_from_rsd_ptr<'a>(ptr: u64) -> &'a XSDP {
        unsafe { &*(ptr as *const XSDP) }
    }
}

// ============================================================
//              **SERCHING THE MEMORY FOR RSDP**
// ============================================================
pub fn rsdp_fallback_search_in_bios() -> Option<VirtAddr> {
    unsafe {
        let mut addr = physical_to_virtual(BIOS_START);
        let end = physical_to_virtual(BIOS_END);
        while addr.as_u64() <= end.as_u64() {
            let vaddr = addr.as_u64() as *const u8;
            let slice = core::slice::from_raw_parts(vaddr, 8);
            if slice == RSD_EXPECTED_SIGNATURE {
                return Some(addr);
            }
            addr += 16;
        }
        None
    }
}

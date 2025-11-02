/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use alloc::string::String;
use bootloader::BootInfo;
use crate::{vgaprint, vgaprintln};
use crate::drivers::vga::{Color, VGAWRITER};

// ============================================================
//               **INITIALIZING THE TABLES**
// ============================================================
pub fn initialize_acpi_tables(boot_info: &BootInfo) {
    let rsdp_address: u64 = get_rsdp_address(boot_info.physical_memory_offset);
    vgaprint!("Validating ACPI tables...");
    //*RSDP / XSDP*
    let mut rsdp = RSDP::new_from_rsd_ptr(rsdp_address);

    if !rsdp.validate_checksum() {
        VGAWRITER.lock().change_foreground_color(Color::Red);
        vgaprintln!(" FAIL!");
        VGAWRITER.lock().change_foreground_color(Color::White);
        return;
    }

    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);


    //detecting ACPI version
    let is_acpi_version_1: bool = rsdp.revision == 0;

    //switch to XSDP if ACPI 2.0 is used
    if is_acpi_version_1 {
        initialize_acpi_v1(rsdp_address);
    } else {
        initialize_acpi_v2_and_newer(rsdp_address);
    }
}

fn initialize_acpi_v1(ptr: u64) {
    vgaprint!("Initlializing ACPI 1.0 tables...");
    let mut rsdp = RSDP::new_from_rsd_ptr(ptr);



    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);

    rsdp.print();
}

fn initialize_acpi_v2_and_newer(ptr: u64) {
    vgaprint!("Initlializing ACPI 2.0 tables...");
    let mut xsdp = XSDP::new_from_rsd_ptr(ptr);
    if !xsdp.validate_extended_checksum() {
        VGAWRITER.lock().change_foreground_color(Color::Red);
        vgaprintln!(" FAIL!");
        VGAWRITER.lock().change_foreground_color(Color::White);
        return;
    }



    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);

    xsdp.print();
}

// ============================================================
//              **SERCHING THE MEMORY FOR RSDP**
// ============================================================
// const PHYS_OFFSET: u64 = 1649267441664u64;
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
                VGAWRITER.lock().change_foreground_color(Color::Green);
                vgaprintln!(" OK!");
                VGAWRITER.lock().change_foreground_color(Color::White);
                return addr + physical_memory_offset;
            }
            addr += 16;
        }

        VGAWRITER.lock().change_foreground_color(Color::Red);
        vgaprintln!(" FAILED!");
        VGAWRITER.lock().change_foreground_color(Color::White);
        BIOS_START
    }
}
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
    //RSDP fields - ACPI 1.0
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
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

    fn validate_checksum(&self) -> bool {
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
    pub fn new_from_rsd_ptr(ptr: u64) -> &'static XSDP {
        unsafe {
            &*(ptr as *const XSDP)
        }
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

    pub fn print(&self) {
        let signature = self.signature;
        let checksum = self.checksum;
        let oem_id = self.oem_id;
        let revision = self.revision;
        let rsdt_address = self.rsdt_address;
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

use crate::asm::inw;
use crate::drivers::acpi::tables::fadt::FADT;
use crate::drivers::acpi::tables::rsdp::{RSDP, XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use crate::vgaprintln;
use alloc::string::String;

#[allow(dead_code)]
pub trait ACPITablePrinter {
    fn print(&self);
}

impl ACPITablePrinter for RSDP {
    fn print(&self) {
        let signature = self.signature;
        let checksum = self.checksum;
        let oem_id = self.oem_id;
        let revision = self.revision;
        let rsdt_address = self.rsdt_address;

        vgaprintln!("==== RSDP Table Descriptor) ====");
        vgaprintln!(
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
        vgaprintln!("Checksum           : {:#04x}", checksum);
        vgaprintln!("OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        vgaprintln!("Revision           : {}", revision);
        vgaprintln!("RSDT Address       : {:#010x}", rsdt_address);
        vgaprintln!("====================================");
    }
}

impl ACPITablePrinter for XSDP {
    fn print(&self) {
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
        vgaprintln!(
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
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

impl ACPITablePrinter for RSDT {
    fn print(&self) {
        let header: ACPISDTHeader = self.header;
        let signature = header.signature;
        let length = header.length;
        let revision = header.revision;
        let checksum = header.checksum;
        let oem_id = header.oem_id;
        let oem_table_id = header.oem_table_id;
        let oem_revision = header.oem_revision;
        let creator_id = header.creator_id;
        let creator_revision = header.creator_revision;

        vgaprintln!("RSDT:");
        vgaprintln!("  Signature: {:?}", signature);
        vgaprintln!("  Length:    {}", length);
        vgaprintln!("  Revision:  {}", revision);
        vgaprintln!("  Checksum:  {}", checksum);
        vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        vgaprintln!(
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        vgaprintln!("  OEM Revision: {}", oem_revision);
        vgaprintln!("  Creator ID:   {:?}", creator_id);
        vgaprintln!("  Creator Rev:  {}", creator_revision);

        // let ptrs: [u32] = self.other_sdt_pointers;
        for i in 0..self.get_mapping_length() {
            let addr = self.other_sdt_pointers[i];
            vgaprintln!("    [{}] 0x{:08X}", i, addr);
        }
    }
}

impl ACPITablePrinter for XSDT {
    fn print(&self) {
        let header: ACPISDTHeader = self.header;
        let signature = header.signature;
        let length = header.length;
        let revision = header.revision;
        let checksum = header.checksum;
        let oem_id = header.oem_id;
        let oem_table_id = header.oem_table_id;
        let oem_revision = header.oem_revision;
        let creator_id = header.creator_id;
        let creator_revision = header.creator_revision;

        vgaprintln!("XSDT:");
        vgaprintln!("  Signature: {:?}", signature);
        vgaprintln!("  Length:    {}", length);
        vgaprintln!("  Revision:  {}", revision);
        vgaprintln!("  Checksum:  {}", checksum);
        vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        vgaprintln!(
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        vgaprintln!("  OEM Revision: {}", oem_revision);
        vgaprintln!("  Creator ID:   {:?}", creator_id);
        vgaprintln!("  Creator Rev:  {}", creator_revision);

        for i in 0..((length as usize - size_of_val(&header)) >> 3) {
            let addr = self.other_sdt_pointers[i];
            vgaprintln!("    [{}] 0x{:08X}", i, addr);
        }
    }
}

impl ACPITablePrinter for FADT {
    fn print(&self) {
        let a = self.smi_command_port;
        let b = self.pm1a_control_block;
        unsafe {
            vgaprintln!("FADT smi command port:");
            vgaprintln!("{}", inw(a as u16));
            vgaprintln!("FADT pm1a control block:");
            vgaprintln!("{}", inw(b as u16));
        }
    }
}

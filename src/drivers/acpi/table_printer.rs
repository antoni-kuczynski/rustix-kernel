use crate::drivers::acpi::tables::fadt::FADT;
use crate::drivers::acpi::tables::rsdp::{RSDP, XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use crate::__vgaprintln;

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

        __vgaprintln!("==== RSDP Table Descriptor) ====");
        __vgaprintln!(
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
        __vgaprintln!("Checksum           : {:#04x}", checksum);
        __vgaprintln!("OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        __vgaprintln!("Revision           : {}", revision);
        __vgaprintln!("RSDT Address       : {:#010x}", rsdt_address);
        __vgaprintln!("====================================");
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

        __vgaprintln!("==== XSDP Table Descriptor) ====");
        __vgaprintln!(
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
        __vgaprintln!("Checksum           : {:#04x}", checksum);
        __vgaprintln!("OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        __vgaprintln!("Revision           : {}", revision);
        __vgaprintln!("RSDT Address       : {:#010x}", rsdt_address);
        __vgaprintln!("Length             : {}", length);
        __vgaprintln!("XSDT Address       : {:#018x}", xsdt_address);
        __vgaprintln!("Extended Checksum  : {:#04x}", extended_checksum);
        __vgaprintln!("Reserved           : {:?}", reserved);
        __vgaprintln!("====================================");
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

        __vgaprintln!("RSDT:");
        __vgaprintln!("  Signature: {:?}", signature);
        __vgaprintln!("  Length:    {}", length);
        __vgaprintln!("  Revision:  {}", revision);
        __vgaprintln!("  Checksum:  {}", checksum);
        __vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        __vgaprintln!(
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        __vgaprintln!("  OEM Revision: {}", oem_revision);
        __vgaprintln!("  Creator ID:   {:?}", creator_id);
        __vgaprintln!("  Creator Rev:  {}", creator_revision);

        // let ptrs: [u32] = self.other_sdt_pointers;
        for i in 0..self.get_mapping_length() {
            let addr = self.other_sdt_pointers[i];
            __vgaprintln!("    [{}] 0x{:08X}", i, addr);
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

        __vgaprintln!("XSDT:");
        __vgaprintln!("  Signature: {:?}", signature);
        __vgaprintln!("  Length:    {}", length);
        __vgaprintln!("  Revision:  {}", revision);
        __vgaprintln!("  Checksum:  {}", checksum);
        __vgaprintln!("  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        __vgaprintln!(
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        __vgaprintln!("  OEM Revision: {}", oem_revision);
        __vgaprintln!("  Creator ID:   {:?}", creator_id);
        __vgaprintln!("  Creator Rev:  {}", creator_revision);

        for i in 0..((length as usize - size_of_val(&header)) >> 3) {
            let addr = self.other_sdt_pointers[i];
            __vgaprintln!("    [{}] 0x{:08X}", i, addr);
        }
    }
}

impl ACPITablePrinter for FADT {
    fn print(&self) {
        let a = self.smi_command_port;
        let b = self.pm1a_control_block;
        __vgaprintln!("FADT smi command port:");
        __vgaprintln!("{}", inw(a as u16));
        __vgaprintln!("FADT pm1a control block:");
        __vgaprintln!("{}", inw(b as u16));
    }
}

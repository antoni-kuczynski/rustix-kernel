use alloc::string::String;
use crate::drivers::acpi::tables::fadt::FADT;
use crate::drivers::acpi::tables::rsdp::{RSDP, XSDP};
use crate::drivers::acpi::tables::rsdt::{RSDT, XSDT};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use crate::{kprintln};
use crate::asm::inw;

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

        kprintln!(Debug, "==== RSDP Table Descriptor) ====");
        kprintln!(Debug, 
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
        kprintln!(Debug, "Checksum           : {:#04x}", checksum);
        kprintln!(Debug, "OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        kprintln!(Debug, "Revision           : {}", revision);
        kprintln!(Debug, "RSDT Address       : {:#010x}", rsdt_address);
        kprintln!(Debug, "====================================");
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

        kprintln!(Debug, "==== XSDP Table Descriptor) ====");
        kprintln!(Debug, 
            "Signature          : {}",
            String::from_utf8_lossy(&signature)
        );
        kprintln!(Debug, "Checksum           : {:#04x}", checksum);
        kprintln!(Debug, "OEM ID             : {}", String::from_utf8_lossy(&oem_id));
        kprintln!(Debug, "Revision           : {}", revision);
        kprintln!(Debug, "RSDT Address       : {:#010x}", rsdt_address);
        kprintln!(Debug, "Length             : {}", length);
        kprintln!(Debug, "XSDT Address       : {:#018x}", xsdt_address);
        kprintln!(Debug, "Extended Checksum  : {:#04x}", extended_checksum);
        kprintln!(Debug, "Reserved           : {:?}", reserved);
        kprintln!(Debug, "====================================");
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

        kprintln!(Debug, "RSDT:");
        kprintln!(Debug, "  Signature: {:?}", signature);
        kprintln!(Debug, "  Length:    {}", length);
        kprintln!(Debug, "  Revision:  {}", revision);
        kprintln!(Debug, "  Checksum:  {}", checksum);
        kprintln!(Debug, "  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        kprintln!(Debug, 
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        kprintln!(Debug, "  OEM Revision: {}", oem_revision);
        kprintln!(Debug, "  Creator ID:   {:?}", creator_id);
        kprintln!(Debug, "  Creator Rev:  {}", creator_revision);

        // let ptrs: [u32] = self.other_sdt_pointers;
        for i in 0..self.get_mapping_length() {
            let addr = self.other_sdt_pointers[i];
            kprintln!(Debug, "    [{}] 0x{:08X}", i, addr);
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

        kprintln!(Debug, "XSDT:");
        kprintln!(Debug, "  Signature: {:?}", signature);
        kprintln!(Debug, "  Length:    {}", length);
        kprintln!(Debug, "  Revision:  {}", revision);
        kprintln!(Debug, "  Checksum:  {}", checksum);
        kprintln!(Debug, "  OEM ID:    {:?}", String::from_utf8_lossy(&oem_id));
        kprintln!(Debug, 
            "  OEM Table ID: {:?}",
            String::from_utf8_lossy(&oem_table_id)
        );
        kprintln!(Debug, "  OEM Revision: {}", oem_revision);
        kprintln!(Debug, "  Creator ID:   {:?}", creator_id);
        kprintln!(Debug, "  Creator Rev:  {}", creator_revision);

        for i in 0..((length as usize - size_of_val(&header)) >> 3) {
            let addr = self.other_sdt_pointers[i];
            kprintln!(Debug, "    [{}] 0x{:08X}", i, addr);
        }
    }
}

impl ACPITablePrinter for FADT {
    fn print(&self) {
        let a = self.smi_command_port;
        let b = self.pm1a_control_block;
        unsafe {
            kprintln!(Debug, "FADT smi command port:");
            kprintln!(Debug, "{}", inw(a as u16));
            kprintln!(Debug, "FADT pm1a control block:");
            kprintln!(Debug, "{}", inw(b as u16));
        }
    }
}

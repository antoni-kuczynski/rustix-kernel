use crate::drivers::acpi::acpi_tables::AcpiSdtTable;

pub mod rsdp;
pub mod rsdt;
pub mod fadt;
pub mod sdt_header;
pub mod dsdt;

#[derive(PartialEq)]
pub enum AcpiRevision {
    Unknown = 3,
    Acpi10 = 0,
    Acpi20 = 2
}

impl AcpiRevision {
    pub fn as_u8(&self) -> u8 {
        match self {
            AcpiRevision::Unknown => 3,
            AcpiRevision::Acpi10 => 0,
            AcpiRevision::Acpi20 => 2
        }
    }
}
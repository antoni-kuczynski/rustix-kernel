use crate::drivers::acpi::acpi_tables::AcpiSdtTable;

pub mod rsdp;
pub mod rsdt;
pub mod fadt;


#[derive(PartialEq)]
pub enum AcpiRevision {
    Unknown = 0,
    Acpi10 = 1,
    Acpi20 = 2
}
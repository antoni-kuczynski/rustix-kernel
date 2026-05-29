pub mod dsdt;
pub mod fadt;
pub mod rsdp;
pub mod rsdt;
pub mod sdt_header;

#[derive(PartialEq)]
pub enum AcpiRevision {
    Unknown = 3,
    Acpi10 = 0,
    Acpi20 = 2,
}

#[allow(dead_code)]
impl AcpiRevision {
    pub fn as_u8(&self) -> u8 {
        match self {
            AcpiRevision::Unknown => 3,
            AcpiRevision::Acpi10 => 0,
            AcpiRevision::Acpi20 => 2,
        }
    }

    pub fn from_u8(val: u8) -> AcpiRevision {
        match val {
            0 => AcpiRevision::Acpi10,
            2 => AcpiRevision::Acpi20,
            _ => AcpiRevision::Unknown,
        }
    }
}

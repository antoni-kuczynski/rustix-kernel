/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use crate::drivers::acpi::acpi_tables::ACPITables;
use crate::vgaprintln;

pub fn init(t: Option<ACPITables>) {
    let tables = match t {
        None => {
            vgaprintln!("ACPI initialization failed!");
            return;
        }
        Some(_) => {
            t.unwrap()
        }
    };

    //enable acpi
    tables.fadt.enable_acpi();

}
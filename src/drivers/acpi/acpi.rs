/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use core::fmt::Error;
use crate::drivers::acpi::acpi_tables::{ACPISignature, ACPITables};
use crate::asm::{inw, outb, outw};
use crate::drivers::acpi::tables::fadt::FADT;
use crate::interrupts::hardware::pic8259::{get_current_time_millis};
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};
use crate::drivers::acpi::tables::dsdt::{S5Obj, DSDT};
use crate::drivers::vga::vga_text::VGAWRITER;
use crate::drivers::vga::vga_text::ColorTextMode;

#[allow(dead_code)]
pub fn enable_acpi(tables: &ACPITables) -> Result<(), Error> {
    vgaprint!("Enabling ACPI...");
    unsafe {
        let fadt: &FADT = match tables.find_sdt_table(ACPISignature::FADT) {
            None => {
                return Err(Error);
            }
            Some(ptr) => {
                FADT::new_from_ptr(ptr)
            }
        };

        outb(fadt.smi_command_port as u16, fadt.acpi_enable);

        //check if ACPI is actually enabled
        let mut prev_time = get_current_time_millis();
        let mut current_time;
        let mut d_time = 0; //wait for 3 seconds (linux approach)
        while (inw(fadt.pm1a_control_block as u16) & 1 == 0) &&  d_time < 3_000 {
            current_time = get_current_time_millis();
            d_time += current_time - prev_time;
            prev_time = current_time;
        }

        if inw(fadt.pm1a_control_block as u16) & 1 == 0 {
            print_fail_msg!();
            return Err(Error);
        }
    }
    print_ok_msg!();
    Ok(())
}

#[allow(dead_code)]
pub fn acpi_soft_off_state(tables: &ACPITables) -> Result<(), Error> {
    let facp_ptr: u64 = match tables.find_sdt_table(ACPISignature::FADT) {
        Some(x) => x,
        None => return Err(Error)
    };

    let fadt: &FADT = FADT::new_from_ptr(facp_ptr);
    let dsdt: &DSDT = DSDT::new_from_ptr(fadt.get_dsdt_pointer() + tables.get_memory_offset());
    let s5 = match S5Obj::new_from_dsdt(dsdt) {
        Some(x) => x,
        None => return Err(Error)
    };

    unsafe {
        outw(fadt.pm1a_control_block as u16, s5.SLP_TYPa as u16 | (1 << 13));
    }
    Ok(())
}


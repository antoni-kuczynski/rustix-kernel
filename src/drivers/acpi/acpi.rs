/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use core::fmt::Error;
use lazy_static::lazy_static;
use spin::Once;
use crate::drivers::acpi::acpi_tables::{ACPISignature, ACPITables};
use crate::asm::{inw, outb};
use crate::drivers::acpi::tables::fadt::FADT;
use crate::interrupts::hardware::pic8259::get_current_time_millis;
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};
use crate::drivers::vga::vga_text::VGAWRITER;
use crate::drivers::vga::vga_text::ColorTextMode;

//please forgive me
pub static mut TABLES: Once<&ACPITables> = Once::new();

#[macro_export]
macro_rules! get_acpi_tables {
    () => {
        {
            *TABLES.get().unwrap_unchecked()
        }
    };
}

pub fn enable_acpi() -> Result<(), Error> {
    vgaprint!("Enabling ACPI...");
    unsafe {
        let fadt: &FADT = match get_acpi_tables!().find_sdt_table(ACPISignature::FADT) {
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

pub fn init(t: &'static Result<ACPITables, Error>) -> Result<(), Error> {
    match t {
        Err(_) => {
            panic!("No ACPI tables found!");
        }
        Ok(t) => {
            unsafe {
                TABLES.call_once(|| t);
            }
        }
    };
    Ok(())
}


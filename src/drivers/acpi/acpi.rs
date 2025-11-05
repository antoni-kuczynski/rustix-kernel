/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use crate::drivers::acpi::acpi_tables::ACPITables;
use crate::{print_fail_msg, print_ok_msg, vgaprint, vgaprintln};
use crate::asm::{inw, outb};
use crate::interrupts::hardware::pic8259::get_current_time_millis;

pub fn enable_acpi() {
    vgaprint!("Enabling ACPI...");
    unsafe {
        outb(self.smi_command_port as u16, self.acpi_enable);
    }

    //check if ACPI is actually enabled
    unsafe {
        let mut prev_time = get_current_time_millis();
        let mut current_time;
        let mut d_time = 0; //wait for 3 seconds (linux approach)
        while (inw(self.pm1a_control_block as u16) & 1 == 0) &&  d_time < 3_000 {
            current_time = get_current_time_millis();
            d_time += current_time - prev_time;
            prev_time = current_time;
        }

        if inw(self.pm1a_control_block as u16) & 1 == 0 {
            print_fail_msg!();
            return;
        }
    }
    print_ok_msg!();
}


fn get_acpi_tables() -> Option<ACPITables> {

}

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
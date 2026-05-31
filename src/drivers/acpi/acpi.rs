/*
 * Created by Antoni Kuczyński
 * 01/11/2025
 */
use crate::asm::{inw, outb, outw};
use crate::drivers::acpi::acpi_tables::{ACPI_TABLES, ACPISignature, acpi_get_sdt_table, acpi_get_revision};
use crate::drivers::acpi::tables::AcpiRevision;
use crate::drivers::acpi::tables::dsdt::{DSDT, S5Obj};
use crate::drivers::acpi::tables::fadt::FADT;
use crate::drivers::vga::vga_text::ColorTextMode;
use crate::drivers::vga::vga_text::VGAWRITER;
use crate::memory::dir_mapping::physical_to_virtual;
use crate::{print_fail_msg, print_ok_msg, vgaprint};
use core::fmt::Error;
use x86_64::{PhysAddr, VirtAddr};
use crate::drivers::apic::apic::timer_lapic_uptime_ms;

#[allow(dead_code)]
pub fn enable_acpi() -> Result<(), Error> {
    vgaprint!("Enabling ACPI...");
    let tables = match ACPI_TABLES.get() {
        None => {
            print_fail_msg!();
            panic!("ACPI tables not initialized!");
        }
        Some(x) => x,
    };
    unsafe {
        let fadt: &FADT = match acpi_get_sdt_table(ACPISignature::FADT) {
            None => {
                return Err(Error);
            }
            Some(ptr) => FADT::new_from_ptr(ptr),
        };

        outb(fadt.smi_command_port as u16, fadt.acpi_enable);

        //check if ACPI is actually enabled
        let mut prev_time = timer_lapic_uptime_ms();
        let mut current_time;
        let mut d_time = 0; //wait for 3 seconds (linux approach)
        while (inw(fadt.pm1a_control_block as u16) & 1 == 0) && d_time < 3_000 {
            current_time = timer_lapic_uptime_ms();
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
pub fn acpi_soft_off_state() -> Result<(), Error> {
    let tables = match ACPI_TABLES.get() {
        None => {
            print_fail_msg!();
            panic!("ACPI tables not initialized!");
        }
        Some(x) => x,
    };

    let facp_ptr: VirtAddr = match acpi_get_sdt_table(ACPISignature::FADT) {
        Some(x) => x,
        None => return Err(Error),
    };

    let fadt: &FADT = FADT::new_from_ptr(facp_ptr);
    let dsdt: &DSDT =
        DSDT::new_from_ptr(physical_to_virtual(PhysAddr::new(fadt.get_dsdt_pointer())));
    let s5 = match S5Obj::new_from_dsdt(dsdt) {
        Some(x) => x,
        None => return Err(Error),
    };

    unsafe {
        outw(
            fadt.pm1a_control_block as u16,
            s5.SLP_TYPa as u16 | (1 << 13),
        );
    }
    Ok(())
}

#[allow(dead_code)]
pub fn acpi2_reset_command() -> Result<(), Error> {
    let tables = match ACPI_TABLES.get() {
        None => {
            print_fail_msg!();
            panic!("ACPI tables not initialized!");
        }
        Some(x) => x,
    };

    if acpi_get_revision() != AcpiRevision::Acpi20 {
        return Err(Error);
    }

    let facp_ptr: VirtAddr = match acpi_get_sdt_table(ACPISignature::FADT) {
        Some(x) => x,
        None => return Err(Error),
    };

    let fadt: &FADT = FADT::new_from_ptr(facp_ptr);

    //check if this feature is supported
    if (fadt.flags >> 10) & 0x01 != 1 {
        return Err(Error);
    }

    unsafe {
        outb(fadt.reset_reg.address as u16, fadt.reset_value);
    }
    Ok(())
}

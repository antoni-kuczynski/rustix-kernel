/*
 * Created by Antoni Kuczyński
 * 03/11/2025
 */
use alloc::string::String;
use core::ptr::slice_from_raw_parts;
use crate::asm::{inw, outb};
use crate::drivers::acpi::acpi_sdt::ACPISDTHeader;
use crate::drivers::acpi::acpi_tables::{RSDT, XSDT};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::interrupts::hardware::pic8259::{get_current_time_millis};
use crate::{vgaprint, vgaprintln};
// ============================================================
//               **FADT FIND**
// ============================================================

#[allow(non_snake_case)]
pub fn find_FADT_address_from_rsdt(rsdt: &RSDT, mem_offset: u64) -> Option<u64> {
    let length = (rsdt.header.length as usize - size_of_val(&rsdt.header)) >> 2;
    for i in 0..length {
        let header = ACPISDTHeader::new_from_ptr_u64(rsdt.other_sdt_pointers[i] as u64 + mem_offset);
        if &header.signature == b"FACP" {
            return Some(rsdt.other_sdt_pointers[i] as u64 + mem_offset);
        }
    }
    None
}

#[allow(non_snake_case)]
pub fn find_FADT_address_from_xsdt(xsdt: &XSDT, mem_offset: u64) -> Option<u64> {
    let length = (xsdt.header.length as usize - size_of_val(&xsdt.header)) >> 3;
    for i in 0..length {
        let header = ACPISDTHeader::new_from_ptr_u64(xsdt.other_sdt_pointers[i] + mem_offset);
        if String::from_utf8_lossy(&header.signature) == "FACP" {
            return Some(xsdt.other_sdt_pointers[i] + mem_offset);
        }
    }
    None
}
// ============================================================
//               **FADT STRUCT**
// ============================================================
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct FADT {
    pub h: ACPISDTHeader,
    pub firmware_ctrl: u32,
    pub dsdt: u32,

    //fields used in ACPI 1.0, for compatibility only
    pub reserved: u8,

    pub preferred_power_management_profile: u8,
    pub sci_interrupt: u16,
    pub smi_command_port: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4bios_req: u8,
    pub pstate_control: u8,
    pub pm1a_event_block: u32,
    pub pm1b_event_block: u32,
    pub pm1a_control_block: u32,
    pub pm1b_control_block: u32,
    pub pm2_control_block: u32,
    pub pm_timer_block: u32,
    pub gpe0_block: u32,
    pub gpe1_block: u32,
    pub pm1_event_length: u8,
    pub pm1_control_length: u8,
    pub pm2_control_length: u8,
    pub pm_timer_length: u8,
    pub gpe0_length: u8,
    pub gpe1_length: u8,
    pub gpe1_base: u8,
    pub cstate_control: u8,
    pub worst_c2_latency: u16,
    pub worst_c3_latency: u16,
    pub flush_size: u16,
    pub flush_stride: u16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alarm: u8,
    pub month_alarm: u8,
    pub century: u8,

    // reserved in ACPI 1.0; used since ACPI 2.0+
    pub boot_architecture_flags: u16,

    pub reserved2: u8,
    pub flags: u32,

    //12byte structure
    pub reset_reg: GenericAddressStructure,

    pub reset_value: u8,
    pub reserved3: [u8; 3],

    // 64bit pointers - Available on ACPI 2.0+
    pub x_firmware_control: u64,
    pub x_dsdt: u64,

    pub x_pm1a_event_block: GenericAddressStructure,
    pub x_pm1b_event_block: GenericAddressStructure,
    pub x_pm1a_control_block: GenericAddressStructure,
    pub x_pm1b_control_block: GenericAddressStructure,
    pub x_pm2_control_block: GenericAddressStructure,
    pub x_pm_timer_block: GenericAddressStructure,
    pub x_gpe0_block: GenericAddressStructure,
    pub x_gpe1_block: GenericAddressStructure,
}


impl FADT {
    pub fn new_from_ptr(ptr: u64) -> &'static FADT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let rsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                (length - size_of_val(&header)) >> 2,
            );

            &*(rsdt_ptr as *const FADT)
        }
    }

    pub fn print(&self) {
        let a = self.smi_command_port;
        let b = self.pm1a_control_block;
        unsafe {
            vgaprintln!("FADT smi command port:");
            vgaprintln!("{}", inw(a as u16));
            vgaprintln!("FADT pm1a control block:");
            vgaprintln!("{}", inw(b as u16));
        }
    }

    pub fn enable_acpi(&self) {
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
                VGAWRITER.lock().change_foreground_color(ColorTextMode::Red);
                vgaprintln!(" FAIL!");
                VGAWRITER.lock().change_foreground_color(ColorTextMode::White);
                return;
            }
        }
        VGAWRITER.lock().change_foreground_color(ColorTextMode::Green);
        vgaprintln!(" OK!");
        VGAWRITER.lock().change_foreground_color(ColorTextMode::White);
    }
}
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;

use core::{panic::PanicInfo};
use core::fmt::Error;
use bootloader::{entry_point, BootInfo};
use crate::drivers::acpi;
use crate::drivers::acpi::acpi::enable_acpi;
use crate::drivers::acpi::acpi_tables::{ACPITables, AcpiError};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::memory::mapping::BootInfoFrameAllocator;
use crate::memory::pages;

mod drivers;
mod interrupts;
mod memory;
mod bootinfo;
pub mod asm;

entry_point!(_start);
fn _start(boot_info: &'static BootInfo) -> ! {
    bootinfo::show_vitals(&boot_info);

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let mut _offset_page_table = pages::init(&boot_info);

    let mut _fa = BootInfoFrameAllocator::init(&boot_info.memory_map);

    memory::gallocator::init(&mut _offset_page_table,&mut _fa)
        .expect("heap init failed");

    let tables = match acpi::acpi_tables::get_acpi_tables(&boot_info) {
        Ok(a) => {a}
        Err(AcpiError::InvalidRsdtMappingsError) => {panic!("No ACPI tables found!")}
        Err(AcpiError::InvalidRevisionError) => {panic!("Invalid ACPI revision number!")},
        Err(AcpiError::InvalidChecksumError(x)) => {panic!("Invalid checksum for {}", x.as_str())},
        Err(AcpiError::InvalidSdpChecksumError()) => todo!()
    };
    enable_acpi(tables).expect("Enabling ACPI failed!");


    loop{
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().change_foreground_color(ColorTextMode::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop{
        x86_64::instructions::hlt();
    }
}

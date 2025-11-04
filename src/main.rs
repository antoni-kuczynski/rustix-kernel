#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;

use core::{panic::PanicInfo};
use bootloader::{entry_point, BootInfo};
use crate::drivers::acpi;
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

    let _acpi_tables = acpi::acpi_tables::initialize_acpi_tables(&boot_info);
    acpi::acpi::init(_acpi_tables);

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

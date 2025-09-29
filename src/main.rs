#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */
use core::{panic::PanicInfo};
use bootloader::{entry_point, BootInfo};

use crate::{drivers::vga::{Color, VGAWRITER}, memory::pages };

mod drivers;
mod interrupts;
mod memory;
mod bootinfo;

entry_point!(_start);
fn _start(boot_info: &'static BootInfo) -> ! {
    bootinfo::show_vitals(&boot_info);

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let _offset_page_table = pages::init(&boot_info);

    vgaprintln!("nie wyjebalo sie jupi");

    loop{
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().change_foreground_color(Color::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop {}
}

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;

use crate::graphics::graphics::PointUnsigned;
use crate::graphics::graphics::Rectangle;
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::graphics::graphics::Graphics;
use crate::memory::mapping::BootInfoFrameAllocator;
use crate::memory::pages;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use crate::drivers::vga::CURRENT_VGA_MODE;
use crate::graphics::vga_demo::{vga_demo};

mod drivers;
mod interrupts;
mod memory;
mod bootinfo;
mod graphics;
mod asm;

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

    CURRENT_VGA_MODE.lock().switch_to(0x03);

    // test_offscreen_primitives();
    let g: Graphics = Graphics::new();
    vga_demo(g);

    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().init_vga_text_mode_03h();  //on panic switch to text mode
    VGAWRITER.lock().change_foreground_color(ColorTextMode::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop{
        x86_64::instructions::hlt();
    }
}

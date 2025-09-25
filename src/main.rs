#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

use core::panic::PanicInfo;

use crate::{drivers::vga::{Color, VGAWRITER}};

mod drivers;
mod interrupts;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

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

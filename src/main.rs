#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

use core::panic::PanicInfo;

use crate::drivers::vga::{Color, VGAWRITER};

mod drivers;
mod interrupts;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {

    interrupts::exceptions::init_idt();
    interrupts::gdt::init_gdt();

    loop{}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().change_foreground_color(Color::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop {}
}

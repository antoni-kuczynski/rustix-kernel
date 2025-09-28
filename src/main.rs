#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */
use core::{panic::PanicInfo};
use bootloader::{entry_point, BootInfo};
use x86_64::VirtAddr;
use crate::{drivers::vga::{Color, VGAWRITER}, memory::pages::{self, v_to_p, }};

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

    pages::init(&boot_info);

    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        boot_info.physical_memory_offset +2,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = v_to_p(virt);
        vgaprintln!("{:?} -> {:?}", virt, phys);
    }

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

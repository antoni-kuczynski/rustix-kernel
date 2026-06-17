#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc;

pub mod asm;
mod boot;
mod drivers;
mod interrupts;
mod memory;
mod graphics;
mod video;

use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use core::panic::PanicInfo;
use crate::boot::cpuid::cpuid_init;
use crate::boot::multiboot::{multiboot2_init};
use crate::drivers::apic::apic::{apic_bsp_init, timer_lapic_sleep};
use crate::interrupts::gdt::gdt_init;
use crate::interrupts::idt_init;
use crate::memory::dir_mapping::dir_mapping_init;
use crate::memory::dma::dma_init;
use crate::memory::eba::eba_init;
use crate::memory::ioremap::ioremap_init;
use crate::memory::kheap::kheap_init;
use crate::memory::pmm::pmm_init;
use crate::memory::secure_stack::switch_to_secure_stack;
use crate::video::console::{put_string_no_bg};
use crate::video::framebuffer::{double_buffering_init, fb_swap_buffers, framebuffer_init, FramebufferColor};

unsafe extern "C" {
    static endKernel: u32;
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

fn kernel_main_post_stack() -> ! {
    interrupts::interrupts_enable();


    fn color_wheel_smooth(mut pos: u16) -> (u8, u8, u8) {
        pos %= 1530;

        let phase = pos / 255;

        let offset = (pos % 255) as u8;
        let inv_offset = 255 - offset;

        match phase {
            0 => (255, offset, 0),
            1 => (inv_offset, 255, 0),
            2 => (0, 255, offset),
            3 => (0, inv_offset, 255),
            4 => (offset, 0, 255),
            5 => (255, 0, inv_offset),
            _ => (0, 0, 0),
        }
    }

    let mut i = 0;
    loop {
        let color_pos = (i as u16 * 50) % 1530;
        let (r, g, b) = color_wheel_smooth(color_pos);
        let current_color = FramebufferColor::from_rgb(r as u32, g as u32, b as u32);

        put_string_no_bg("hey\n", &current_color);
        i += 1;
        fb_swap_buffers();
        timer_lapic_sleep(50);
    }


    loop {
        x86_64::instructions::hlt();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    idt_init();
    gdt_init();

    eba_init();
    cpuid_init();
    multiboot2_init();
    pmm_init();
    dir_mapping_init();
    framebuffer_init();

    kheap_init();
    dma_init();
    ioremap_init();

    double_buffering_init();
    // acpi_init();
    apic_bsp_init();
    // pci_init();

    switch_to_secure_stack()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER
        .lock()
        .change_foreground_color(ColorTextMode::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop {
        x86_64::instructions::hlt();
    }
}

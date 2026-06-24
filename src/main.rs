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
pub mod video;
mod gop_demo;
mod misc;

use alloc::string::String;
use alloc::vec;
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
use crate::memory::pat::pat_init;
use crate::memory::pmm::pmm_init;
use crate::memory::secure_stack::switch_to_secure_stack;
use crate::misc::prng::prng_init;
use crate::video::console::{fb_put_string_no_bg, fb_set_foreground};
use crate::video::framebuffer::{double_buffering_init, fb_plot_pixel, fb_swap_buffers, framebuffer_init, FramebufferColor, FRAMEBUFFER_PIXEL_INFO};
use crate::video::kprint::{early_fb_buffer_init, BUF};
// p rustix::video::framebuffer::FRAMEBUFFER

unsafe extern "C" {
    static endKernel: u32;
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    fb_set_foreground(FramebufferColor::from_rgb(220,0,0));
    kprintln_panic!("===============================================");
    kprintln_panic!("Kernel panic!");
    if let Some(location) = _info.location() {
        kprintln_panic!("Panicked at {}", location);
    }
    kprintln_panic!("With message: {}", _info.message());
    kprintln_panic!("===============================================");
    loop {
        x86_64::instructions::hlt();
    }
}

fn kernel_main_post_stack() -> ! {
    interrupts::interrupts_enable();

    gop_demo::demo();

    loop {
        x86_64::instructions::hlt();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    eba_init();
    multiboot2_init();
    pat_init();
    framebuffer_init();

    idt_init();
    gdt_init();

    cpuid_init();
    pmm_init();
    dir_mapping_init();

    kheap_init();
    dma_init();
    ioremap_init();

    double_buffering_init();
    // acpi_init();
    apic_bsp_init();
    // pci_init();

    prng_init();

    switch_to_secure_stack()
}

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
mod misc;

use alloc::string::ToString;
use core::panic::PanicInfo;
use crate::boot::cpuid::cpuid_init;
use crate::boot::multiboot::multiboot2_init;
use crate::drivers::acpi::acpi_tables::acpi_init;
use crate::drivers::apic::apic::apic_bsp_init;
use crate::drivers::pci::pci::pci_init;
use crate::interrupts::gdt::gdt_init;
use crate::interrupts::{idt_init, interrupts_enable};
use crate::memory::dir_mapping::dir_mapping_init;
use crate::memory::dma::dma_init;
use crate::memory::eba::eba_init;
use crate::memory::ioremap::ioremap_init;
use crate::memory::kheap::kheap_init;
use crate::memory::pat::pat_init;
use crate::memory::pmm::pmm_init;
use crate::memory::secure_stack::switch_to_secure_stack;
use crate::misc::prng::prng_init;
use crate::video::framebuffer::{double_buffering_init, framebuffer_init, FramebufferColor, FRAMEBUFFER, __FB_PANIC_ONLY};
use crate::video::kprint::early_text_buffer_init;

unsafe extern "C" {
    static endKernel: u32;
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    //something else is using the framebuffer
    if FRAMEBUFFER.is_locked() {
        emergency_panic(_info);
    }


    let mut fb_lock = FRAMEBUFFER.lock();
    let fb_mut = fb_lock.as_mut();
    if fb_mut.is_none() {
        loop {
            x86_64::instructions::hlt();
        }
    }
    let fb = fb_mut.unwrap();
    fb.set_foreground(FramebufferColor::from_rgb(220, 0, 0));
    unsafe { FRAMEBUFFER.force_unlock() }; //no deadlocks in kprint
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

/// This happens when the framebuffer mutex is locked - something else is using it
fn emergency_panic(_info: &PanicInfo) {
    //we use a second mutex only for emergency panics to prevent deadlock
    //who cares about safety at this point, i just wanna see stuffff!
    //i probably could've just unlocked the mutex/force write into it,
    //but it's only possible if defined as mut (i dont wanna do that)
    let mut fb_lock = __FB_PANIC_ONLY.lock();
    if fb_lock.is_none() {
        loop {
            x86_64::instructions::hlt();
        }
    }
    let fb = fb_lock.as_mut().unwrap();

    fb.set_background(FramebufferColor::from_rgb(0,0,255));
    let panic_str = "KERNEL PANIC!";
    let mut pos_x = 10;
    let mut pos_y = 10;
    for c in panic_str.chars() {
        fb.putchar(pos_x,pos_y, c, false);
        pos_x += 14;
    }

    pos_y += 14;
    pos_x = 10;

    if let Some(location) = _info.location() {
        let location_string = location.to_string();

        for c in location_string.chars() {
            fb.putchar(pos_x,pos_y, c, false);
            pos_x += 14;
        }
    }

    pos_y += 14;
    pos_x = 10;

    if let Some(message_str) = _info.message().as_str() {
        for c in message_str.chars() {
            if pos_x > fb.width() {
                pos_x = 10;
                pos_y += 14;
            }

            fb.putchar(pos_x,pos_y, c, false);
            pos_x += 14;
        }
    }

    if fb.is_double_buffered {
        fb.swap_buffers();
    }

    loop {
        x86_64::instructions::hlt();
    }
}

fn kernel_main_post_stack() -> ! {
    interrupts_enable();

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
    early_text_buffer_init();

    idt_init();
    gdt_init();
    cpuid_init();

    pmm_init();
    dir_mapping_init();
    kheap_init();
    double_buffering_init();
    dma_init();
    ioremap_init();

    acpi_init();
    apic_bsp_init();
    pci_init();

    prng_init();

    switch_to_secure_stack()
}

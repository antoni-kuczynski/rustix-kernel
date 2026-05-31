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

use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use core::panic::PanicInfo;
use crate::boot::cpuid::cpuid_init;
use crate::boot::multiboot::{multiboot2_init};
use crate::drivers::acpi::acpi_tables::acpi_init;
use crate::drivers::apic::apic::apic_bsp_init;
use crate::drivers::pci::pci::pci_init;
use crate::interrupts::gdt::gdt_init;
use crate::interrupts::idt_init;
use crate::memory::dir_mapping::dir_mapping_init;
use crate::memory::dma::dma_init;
use crate::memory::eba::eba_init;
use crate::memory::ioremap::ioremap_init;
use crate::memory::kheap::kheap_init;
use crate::memory::pmm::pmm_init;
use crate::memory::secure_stack::switch_to_secure_stack;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".multiboot2_header")]
#[used]
pub static MULTIBOOT2_HEADER: [u32; 6] = [
    0xE85250D6,                 // magic
    0,                          // architecture
    24,                         // header length
    !(0xE85250D6 + 0 + 24) + 1, // checksum
    0,                          // end tag type
    8,                          // end tag size
];

unsafe extern "C" {
    static endKernel: u32;
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

fn kernel_main_post_stack() -> ! {
    interrupts::interrupts_enable();

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

    kheap_init();
    dma_init();
    ioremap_init();

    acpi_init();
    apic_bsp_init();
    pci_init();

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

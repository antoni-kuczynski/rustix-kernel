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
use core::ptr;
use crate::boot::cpuid::cpuid_init;
use crate::boot::multiboot::{multiboot2_get_framebuffer_info, multiboot2_init};
use crate::drivers::acpi::acpi::acpi_soft_off_state;
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

unsafe extern "C" {
    static endKernel: u32;
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

fn kernel_main_post_stack() -> ! {
    interrupts::interrupts_enable();

    unsafe {
        let fb_tag = multiboot2_get_framebuffer_info().expect("framebuffer tag not found");
        // ptr::write_volatile((fb_tag.base as *mut u32).add(0), 0x000000ff); //blue
        // ptr::write_volatile((fb_tag.base as *mut u32).add(1), 0x0000ff00); //red
        // ptr::write_volatile((fb_tag.base as *mut u32).add(2), 0xff000000);
        // ptr::write_volatile((fb_tag.base as *mut u32).add(3), 0xffffffff); //black
        // ptr::write_volatile((fb_tag.base as *mut u32).add(4), 0x00000000); //white

        //format BGRx

        let mut p = (fb_tag.base) as *mut u8;


        for i in 0..767 {
            for j in (0..1023).step_by(4) {
                let off = j * 3;

                // pixel 2 RED
                *p.add(off + 0) = 0x00;
                *p.add(off + 1) = 0x00;
                *p.add(off + 2) = 0xff;

                // pixel 1 GREEN
                *p.add(off + 3) = 0x00;
                *p.add(off + 4) = 0xff;
                *p.add(off + 5) = 0x00;

                // pixel 0 BLUE
                *p.add(off + 6) = 0xff;
                *p.add(off + 7) = 0x00;
                *p.add(off + 8) = 0x00;

                // pixel 3 WHITE
                *p.add(off + 9) = 0xff;
                *p.add(off + 10) = 0xff;
                *p.add(off + 11) = 0xff;
            }

            p = p.add(fb_tag.pitch);
        }

        //
        // core::ptr::write_volatile(p.add(0), 0xff);
        // core::ptr::write_volatile(p.add(1), 0x00);
        // core::ptr::write_volatile(p.add(2), 0x00);
        // core::ptr::write_volatile(p.add(3), 0x00);

        // acpi_soft_off_state().expect("TODO: panic message");

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

    kheap_init();
    dma_init();
    ioremap_init();

    // acpi_init();
    // apic_bsp_init();
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

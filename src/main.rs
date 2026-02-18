#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

// extern crate alloc;

// use core::{panic::PanicInfo};
// use crate::drivers::acpi::acpi::{acpi2_reset_command, enable_acpi};
// use crate::drivers::acpi::acpi_tables::{get_acpi_tables};
// use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
// use crate::interrupts::hardware::pic8259::sleep;
// use crate::memory::pages;

mod drivers;
mod interrupts;
// mod memory;
// mod bootinfo;
pub mod asm;
mod boot;
mod memory;
// mod graphics;

// entry_point!(_start);
// fn _start(boot_info: &'static BootInfo) -> ! {
//     bootinfo::show_vitals(&boot_info);
//
//     interrupts::init_idt();
//     interrupts::gdt::init_gdt();
//     interrupts::hardware::pic8259::init_pics();
//     interrupts::enable();
//
//     let mut _offset_page_table = pages::init(&boot_info);
//     let mut _fa = BootInfoFrameAllocator::init(&boot_info.memory_map);
//     memory::gallocator::init(&mut _offset_page_table,&mut _fa)
//         .expect("heap init failed");
//
//     let tables = get_acpi_tables(&boot_info).expect("Acpi tables init failed!");
//     enable_acpi(&tables).expect("Enabling ACPI failed!");
//
//     sleep(2000);
//     acpi2_reset_command(&tables).expect("failed to acpi reset the pc");
//
//     loop{
//         x86_64::instructions::hlt();
//     }
// }

use core::arch::asm;
use core::panic::PanicInfo;
use crate::boot::multiboot::{MultibootInfoView};
use crate::drivers::vga::vga_text::{ColorTextMode, VgaTextMode, VGAWRITER};

pub struct BootInfo {
    pub physical_memory_offset: u64
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".multiboot2_header")]
#[used]
pub static MULTIBOOT2_HEADER: [u32; 6] = [
    0xE85250D6, // magic
    0,          // architecture
    24,         // header length
    !(0xE85250D6 + 0 + 24) + 1, // checksum
    0,          // end tag type
    8,          // end tag size
];

unsafe extern "C" {
    static endKernel: u32;
}

const PHYS_BASE: u32 = 0x00100000;
const KERNEL_OFFSET: u64 = 0xFFFFFFFF80000000;

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    let kernel_offset = KERNEL_OFFSET;
    let phys_base = PHYS_BASE;
    let end_kernel = unsafe {&endKernel as *const u32 as u64};

    let multiboot_addr = MultibootInfoView::get_multiboot_address_from_ebx();
    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let multiboot_info = MultibootInfoView::new(multiboot_addr);
    vgaprintln!("==============================");
    vgaprintln!("Bootloader name: {}", multiboot_info.get_boot_loader_name().unwrap());
    // multiboot_info.print_memory_map();
    vgaprintln!("Kernel physical base: {:#06x}", phys_base);
    vgaprintln!("Kernel logical offset: {:#011x}", kernel_offset);
    vgaprintln!("Kernel physical end: {:#011x}", end_kernel);


    unsafe {
        // let kernel_end = &endKernel as *const u32 as u64;
        vgaprintln!("==============================");
        // vgaprintln!("Kernel start: {:#011x}, Kernel end: {:#011x}", KERNEL_START, kernel_end);
        // vgaprintln!("Kernel: {}KB, Total memory: {}MB", (kernel_end - KERNEL_START) / 1024, (multiboot_info.get_available_memory_bytes().unwrap() / 1024) / 1024);
    }






    // memory::pmm::init(&multiboot_info).expect("pmm init failed");
    // memory::paging::init(&multiboot_info).expect("TODO: panic message");



    loop {
        x86_64::instructions::hlt();
    }


    //
    // let mut _offset_page_table = pages::init(&boot_info);
    // let mut _fa = BootInfoFrameAllocator::init(&boot_info.memory_map);
    // memory::gallocator::init(&mut _offset_page_table,&mut _fa)
    //     .expect("heap init failed");
    //
    // let tables = get_acpi_tables(&boot_info).expect("Acpi tables init failed!");
    // enable_acpi(&tables).expect("Enabling ACPI failed!");
    //
    // sleep(2000);
    // acpi2_reset_command(&tables).expect("failed to acpi reset the pc");
    //
    // loop{
    //     x86_64::instructions::hlt();
    // }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().change_foreground_color(ColorTextMode::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop{
        x86_64::instructions::hlt();
    }
}

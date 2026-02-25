#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

mod drivers;
mod interrupts;
pub mod asm;
mod boot;
mod memory;


use core::panic::PanicInfo;
use crate::boot::multiboot::{MultibootInfoView};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::memory::{P2V, PHYS_BASE, VIRT_BASE};

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

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    let kernel_offset = VIRT_BASE;
    let phys_base = PHYS_BASE;
    let end_kernel = unsafe {&endKernel as *const u32 as u64};

    let multiboot_addr: u64 = P2V(MultibootInfoView::get_multiboot_address_from_ebx() as u64);
    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();



    unsafe {
        let multiboot_info = MultibootInfoView::new(multiboot_addr);
        let memory_tag = multiboot_info.get_memory_map_tag().unwrap();

        vgaprintln!("==============================");
        vgaprintln!("Boot info:");
        vgaprintln!("==============================");
        vgaprintln!("Bootloader name: {}", multiboot_info.get_boot_loader_name().unwrap());
        vgaprintln!("Kernel physical base: {:#06x}", phys_base);
        vgaprintln!("Kernel logical offset: {:#011x}", kernel_offset);
        vgaprintln!("Kernel physical end: {:#011x}", end_kernel);
        vgaprintln!("Available memory: {}mb", (*memory_tag).get_available_memory_bytes() / 1048576);
        vgaprintln!("==============================");

    }

    //0xffffffff80103441

    // memory::pmm::init(&multiboot_info).expect("pmm init failed");
    // memory::paging::init(&multiboot_info).expect("TODO: panic message");



    loop {
        x86_64::instructions::hlt();
    }
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

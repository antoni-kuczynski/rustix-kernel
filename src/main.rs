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

use core::panic::PanicInfo;
use crate::drivers::vga::vga_text::VgaTextMode;

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

pub fn cpu_in_long_mode() -> bool {
    let (low, _high): (u32, u32);

    unsafe {
        core::arch::asm!(
        "mov ecx, 0xC0000080", // IA32_EFER MSR
        "rdmsr",               // read into EDX:EAX
        out("eax") low,
        out("edx") _high,
        out("ecx") _,          // clobbered
        );
    }

    //bit 10 = LMA (Long Mode Active)
    (low & (1 << 10)) != 0
}



#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    let vga = 0xb8000 as *mut u8;
    if cpu_in_long_mode() {
        unsafe {
            *vga.offset(0) = b'Y';
            *vga.offset(1) = 0x0F;
            *vga.offset(2) = b'E';
            *vga.offset(3) = 0x0F;
            *vga.offset(4) = b'S';
            *vga.offset(5) = 0x0F;
        }
    } else {
        unsafe {
            *vga.offset(0) = b'N';
            *vga.offset(1) = 0x0F;
            *vga.offset(2) = b'O';
            *vga.offset(3) = 0x0F;
        }
    }

    loop {
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
    loop {
        x86_64::instructions::hlt();
    }
}

// #[panic_handler]
// fn panic(_info: &PanicInfo) -> ! {
//     VGAWRITER.lock().change_foreground_color(ColorTextMode::LightRed);
//     vgaprintln!("=!==============================!=");
//     vgaprintln!("Kernel panic! \n{}", _info);
//     vgaprintln!("=!==============================!=");
//     loop{
//         x86_64::instructions::hlt();
//     }
// }

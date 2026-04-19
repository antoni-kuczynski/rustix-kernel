#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod drivers;
mod interrupts;
pub mod asm;
mod boot;
mod memory;
// mod graphics;

use core::panic::PanicInfo;
use crate::boot::multiboot::{multiboot2_bootloader_name, multiboot2_logical_end, multiboot2_memory_map_tag, MultibootInfo, MULTIBOOT_INFO};
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::memory::{SizeUnit, _P2V_kernel, KERNEL_PHYS_BASE, KERNEL_VIRT_BASE};
use crate::memory::pmm::{PMM_BITMAP};

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
    static earlyHeapStart: u64;
    static earlyHeapEnd: u64;
    static __oldMultibootPhysAddr: u32;
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    interrupts::idt_init();
    interrupts::gdt::gdt_init();
    interrupts::hardware::pic8259::pics_init();

    memory::eba::eba_init();
    boot::cpuid::cpuid_init();
    boot::multiboot::multiboot2_init();
    memory::pmm::pmm_init();
    memory::dir_mapping::dir_mapping_init();


    interrupts::enable();

    unsafe {
        let kernel_offset = KERNEL_VIRT_BASE;
        let phys_base = KERNEL_PHYS_BASE;
        let end_kernel = &endKernel as *const u32 as u64;

        // let a = PhysAddr::new(0x01000000);
        // let virt = physical_to_virtual(a);
        // let phys = virtual_to_physical(virt);
        //
        // vgaprintln!("Original: {:#011x}", a);
        // vgaprintln!("Virt: {:#011x}", virt.as_u64());
        // vgaprintln!("Phys: {:#011x}", phys.unwrap().as_u64());
        // *(virt.as_u64() as *mut u32) = 0xdeaddead;
        // vgaprintln!("Val1: {:#011x}", *(virt.as_u64() as *mut u32));
        // *(virt.as_u64() as *mut u32) = 0xdeadc0de;
        // vgaprintln!("Val2: {:#011x}", *(virt.as_u64() as *mut u32));



        // print_page_table_tree(kernel_offset as u64);

        let memory_tag = multiboot2_memory_map_tag().unwrap();

        vgaprintln!("=========KERNEL INFO==========");
        vgaprintln!("Kernel LOGICAL end:       {:#011x}", _P2V_kernel(end_kernel));
        vgaprintln!("Kernel PHYSICAL base:      {:#06x}", phys_base);
        vgaprintln!("Kernel PHYS2VIRT offset:   {:#011x}", kernel_offset);
        vgaprintln!();
        vgaprintln!("=======EARLY HEAP INFO========");
        vgaprintln!("EH VIRTUAL start:  {:#011x}", _P2V_kernel(earlyHeapStart));
        vgaprintln!("EH VIRTUAL end:    {:#011x}", _P2V_kernel(earlyHeapEnd));
        vgaprintln!();
        vgaprintln!("=========MEMORY INFO==========");
        vgaprintln!("Available memory:  {}mb", (*memory_tag).get_available_memory(SizeUnit::Megabyte));
        vgaprintln!("Bitmap size:   {}kb", PMM_BITMAP.lock().length() / SizeUnit::Kilobyte.as_u64());
        vgaprintln!();
        vgaprintln!("=======MULTIBOOT INFO=========");
        vgaprintln!("Multiboot length: {}b", MULTIBOOT_INFO.get().unwrap().length());
        vgaprintln!("Multiboot start VIRTUAL: {:#011x}", MULTIBOOT_INFO.get().unwrap().base() as *const MultibootInfo as u64);
        vgaprintln!("Multiboot end VIRTUAL: {:#011x}", multiboot2_logical_end().as_u64());
        vgaprintln!("Bootloader name: {}", multiboot2_bootloader_name().unwrap());


        // vgaprintln!("0 = free | 1 = used");
        // PMM_BITMAP.lock().print(540);

        // let mut modules = multiboot_info.get_modules_tag(multiboot_info.tags);
        //
        // while modules != None {
        //     let module = modules.unwrap();
        //     (*module).print();
        //     let start_ptr = module.byte_add((((*module).header().size() + 7) & !0x7) as usize);
        //     modules = multiboot_info.get_modules_tag(start_ptr as *const u32);
        // }
        // (*multiboot_info.get_memory_map_tag().unwrap()).print_memory_map();


    }
    // let tables = get_acpi_tables(&boot_info).expect("Acpi tables init failed!");
    // enable_acpi(&tables).expect("Enabling ACPI failed!");
    //
    // sleep(2000);
    // acpi2_reset_command(&tables).expect("failed to acpi reset the pc");
    //
    // pci::pci_init();
    //
    // loop{
    //     x86_64::instructions::hlt();
    // }

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

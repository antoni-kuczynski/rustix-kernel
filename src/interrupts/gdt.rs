
/*
 * Created by Oskar Przybylski
 * 23/09/2025
 *
 * TSS is a legacy structure that was used for hardware context switching
 * Now it contains two stack tables (PST and IST).  Interrupt Stack table
 * allows us to swich stacks when excetions occur, and prevent triple faults 
 * from happening for ex. from kernel stack overflow.
 *
 * TSS has following format:
 * Field        Type
 * Reserved     u32
 * PST          [u64;3]
 * Reserved     u64
 * IST          [u64;7]
 * Reserved     u64
 * Reserved     u16
 * I/O MBA      u16
 *
 * To make our TSS structure visible for CPU we need to insert it into
 * GDT (Global Descritor Table). GDT is used for two things: Switching
 * between user and kernel space and loading a TSS structure
*/

use lazy_static::lazy_static;
use x86_64::{instructions::tables::load_tss, registers::segmentation::{Segment, CS}, structures::{gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector}, tss::TaskStateSegment}, VirtAddr};

use crate::{drivers::vga::{Color, VGAWRITER}, vgaprint, vgaprintln};

pub const DOUBLE_FAULT_IST_INDEX : u16 = 0;

pub fn init_gdt() {
    vgaprint!("Initlializing global descriptor table...");

    GDT.0.load(); // load gdt 

    unsafe { // it might break memory safety with invalid selectors
        CS::set_reg(GDT.1.code_selector); // set code selector register 
        load_tss(GDT.1.tss_selector);     // set tss selector
    }

    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

lazy_static!{
    static ref TSS: TaskStateSegment = {
        // initialize the tss struct
        let mut tss = TaskStateSegment::new();

        // add entry for double fault exception
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize]= {
            // setting stack for double fault
            const STACK_SIZE: u64 = 4096 * 16; // 64kB of memory for DOUBLE_FAULT stack
            static mut STACK: [u8; STACK_SIZE as usize] = [0;STACK_SIZE as usize];
            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };

        // return the tss
        tss
    };

    static ref GDT: (GlobalDescriptorTable,Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        // tell CPU where kernel code is so we dont 
        // get General Protection Fault in protected/long mode
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let tss_selector  = gdt.append(Descriptor::tss_segment(&TSS));

        (gdt, Selectors{code_selector,tss_selector})
    };
}

use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

use crate::{drivers::vga_text::{Color, VGAWRITER}, interrupts::{exceptions::*, gdt::DOUBLE_FAULT_IST_INDEX, hardware::pic8259::{keyboard_interrupt_handler, timer_interrupt_handler, PicInterruptIndex}}, vgaprint, vgaprintln};

pub mod exceptions;
pub mod gdt;
pub mod hardware;

lazy_static!{
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        //exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.invalid_opcode.set_handler_fn(invalid_optcode_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.divide_error.set_handler_fn(division_error_handler);
        unsafe{
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX); // <- this line is unsafe 
                                                          // we have to give valid, unused and
                                                          // initialized stack index
            }

        // interrupts
        idt[PicInterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler);
        idt[PicInterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

pub fn init_idt() {
    vgaprint!("Initlializing interrupt descriptor table...");

    IDT.load();

    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);
}


pub fn enable(){
    vgaprint!("Enabling interrupts...");

    x86_64::instructions::interrupts::enable();

    VGAWRITER.lock().change_foreground_color(Color::Green);
    vgaprintln!(" OK!");
    VGAWRITER.lock().change_foreground_color(Color::White);
}

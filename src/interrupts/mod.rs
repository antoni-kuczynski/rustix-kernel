use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

use crate::{interrupts::{
    exceptions::*,
    gdt::DOUBLE_FAULT_IST_INDEX,
}, kprintln_ok};
use crate::drivers::apic::apic::{apic_error_interrupt_handler, apic_spurious_interrupt_handler, lapic_timer_interrupt_handler, LAPIC_ERROR_VECTOR, LAPIC_SPURIOUS_VECTOR_IDT_INDEX, LAPIC_TIMER_VECTOR};

pub mod exceptions;
pub mod gdt;

lazy_static! {
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
        idt[LAPIC_TIMER_VECTOR].set_handler_fn(lapic_timer_interrupt_handler);
        idt[LAPIC_SPURIOUS_VECTOR_IDT_INDEX].set_handler_fn(apic_spurious_interrupt_handler);
        idt[LAPIC_ERROR_VECTOR].set_handler_fn(apic_error_interrupt_handler);
        idt
    };
}

pub fn idt_init() {
    IDT.load();
    kprintln_ok!("Initialized interrupt descriptor table.");
}

pub fn interrupts_enable() {
    x86_64::instructions::interrupts::enable();
    kprintln_ok!("Enabled interrupts.");
}

use core::cell::UnsafeCell;
use x86_64::structures::idt::InterruptDescriptorTable;

use crate::drivers::apic::apic::{
    LAPIC_ERROR_VECTOR, LAPIC_SPURIOUS_VECTOR_IDT_INDEX, LAPIC_TIMER_VECTOR,
    apic_error_interrupt_handler, apic_spurious_interrupt_handler, lapic_timer_interrupt_handler,
};
use crate::{
    drivers::vga::vga_text::{ColorTextMode, VGAWRITER},
    interrupts::{exceptions::*, gdt::DOUBLE_FAULT_IST_INDEX},
    print_ok_msg, vgaprint,
};

pub mod exceptions;
pub mod gdt;
pub mod router;
pub mod vector;

struct IdtCell(UnsafeCell<InterruptDescriptorTable>);

unsafe impl Sync for IdtCell {}

static IDT: IdtCell = IdtCell(UnsafeCell::new(InterruptDescriptorTable::new()));

unsafe fn init_static_idt(idt: &mut InterruptDescriptorTable) {
    // exceptions
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.invalid_opcode.set_handler_fn(invalid_optcode_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    idt.divide_error.set_handler_fn(division_error_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
    }

    // interrupts
    idt[LAPIC_TIMER_VECTOR].set_handler_fn(lapic_timer_interrupt_handler);
    idt[LAPIC_SPURIOUS_VECTOR_IDT_INDEX].set_handler_fn(apic_spurious_interrupt_handler);
    idt[LAPIC_ERROR_VECTOR].set_handler_fn(apic_error_interrupt_handler);
}

pub fn install_dynamic_idt_route(vector: vector::InterruptVector) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| unsafe {
        let idt = &mut *IDT.0.get();
        let installed = router::install_idt_route(idt, vector);
        if installed {
            idt.load();
        }
        installed
    })
}

pub fn idt_init() {
    vgaprint!("Initializing interrupt descriptor table...");

    unsafe {
        let idt = &mut *IDT.0.get();
        init_static_idt(idt);
        idt.load();
    }

    print_ok_msg!();
}

pub fn interrupts_enable() {
    vgaprint!("Enabling interrupts...");
    print_ok_msg!();
    x86_64::instructions::interrupts::enable();
}

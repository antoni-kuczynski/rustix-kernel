#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 30/05/2026
 */
use crate::asm::outb;

pub mod apic;
mod pit;

pub const PIC1_COMMAND: u16 = 0x20;
pub const PIC1_DATA: u16 = 0x21;
pub const PIC2_COMMAND: u16 = 0xA0;
pub const PIC2_DATA: u16 = 0xA1;

pub const PIC1_VECTOR_OFFSET: u8 = 0x20;
pub const PIC2_VECTOR_OFFSET: u8 = 0x28;

pub const ICW1_INIT: u8 = 0x10;
pub const ICW1_ICW4: u8 = 0x01;
pub const ICW1_INIT_ICW4: u8 = ICW1_INIT | ICW1_ICW4;

pub const ICW3_MASTER_IRQ2: u8 = 0x04;
pub const ICW3_SLAVE_ID_2: u8 = 0x02;

pub const ICW4_8086: u8 = 0x01;

pub const PIC_MASK_ALL: u8 = 0xFF;

unsafe fn disable_pic() {
    outb(PIC1_COMMAND, ICW1_INIT_ICW4);
    outb(PIC2_COMMAND, ICW1_INIT_ICW4);

    outb(PIC1_DATA, PIC1_VECTOR_OFFSET);
    outb(PIC2_DATA, PIC2_VECTOR_OFFSET);

    outb(PIC1_DATA, ICW3_MASTER_IRQ2);
    outb(PIC2_DATA, ICW3_SLAVE_ID_2);

    outb(PIC1_DATA, ICW4_8086);
    outb(PIC2_DATA, ICW4_8086);

    outb(PIC1_DATA, PIC_MASK_ALL);
    outb(PIC2_DATA, PIC_MASK_ALL);
}
#![allow(dead_code)]
/*
 *  Created bt Oskar Przybylski 
 *  24/09/2025
 *
 *  pic8259 have this layout:
 *                        ____________                          ____________
 *   Real Time Clock --> |            |   Timer -------------> |            |
 *   ACPI -------------> |            |   Keyboard-----------> |            |      _____
 *   Available --------> | Secondary  |----------------------> | Primary    |     |     |
 *   Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
 *   Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
 *   Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
 *   Primary ATA ------> |            |   Floppy disk -------> |            |
 *   Secondary ATA ----> |____________|   Parallel Port 1----> |____________|
 *
 *   (ascii art source https://os.phil-opp.com/hardware-interrupts/#the-8259-pic)
 */

use pic8259::ChainedPics;
use spin::{mutex::Mutex};
use x86_64::{structures::idt::InterruptStackFrame};

use crate::{drivers::vga::vga_text::{ColorTextMode, VGAWRITER}, print_ok_msg, vgaprint};

// indexes of pic interrupts handlers in IDT
// Primary
pub const PIC_1_OFFSET: u8 = 32;            // 0-31 are reserved for cpu exceptions
// Secondary
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET+8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe{ // unsafe beacouse wrong offests provided cause undefined behaviour
        ChainedPics::new(PIC_1_OFFSET,PIC_2_OFFSET)
    });

fn end_of_interrupt(id: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(id);
    }
}

pub fn pics_init(){
    vgaprint!("Initializing pic8259 hardware interrupts...");

    unsafe { PICS.lock().initialize(); }

    print_ok_msg!();
}

#[derive(Debug,Clone,Copy)]
#[repr(u8)]
pub enum PicInterruptIndex {
    Timer = PIC_1_OFFSET,        // line 0 on primary pic
    Keyboard = PIC_1_OFFSET + 1, // line 1 on primary pic
}

impl PicInterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// timer interrupt handler
static mut TICKS : u64 = 0;
pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame){
    unsafe { TICKS += 1; };
    end_of_interrupt(PicInterruptIndex::Timer.as_u8());
}

pub fn get_ticks() -> u64 {
    unsafe { TICKS }
}

pub fn get_current_time_millis() -> u64 {
    get_ticks().saturating_mul(TICKS_PER_MS as u64)
}

static TICKS_PER_MS: isize = 55;

pub fn sleep(ms: usize){
    let mut ms_to_pass: isize = ms as isize;
    let mut ticks = get_ticks();
    loop{
        let new_ticks = get_ticks();
        if ticks != new_ticks{
            ticks = new_ticks;
            ms_to_pass-=TICKS_PER_MS;
        }
        if ms_to_pass <= 0{
            return;
        }
    }
}

// keyboard interrupt handler (PS/2)
pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame){
    end_of_interrupt(PicInterruptIndex::Keyboard.as_u8());
}

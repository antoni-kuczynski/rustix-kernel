#![allow(dead_code)]

mod vga_regs;
pub mod vga_io;
// ============================================================
//                     **VGA PORTS**
// ============================================================

// --------------------- *CRT CONTROLLER* ---------------------
const VGA_CRT_CONTROL_INDEX: u16 = 0x03D4;
const VGA_CRT_CONTROL_DATA:  u16 = 0x03D5;

// ------------------ *ATTRIBUTE CONTROLLER* ------------------
const VGA_AC_INDEX:      u16 = 0x03C0;
const VGA_AC_WRITE:      u16 = 0x03C0;  // same as INDEX
const VGA_AC_READ:       u16 = 0x03C1;
const VGA_INSTAT_READ:   u16 = 0x03DA;  // resets AC flip-flop

// ------------------------ *SEQUENCER* -----------------------
const VGA_SEQUENCER_INDEX: u16 = 0x03C4;
const VGA_SEQUENCER_DATA:  u16 = 0x03C5;

// ------------------- *MISC OUTPUT REGISTER* -----------------
const VGA_MISC_OUTPUT_INDEX: u16 = 0x03C2;
const VGA_MISC_OUTPUT_READ:  u16 = 0x03CC;

// -------------------- *GRAPHICS CONTROLLER* -----------------
const VGA_GRAPHICS_CONTROLLER_INDEX: u16 = 0x03CE;
const VGA_GRAPHICS_CONTROLLER_DATA:  u16 = 0x03CF;

// ----------------------- *DAC / PALETTE* --------------------

const VGA_DAC_READ_INDEX:  u16 = 0x03C7;
const VGA_DAC_WRITE_INDEX: u16 = 0x03C8;
const VGA_DAC_DATA:        u16 = 0x03C9;
//---------------------------------------------------------------
use core::arch::asm;
use crate::drivers::vga::registers::*;
use crate::drivers::vga::registers::regs::*;

//  *REG WRITE FUNCTIONS*
pub unsafe fn graphics_controller_write(index: u8, value: u8) {
    unsafe {
        outb(VGA_GRAPHICS_CONTROLLER_INDEX, index);
        outb(VGA_GRAPHICS_CONTROLLER_INDEX + 1, value); // GC_DATA = 0x3CF
    }
}
pub unsafe fn sequcencer_write(index: u8, value: u8) {
    unsafe {
        outb(VGA_SEQUENCER_INDEX, index);
        outb(VGA_SEQUENCER_INDEX + 1, value); // SC_DATA = 0x3C5
    }
}

pub unsafe fn crtc_write(index: u8, value: u8) {
    outb(VGA_CRT_CONTROL_INDEX, index);
    outb(VGA_CRT_CONTROL_DATA, value);
}

pub unsafe fn attribute_controller_write(index: u8, value: u8) {
    unsafe {
        // Reset flip-flop
        inb(VGA_INSTAT_READ);
        // Write index, then data
        outb(VGA_AC_INDEX, index);
        outb(VGA_AC_WRITE, value);
    }
}

pub unsafe fn misc_output_write(value: u8) {
    unsafe {
        outb(VGA_MISC_OUTPUT_INDEX, value);
    }
}

//-------------------------------------
//  **REGISTRY SETTING**
pub fn set_13h_mode_regs() {
    set_reg_values(
        VGA_13H_MISC_OUTPUT_REG,
        VGA_13H_CRT_CONTROL_REGS,
        VGA_13H_SEQUENCER_REGS,
        VGA_13H_GRAPHICS_CONTROLLER_REGS,
        VGA_13H_ATTRIBUTE_CONTROLLER_REGS
    );
}

pub fn set_12h_mode_regs() {
    set_reg_values(
        VGA_12H_MISC_OUTPUT_REG,
        VGA_12H_CRT_CONTROL_REGS,
        VGA_12H_SEQUENCER_REGS,
        VGA_12H_GRAPHICS_CONTROLLER_REGS,
        VGA_12H_ATTRIBUTE_CONTROLLER_REGS
    );
}

pub fn set_03h_mode_regs() {
    set_reg_values(
        VGA_03H_MISC_OUTPUT_REG,
        VGA_03H_CRT_CONTROL_REGS,
        VGA_03H_SEQUENCER_REGS,
        VGA_03H_GRAPHICS_CONTROLLER_REGS,
        VGA_03H_ATTRIBUTE_CONTROLLER_REGS
    );
}

fn set_reg_values(
    vga_misc_output_reg: u8,
    vga_crt_control_regs: [u16; 18],
    vga_sequencer_regs: [u16; 5],
    graphics_controller_regs: [u16; 9],
    attribute_controller_regs: [u8; 21]
) {
    unsafe {
        asm!("cli");

        //Miscellaneous Output Register
        outb(VGA_MISC_OUTPUT_INDEX, vga_misc_output_reg);

        //CRT Control registers
        outw(VGA_CRT_CONTROL_INDEX, vga_crt_control_regs[11]); //first write register 0x11 to unlock regs 0x00 to 0x07
        for (index, reg) in vga_crt_control_regs.iter().enumerate() {
            if index == 11 {
                continue;   //we've already written to that register so skip it
            }
            outw(VGA_CRT_CONTROL_INDEX, *reg);
        }

        //Sequencer registers (0x03C4):
        for reg in vga_sequencer_regs.iter() {
            outw(VGA_SEQUENCER_INDEX, *reg);
        }

        //Graphics controller registers (0xCE):
        for reg in graphics_controller_regs.iter() {
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, *reg);
        }

        //Attribute controller registers:
        for (i, &val) in attribute_controller_regs.iter().enumerate() {
            inb(VGA_INSTAT_READ);   //reset flip-flop
            outb(VGA_AC_INDEX, i as u8);    //select register
            outb(VGA_AC_WRITE, val);        //write value
        }

        //Lock 16-color palette and unblank display
        inb(VGA_INSTAT_READ);
        outb(VGA_AC_INDEX, 0x20);
    }
}
//----------------------------------------------
//  *LOADING COLOR PALLETE INTO DAC*
unsafe fn dac_color_output(r: u8, g: u8, b: u8) {
    unsafe {
        outb(0x03C9, r);
        outb(0x03C9, g);
        outb(0x03C9, b);
    }
}

#[inline(always)]
fn to_dac(val: u8) -> u8 {
    (val >> 2) & 0x3F
}

pub unsafe fn load_8bit_color_pallet_into_dac() {
    unsafe {
        //Unmask DAC palette
        outb(0x03C6, 0xFF);

        //Set the color start index to 0
        outb(0x03C8, 0x00);


        for r in 0..8 {        //3 bits for red
            for g in 0..8 {    //3 bits for green
                for b in 0..4 { //2 bits for blue
                    //Scale to 0..63 (DAC range)
                    let r6 = (r * 63 / 7) as u8;
                    let g6 = (g * 63 / 7) as u8;
                    let b6 = (b * 63 / 3) as u8;

                    dac_color_output(r6, g6, b6);
                    // outb(0x03C9, r6);
                    // outb(0x03C9, g6);
                    // outb(0x03C9, b6);
                }
            }
        }
    }
}

pub unsafe fn load_4bit_color_palette_into_dac() {
    //Standard 16 VGA colors
    let palette: [(u8, u8, u8); 16] = [
        (0,   0,   0),     //Black  0x0
        (0,   0, 168),     //Blue   0x1
        (0, 168,   0),     //Green  0x2
        (0, 168, 168),     //Cyan   0x3
        (168, 0,   0),     //Red    0x4
        (168, 0, 168),     //Magenta    0x5
        (168, 84,  0),     //Brown  0x6
        (168,168,168),     //Light Gray 0x7
        (84, 84,  84),     //Dark Gray  0x8
        (84, 84, 252),     //Light Blue 0x9
        (84, 252, 84),     //Light Green    0xA
        (84, 252,252),     //Light Cyan 0xB
        (252, 84, 84),     //Light Red  0xC
        (252, 84,252),     //Light Magenta  0xD
        (252,168, 84),     //Yellow 0xE
        (252,252,252),     //White  0xF
    ];

    unsafe {
        //Unmask DAC palette
        outb(0x03C6, 0xFF);

        //Start writing at color index 0
        outb(0x03C8, 0x00);

        for &(r, g, b) in &palette {
            dac_color_output(
                to_dac(r),
                to_dac(g),
                to_dac(b)
            );
        }
    }
}
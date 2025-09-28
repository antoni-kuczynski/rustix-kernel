#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 26/09/2025
 */
use core::arch::asm;
use core::ffi::c_uchar;
use crate::drivers::vga_text::Color;

pub unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in ("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

pub unsafe fn inb(port: u16) -> u8 {
    unsafe {
        let value: u8;
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
        value
    }
}

unsafe fn g_set_color(color_number: c_uchar, r: c_uchar, g: c_uchar, b: c_uchar) {
    unsafe {
        outb(0x03C6, 0xff);
        outb(0x03C8, color_number);
        outb(0x03C9, r / 4);
        outb(0x03C9, g / 4);
        outb(0x03C9, b / 4);
    }
}

const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 200;
const BUFFER_LENGTH_BYTES: usize = VIDEO_WIDTH * VIDEO_HEIGHT;

pub struct VgaVideoColor(pub u8);

pub struct VgaVideoMode {
    video_width_px: usize, //res width
    video_height_px: usize, //res height
    color_depth_bits: usize, //color depth
    pitch: usize, //how many bytes of VRAM you should skip to go one pixel down
    pixel_width: usize, //how many bytes of VRAM you should skip to go one pixel right
    mode_value: u8, //the mode value in hex
    video_buffer: &'static mut [VgaVideoColor; VIDEO_WIDTH * VIDEO_HEIGHT]
}

impl VgaVideoMode {
    pub fn put_pixel(&mut self, pos_x: usize, pos_y: usize, color: VgaVideoColor) {
        let location = self.video_width_px * pos_y + pos_x;
        self.video_buffer[location] = color;
    }

    pub fn new_vga_320x200_256_mode() -> Self {
        VgaVideoMode {
            video_width_px: VIDEO_WIDTH,
            video_height_px: VIDEO_HEIGHT,
            color_depth_bits: 8,
            pitch: 320,
            pixel_width: 1,
            mode_value: 0x13,
            video_buffer: unsafe {
                &mut *(0xA0000 as *mut [VgaVideoColor; VIDEO_WIDTH * VIDEO_HEIGHT])
            }
        }
    }

    pub fn init_mode(&mut self) {
        unsafe {
            asm!("cli");

            //Miscellaneous Output Register
            outb(0x03C2, 0x43);

            //CRT Control registers
            //Vertical Retrace End Register (index 0x11, set first to unlock registers 0x0 to 0x07)
            outw(0x03D4, 0x0E11);

            //Horizontal total register (index 0x00)
            outw(0x03D4, 0x5F00);

            //Horizontal Display-Enable End Register (index 0x01)
            outw(0x03D4, 0x4F01);

            //Start Horizontal Blanking Register (index 0x02)
            outw(0x03D4, 0x5002);

            //End Horizontal Blanking Register (index 0x03)
            outw(0x03D4, 0x8203);

            //Start Horizontal Retrace Pulse Register (index 0x04)
            outw(0x03D4, 0x5404);

            //End Horizontal Retrace Register (index 0x05)
            outw(0x03D4, 0x8005);

            //Offset Register (0x13)
            outw(0x03D4, 0x2813);

            //Vertical Total Register (index 0x06)
            outw(0x03D4, 0xBF06);

            //Overflow Register (0x07)
            outw(0x03D4, 0x1F07);

            //Maximum scan line regisyer (index 0x09)
            outw(0x03D4, 0x4109);

            //Vertical Retrace Start Register (index 0x10)
            outw(0x03D4, 0x9C10);

            //Vertical Retrace End Register (0x11) again
            outw(0x03D4, 0x8E11);

            //Vertical Display-Enable End Register (0x12)
            outw(0x03D4, 0x8F12);

            //Start Vertical Blanking Register (0x15)
            outw(0x03D4, 0x9615);

            //End Vertical Blanking Register (0x16)
            outw(0x03D4, 0xB916);

            //Preset row scan register (index 0x08)
            outw(0x03D4, 0x0008);

            //Underline Location Register (index 0x14)
            outw(0x03D4, 0x4014);

            //CRT Mode Control Register (0x17)
            outw(0x03D4, 0xA317);


            //Sequencer registers (0x03C4):
            //Memory mode register (index 0x04)
            outw(0x03C4, 0xE04);

            //Clocking mode register (index 0x01)
            outw(0x03C4, 0x0B01);

            //Map mask register (index 0x02)
            outw(0x03C4, 0x0F02);

            //Graphics controller registers (0xCE):
            outw(0x03CE, 0x4005);

            //Miscellaneous Register (index 0x06)
            outw(0x03CE, 0x0506);


            //Attribute controller registers:
            inb(0x03DA);
            outb(0x03C0, 0x30);
            outb(0x03C0, 0x41);
            outb(0x03C0, 0x33);
            outb(0x03C0, 0x00);


            //Setting the color pallete
            for i in 0..15 {
                let base = 16 * i;

                g_set_color(base + Color::Black as u8, 0, 0, 0);
                g_set_color(base + Color::Blue as u8, 0, 0, 168);
                g_set_color(base + Color::Green as u8, 0, 168, 0);
                g_set_color(base + Color::Cyan as u8, 0, 168, 168);
                g_set_color(base + Color::Red as u8, 168, 0, 0);
                g_set_color(base + Color::Magenta as u8, 168, 0, 168);
                g_set_color(base + Color::Brown as u8, 168, 84, 0);
                g_set_color(base + Color::LightGray as u8, 168, 168, 168);
                g_set_color(base + Color::DarkGray as u8, 84, 84, 84);
                g_set_color(base + Color::LightBlue as u8, 84, 84, 252);
                g_set_color(base + Color::LightGreen as u8, 84, 252, 84);
                g_set_color(base + Color::LightCyan as u8, 84, 252, 252);
                g_set_color(base + Color::LightRed as u8, 252, 84, 84);
                g_set_color(base + Color::Pink as u8, 252, 84, 252);
                g_set_color(base + Color::Yellow as u8, 252, 168, 84);
                g_set_color(base + Color::White as u8, 252, 252, 252);
            }
            asm!("sti");
        }
    }
}



#![allow(dead_code)]

use core::arch::asm;
use core::fmt::Arguments;
use core::ops::Add;
use core::ptr;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::drivers::vga::registers::vga_io::{load_4bit_color_palette_into_dac, set_03h_mode_regs, set_12h_mode_regs};
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 *
 * Vga buffer has typically two dimensional array
 * with size of 25 rows and 80 columns which is directly
 * rendered to the screen.
 * Each array entry discribes a single screen character with 
 * following format:
 *
 * Bit(s)   Value
 * 0-7      ASCII (code page 473 to be specific) code point
 * 8-11     Foreground color
 * 12-14    Background color
 * 15       Blink
 *
 * Colors that are available where Bit 4 is the bright bit:
 * (Note: For background color this bit is repurposed as the blink bit)
 *
 * Number   Color       Number          BrightColor
 *                      + Bright Bit
 *
 * 0x0      Black       0x8             Dark Gray
 * 0x1      Blue        0x9             Light Blue
 * 0x2      Green       0xa             Light Green
 * 0x3      Cyan        0xb             Light Cyan
 * 0x4      Red         0xc             Light Red
 * 0x5      Magenta     0xd             Pink
 * 0x6      Brown       0xe             Yellow
 * 0x7      LightGray   0xf             White
 */

//  **VGA REGISTER VALUES**
//  *MODE 0x03 TEXT MODE 80x25chars 16 colors*


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)] // u4 would be sufficient but rust does not have such type
pub enum Color{
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] // to make sure ColorCode is size of u8 
                     // and nothing more
struct ColorCode(u8);
impl ColorCode{
    // handy abstraction for making colors without
    // dealing with manual bytes making
    fn new(foreground: Color, background: Color) -> Self{
        Self((background as u8) << 4 | (foreground as u8) )
    }

    // returns background color of ColorCode
    fn foreground(&self) -> Color {
        let fg = self.0 & 0x0f; // lower 4 bits
        self.try_from_u8(fg)
    }

    // returns background color of ColorCode
    fn background(&self) -> Color {
        let bg = (self.0 >> 4) & 0x0f; // upper 4 bits
        self.try_from_u8(bg)
    }

    // matches u8 to Color
    // if v does not match any Color
    // returns white
    fn try_from_u8(&self, v : u8) -> Color {
        match v {
            0  => Color::Black,
            1  => Color::Blue,
            2  => Color::Green,
            3  => Color::Cyan,
            4  => Color::Red,
            5  => Color::Magenta,
            6  => Color::Brown,
            7  => Color::LightGray,
            8  => Color::DarkGray,
            9  => Color::LightBlue,
            10 => Color::LightGreen,
            11 => Color::LightCyan,
            12 => Color::LightRed,
            13 => Color::Pink,
            14 => Color::Yellow,
            _  => Color::White,
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] // to make sure field orderig does not change
struct ScreenChar {
    ascii_char_code: u8,
    color_code:  ColorCode,
}

const VGA_BUFFER_HEIGHT : usize = 25;
const VGA_BUFFER_WIDTH  : usize = 80;

#[repr(transparent)] // -//-
struct VgaBuffer{
    //TODO make volatile guard
    chars:  [[ScreenChar; VGA_BUFFER_WIDTH] ; VGA_BUFFER_HEIGHT],
}

pub struct VgaTextMode {
    column_position: usize,         // keeps track of current position in the row
    row_position: usize,            // keeps track of current row
    color_code: ColorCode,          // specifies currently used colors
    buffer: &'static mut VgaBuffer, // 'static is valid for VGA text buffer
    buf_start_p: usize, //buffer start address - used for clear_buf
    buf_end_p: usize    //buffer end address
}

impl VgaTextMode {

    fn new() -> Self{
        Self {
            column_position: 0,
            row_position: 0,
            color_code: ColorCode::new(Color::White,Color::Black),
            buffer: unsafe { &mut *(0xb8000 as *mut VgaBuffer) },
            buf_start_p: 0xB8000,
            buf_end_p: 0xBBFFF
        }
    }

    // writes string to vga buffer
    pub fn write(&mut self, s: &str){
        for byte in s.bytes() {
            match byte {
                // ASCII values
                0x20..=0x7E => self.write_byte(byte),

                // newline
                b'\n' => self.new_line(),

                // carriage return
                b'\r' => self.column_position = 0,

                // not in ASCII range
                _ => self.write_byte('?' as u8),
            }
        }
    }

    // changes foreground color 
    pub fn change_foreground_color(&mut self, fc: Color){
        self.color_code = ColorCode::new(
            fc,
            self.color_code.background()
        );
    }

    // changes background color 
    pub fn change_background_color(&mut self, bc: Color){
        self.color_code = ColorCode::new(
            self.color_code.foreground(),
            bc
        );
    }

    // changes color 
    pub fn change_color(&mut self, fc: Color, bc: Color){
        self.color_code = ColorCode::new(
            fc,
            bc
        );
    }

   // writes single ascii byte to the buffer
   fn write_byte(&mut self, byte: u8) {
        match byte {

            byte if self.column_position == VGA_BUFFER_WIDTH-1 => {
                self.new_line();
                self.write_byte(byte);
            },

            _ => {
                let row = self.row_position;
                let col = self.column_position;
                let color_code = self.color_code;

                // make ScreenChar with selected values
                let char = ScreenChar{ ascii_char_code: byte, color_code };

                // write it to buffer
                self.buffer.chars[row][col] = char;

                // update self state
                self.column_position += 1;
            }
        }
   }

   fn new_line(&mut self){
       if self.row_position + 1>= VGA_BUFFER_HEIGHT {
           self.shift_up();
       }else{
           self.row_position += 1;
       }
       self.column_position = 0;
   }

   pub fn shift_up(&mut self){
       for row in 1..VGA_BUFFER_HEIGHT{
           self.buffer.chars[row-1] = self.buffer.chars[row];
       }

       self.buffer.chars[VGA_BUFFER_HEIGHT-1]
           .fill(ScreenChar {
               ascii_char_code: b' ',
               color_code: self.color_code
           });
   }

    fn _vga_clear_mode_03h_buffer(&mut self) {
        unsafe {
            let pixel_p: *mut u8 = self.buf_start_p as *mut u8;
            let buf_size = self.buf_end_p - self.buf_start_p;
            for i in 0..buf_size {
                ptr::write_volatile(pixel_p.add(i), 0x00);
            }
        }
    }

    pub fn init_vga_text_mode_03h(&mut self) {
        unsafe {
            asm!("cli");
            set_03h_mode_regs();
            asm!("sti");
        }
        self._vga_clear_mode_03h_buffer();
    }
}


impl core::fmt::Write for VgaTextMode {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s);
        Ok(())
    }
}

// static instance of VgaWriter
// to access use vga::VGAWRITER.lock()
lazy_static! {
    pub static ref VGAWRITER: Mutex<VgaTextMode> = Mutex::new(VgaTextMode::new());
}

#[macro_export]
macro_rules! vgaprint {
    ($($arg:tt)*) => ($crate::drivers::vga::vga_text::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! vgaprintln {
    () => ($crate::vgaprint!("\n"));
    ($($arg:tt)*) => ($crate::vgaprint!("{}\n", format_args!($($arg)*)));
}


#[doc(hidden)]
pub fn _print(args: Arguments) {
    use core::fmt::Write;
    VGAWRITER.lock().write_fmt(args).unwrap();
}

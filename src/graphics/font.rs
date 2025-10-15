use crate::drivers::vga_graphics::{Font, Fonts};
use lazy_static::lazy_static;
/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

struct BitmapFont<const MEM_LENGTH: usize> {
    mem: &'static [u8; MEM_LENGTH],
    lochar: usize,
    hichar: usize,
    bytes_per_char: usize,
    height: usize,
    width: usize
}

impl<const MEM_LENGTH: usize> BitmapFont<MEM_LENGTH> {
    pub const fn new(mem: &'static [u8; MEM_LENGTH], lochar: usize, hichar: usize, width_bytes: usize, height: usize, width: usize) -> Self {
        Self {
            mem,
            lochar,
            hichar,
            bytes_per_char: width_bytes,
            height,
            width
        }
    }
}

//static font instances
lazy_static! {
    pub static ref  VGA_FONT_8PX: Font<768> = Fonts::font_8x8_px();
    pub static ref  VGA_FONT_16PX: Font<1536> = Fonts::font_8x16_px();
}
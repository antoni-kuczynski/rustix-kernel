use lazy_static::lazy_static;
use crate::drivers::vga_graphics::VgaFont;
/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

pub struct BitmapFont {
    mem: &'static [u8],
    lochar: usize,
    hichar: usize,
    bytes_per_char: usize,
    height: usize,
    width: usize
}

impl BitmapFont {
    pub const fn new(mem: &'static [u8], lochar: usize, hichar: usize, width_bytes: usize, height: usize, width: usize) -> Self {
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
    pub static ref  VGA_FONT_8PX: VgaFont = VgaFont::FONT_8PX;
    pub static ref  VGA_FONT_16PX: VgaFont = VgaFont::FONT_16PX;
}
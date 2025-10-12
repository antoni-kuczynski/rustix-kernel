#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct U8Color(pub u8);

impl U8Color {
    pub const WHITE: Self = Self(0xFF);
    pub const BLACK: Self = Self(0x00);
    pub const RED: Self = Self(0b11100000);
    pub const GREEN: Self = Self(0b00011100);
    pub const BLUE: Self = Self(0b00000011);
    pub const YELLOW: Self = Self(0b11111100); //mix of green and red
    pub const CYAN: Self = Self(0b00011111); //mix of green and blue
    pub const MAGENTA: Self = Self(0b11100011); //mix of red and blue


    pub fn from_u24_rgb_to_u8(r: u8, g: u8, b: u8) -> Self {
        //Returns "compressed" color from 24bit to 8bit
        /*
        7   6   5   4   3   2   1   0
        R   R   R   G   G   G   B   B
         */
        let r_dac =  r >> 5 << 5; //3 bytes
        let g_dac =  g >> 5 << 2; //3 bytes
        let b_dac = b >> 6; //2 bytes
        U8Color(r_dac | g_dac | b_dac)
    }

    pub fn from_u8(value: u8) -> Self {
        Self(value)
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }
}
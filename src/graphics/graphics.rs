use crate::drivers::vga::vga_graphics::{VgaVideoMode};
#[allow(dead_code)]
use crate::graphics::bitmap::Bitmap;
use crate::graphics::color::ColorU8;
use crate::drivers::vga::vga_fonts::*;
/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

pub struct PointUnsigned {
    pub(crate) x: usize,
    pub(crate) y: usize
}

impl PointUnsigned {

}

pub struct Rectangle {
    pub p0: PointUnsigned,
    pub width: usize,
    pub height: usize
}


pub struct Graphics {
    color: ColorU8,
    font: VgaFont,
    device: VgaVideoMode<64000>
}

impl Graphics {
    pub fn new() -> Self {
        let mut temp = Graphics {
            color: ColorU8::BLACK,
            font: VgaFont::FONT_8PX,
            device: VgaVideoMode::<64000>::new_vga_0x13_320x200_256color_mode()
        };
        temp.device.vga13h_init();
        temp
    }

    pub fn get_video_width(&self) -> usize {
        self.device.video_width_px
    }

    pub fn get_video_height(&self) -> usize {
        self.device.video_height_px
    }

    pub fn clear(&mut self) {
        self.device._vga13h_clear_back_buffer(self.color.as_u8());
    }

    pub fn set_color(&mut self, color: ColorU8) {
        self.color = color;
    }

    pub fn set_font(&mut self, font: VgaFont) {
        self.font = font;
    }

    pub fn fill_rect(&mut self, rect: &Rectangle) {
        let p0 = &rect.p0;
        self.device._vga13h_fill_rect(
            p0.x, p0.y,
            rect.width, rect.height,
            self.color.as_u8()
        );
    }

    pub fn draw_rect(&mut self, rect: &Rectangle) {
        let p0 = &rect.p0;
        self.device._vga13h_draw_rect(
            p0.x, p0.y,
            rect.width, rect.height,
            self.color.as_u8()
        );
    }

    pub fn fill_triangle(&mut self, p0: &PointUnsigned, p1: &PointUnsigned, p2: &PointUnsigned) {
        self.device._vga13h_fill_triangle(
            p0.x, p0.y,
            p1.x, p1.y,
            p2.x, p2.y,
            self.color.as_u8()
        );
    }

    pub fn draw_triangle(&mut self, p0: &PointUnsigned, p1: &PointUnsigned, p2: &PointUnsigned) {
        self.device._vga13h_draw_triangle(
            p0.x, p0.y,
            p1.x, p1.y,
            p2.x, p2.y,
            self.color.as_u8()
        );
    }

    pub fn draw_line(&mut self, p0: &PointUnsigned, p1: &PointUnsigned) {
        self.device._vga13h_draw_line(
            p0.x, p0.y,
            p1.x, p1.y,
            self.color.as_u8()
        );
    }

    pub fn draw_bitmap(&mut self, p: &PointUnsigned, bitmap: &Bitmap) {
        self.device._vga13h_draw_bitmap(
            p.x, p.y,
            bitmap.width, bitmap.height,
            &bitmap.mem
        );
    }

    pub fn draw_char(&mut self, p: &PointUnsigned, char: char) {
        self.device._vga13h_draw_char_transparent(
            p.x,p.y,
            char,
            &self.font,
            self.color.as_u8()
        )
    }

    pub fn draw_str(&mut self, p: &PointUnsigned, str: &str) {
        self.device._vga13h_draw_string(
            p.x,p.y,
            str,
            &self.font,
            self.color.as_u8()
        )
    }

    pub fn fill_elipse(&mut self, p: &PointUnsigned, width: usize, height: usize) {
        self.device._vga13h_fill_ellipse(
            p.x, p.y,
            width, height,
            self.color.as_u8()
        );
    }

    pub fn draw_elipse(&mut self, p: &PointUnsigned, width: usize, height: usize) {
        self.device._vga13h_draw_ellipse(
            p.x, p.y,
            width, height,
            self.color.as_u8()
        );
    }

    pub fn update(&mut self) {
        self.device.vga13h_update();
    }
}

#[macro_export]
macro_rules! point {
    ($x:expr , $y:expr) => {
        PointUnsigned {
            x: $x,
            y: $y
        }
    };
}

#[macro_export]
macro_rules! rect {
    ($x:expr , $y:expr, $w:expr, $h:expr) => {
        Rectangle {
            p0: point!($x, $y),
            width: $w,
            height: $h
        }
    };
}
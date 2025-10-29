use crate::drivers::vga::vga_graphics::{VgaVideoMode};
#[allow(dead_code)]
use crate::graphics::bitmap::Bitmap;
use crate::graphics::color::U8Color;
use crate::drivers::vga::vga_fonts::*;
/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

pub struct UPoint {
    pub(crate) x: usize,
    pub(crate) y: usize
}

impl UPoint {

}

pub struct Rectangle {
    pub p0: UPoint,
    pub width: usize,
    pub height: usize
}

pub struct Triangle {
    pub p0: UPoint,
    pub p1: UPoint,
    pub p2: UPoint
}


pub struct Graphics {
    color: U8Color,
    font: VgaFont,
    device: VgaVideoMode<64000>
}

impl Graphics {
    pub fn new() -> Self {
        let mut temp = Graphics {
            color: U8Color::BLACK,
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
        self.device._vga13h_clear_back_buffer();
    }

    pub fn set_color(&mut self, color: U8Color) {
        self.color = color;
    }

    pub fn set_font(&mut self, font: VgaFont) {
        self.font = font;
    }

    pub fn fill_rect(&mut self, rect: Rectangle) {
        let p0 = rect.p0;
        self.device._vga13h_fill_rect(
            p0.x, p0.y,
            rect.width, rect.height,
            self.color.as_u8()
        );
    }

    pub fn draw_rect(&mut self, rect: Rectangle) {
        let p0 = rect.p0;
        self.device._vga13h_draw_rect(
            p0.x, p0.y,
            rect.width, rect.height,
            self.color.as_u8()
        );
    }

    pub fn fill_triangle(&mut self, tr: Triangle) {
        let p0 = tr.p0;
        let p1 = tr.p1;
        let p2 = tr.p2;
        self.device._vga13h_fill_triangle(
            p0.x, p0.y,
            p1.x, p1.y,
            p2.x, p2.y,
            self.color.as_u8()
        );
    }

    pub fn draw_triangle(&mut self, tr: Triangle) {
        let p0 = tr.p0;
        let p1 = tr.p1;
        let p2 = tr.p2;
        self.device._vga13h_draw_triangle(
            p0.x, p0.y,
            p1.x, p1.y,
            p2.x, p2.y,
            self.color.as_u8()
        );
    }

    pub fn draw_line(&mut self, p0: &UPoint, p1: &UPoint) {
        self.device._vga13h_draw_line(
            p0.x, p0.y,
            p1.x, p1.y,
            self.color.as_u8()
        );
    }

    pub fn draw_bitmap(&mut self, p: UPoint, bitmap: &Bitmap) {
        self.device._vga13h_draw_bitmap(
            p.x, p.y,
            bitmap.width, bitmap.height,
            &bitmap.mem
        );
    }

    pub fn draw_char(&mut self, p: UPoint, char: char) {
        self.device._vga13h_draw_char_transparent(
            p.x,p.y,
            char,
            &self.font,
            self.color.as_u8()
        )
    }

    pub fn draw_str(&mut self, p: UPoint, str: &str) {
        self.device._vga13h_draw_string(
            p.x,p.y,
            str,
            &self.font,
            self.color.as_u8()
        )
    }

    pub fn fill_elipse(&mut self, p: &UPoint, width: usize, height: usize) {
        self.device._vga13h_fill_elipse(
            p.x, p.y,
            width, height,
            self.color.as_u8()
        );
    }

    pub fn draw_elipse(&mut self, p: UPoint, width: usize, height: usize) {
        self.device._vga13h_draw_elipse(
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
        UPoint {
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

#[macro_export]
macro_rules! triangle {
    ($x0:expr , $y0:expr, $x1:expr, $y1:expr, $x2:expr, $y2:expr) => {
        Triangle {
            p0: point!($x0, $y0),
            p1: point!($x1, $y1),
            p2: point!($x2, $y2),
        }
    };
}
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

use crate::graphics::graphics::Triangle;
use crate::graphics::graphics::Rectangle;
use core::panic::PanicInfo;

use crate::{drivers::vga_text::{Color, VGAWRITER}};
use crate::drivers::vga_graphics::Fonts;
use crate::graphics::bitmap::Bitmap;
use crate::graphics::color::U8Color;
use crate::graphics::font::VGA_FONT_16PX;
use crate::graphics::graphics::{Graphics, UPoint};
use crate::test_bitmap::{DRAWN_HOUSE_BITMAP_DATA, MY_CAT_BITMAP_DATA};

mod drivers;
mod interrupts;
mod test_bitmap;
mod graphics;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let mut graphics = Graphics::new();

    let house: Bitmap<64000> = Bitmap::new(320, 200, DRAWN_HOUSE_BITMAP_DATA);
    let my_cat: Bitmap<16000> = Bitmap::new(160, 100, MY_CAT_BITMAP_DATA);
    graphics.draw_bitmap(point!(0,0), house);
    graphics.draw_bitmap(point!(0,0), my_cat);

    graphics.set_color(U8Color::MAGENTA);
    graphics.draw_line(point!(1,1), point!(50,50));

    graphics.set_color(U8Color::GREEN);
    graphics.fill_rect(rect!(60,60,40,20));
    graphics.draw_rect(rect!(60,90,40,20));

    graphics.set_color(U8Color::CYAN);
    graphics.fill_triangle(triangle!(200,50,160,120,230,100));
    graphics.draw_triangle(triangle!(200,120,160,190,230,170));

    graphics.set_color(U8Color::YELLOW);
    graphics.draw_str(point!(10,170), "hello world", Fonts::font_8x8_px());
    graphics.set_color(U8Color::BLUE);
    graphics.draw_str(point!(10,180), "hello world x2", Fonts::font_8x16_px());
    graphics.set_color(U8Color::YELLOW);
    graphics.draw_char(point!(10,150), 'a', Fonts::font_8x16_px());

    loop{
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().change_foreground_color(Color::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop {}
}

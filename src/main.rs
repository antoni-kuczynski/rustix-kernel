#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

use core::panic::PanicInfo;

use crate::{drivers::vga_text::{Color, VGAWRITER}};
use crate::drivers::vga_graphics::{Bitmap, Fonts, VgaVideoColor, VgaVideoMode};
use crate::test_bitmap::{DATA_TRIMMED};

mod drivers;
mod interrupts;
mod test_bitmap;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let mut video: VgaVideoMode<64000> = VgaVideoMode::<64000>::new_vga_0x13_320x200_256color_mode();
    video.init_mode();
    let bitmap: Bitmap<16000> = Bitmap::new(160, 100, DATA_TRIMMED);
    video.draw_bitmap(159,99, bitmap);

    // video.fill_rect(100,40,20,20, VgaVideoColor::from_u24_rgb(40, 117, 223));
    video.fill_rect(0, 179, 20, 20, VgaVideoColor::RED);
    video.fill_rect(20, 179, 20, 20, VgaVideoColor::GREEN);
    video.fill_rect(40, 179, 20, 20, VgaVideoColor::BLUE);
    video.fill_rect(60, 179, 20, 20, VgaVideoColor::YELLOW);
    video.fill_rect(80, 179, 20, 20, VgaVideoColor::CYAN);
    video.fill_rect(100, 179, 20, 20, VgaVideoColor::MAGENTA);
    video.fill_rect(120, 179, 20, 20, VgaVideoColor::WHITE);
    video.fill_rect(140, 179, 20, 20, VgaVideoColor::BLACK);
    video.draw_line(200,10,240,100, VgaVideoColor::from_u24_rgb_to_u8(166, 184, 102));
    video.draw_rect(220, 20, 50, 60, VgaVideoColor::from_u24_rgb_to_u8(255, 171, 0));

    // video.draw_char_transparent(200, 100, 'f', &Font::<768>::font_8px(), VgaVideoColor::WHITE);
    video.draw_string(0, 10, "8 pixel height:", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 20, "abcdefghijklmnoprstuwxyz", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 30, "ABCDEFGHIJKLMNOPRSTUWXYZ", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 40, "1234567890", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);

    video.draw_string(0, 60, "16 pixel height:", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 80, "abcdefghijklmnoprstuwxyz", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 100, "ABCDEFGHIJKLMNOPRSTUWXYZ", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 120, "1234567890!@#$%^&*()+-=[]{}<>?,/;':\"", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    video.draw_string(0, 190, "ąę©ąąśðæðśæ„ćź", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);

    // let mut video_12h = VgaVideoMode::<38400>::new_vga_0x12_640x480_16color_mode();
    // video_12h.init_mode_0x12();
    // // for x in 10..30 {
    // //     video_12h.
    // // }
    // video_12h.put_pixel(6,0,VgaVideoColor(0xD));

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

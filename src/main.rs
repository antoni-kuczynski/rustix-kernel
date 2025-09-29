#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

use core::panic::PanicInfo;

use crate::{drivers::vga_text::{Color, VGAWRITER}};
use crate::drivers::vga_graphics::{Bitmap, Font, VgaVideoColor, VgaVideoMode};
// use crate::test_bitmap::{DATA, DATA_TRIMMED};
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

    let mut video = VgaVideoMode::new_vga_320x200_256_mode();
    video.init_mode();
    let bitmap: Bitmap<16000> = Bitmap::new(160, 100, DATA_TRIMMED);
    video.draw_bitmap(50,80, bitmap);

    // video.fill_rect(100,40,20,20, VgaVideoColor::from_u24_rgb(40, 117, 223));
    video.fill_rect(0,40,20,20, VgaVideoColor::RED);
    video.fill_rect(20,40,20,20, VgaVideoColor::GREEN);
    video.fill_rect(40,40,20,20, VgaVideoColor::BLUE);
    video.fill_rect(60,40,20,20, VgaVideoColor::YELLOW);
    video.fill_rect(80,40,20,20, VgaVideoColor::CYAN);
    video.fill_rect(100,40,20,20, VgaVideoColor::MAGENTA);
    video.fill_rect(120,40,20,20, VgaVideoColor::WHITE);
    video.fill_rect(140,40,20,20, VgaVideoColor::BLACK);
    video.draw_line(20,10,60,100, VgaVideoColor::from_u24_rgb(166, 184, 102));
    video.draw_rect(150, 20, 50, 60, VgaVideoColor::from_u24_rgb(255, 171, 0));

    // video.draw_char_transparent(200, 100, 'f', &Font::<768>::font_8px(), VgaVideoColor::WHITE);
    video.draw_string(200,100, "abcdef", &Font::<768>::font_8px(), VgaVideoColor::WHITE);

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

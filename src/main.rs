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
use crate::test_bitmap::{DRAWN_HOUSE_BITMAP_DATA, MY_CAT_BITMAP_DATA};

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
    video.init_mode_0x13();

    let bitmap1: Bitmap<64000> = Bitmap::new(320, 200, DRAWN_HOUSE_BITMAP_DATA);
    video.draw_bitmap(0,0,bitmap1);
    // video.draw_string(5,5,"Domek", &Fonts::font_8x16_px(), VgaVideoColor::MAGENTA);
    // video.draw_line(50,50,200,130, VgaVideoColor::MAGENTA);

    // video.fill_triangle(
    //     80,20,
    //     10,80,
    //     120,80,
    //     VgaVideoColor::BLUE
    // );
    // video.draw_line(0,20,320,20,VgaVideoColor::RED);
    // video.draw_line(0,80,320,80,VgaVideoColor::RED);
    // video.draw_line(10,20,10,80,VgaVideoColor::RED);
    // video.draw_line(120,20,120,80,VgaVideoColor::RED);
    //
    //
    // // Smaller green triangle (flat-bottom)
    // video.fill_triangle(
    //     100,160,  // top
    //     70,180,   // bottom-left
    //     130,180,  // bottom-right
    //     VgaVideoColor::GREEN
    // );
    //
    // // Bounding box lines (red)
    // video.draw_line(0,160,320,160,VgaVideoColor::RED);  // top
    // video.draw_line(0,180,320,180,VgaVideoColor::RED);  // bottom
    // video.draw_line(70,160,70,180,VgaVideoColor::RED);  // left
    // video.draw_line(130,160,130,180,VgaVideoColor::RED); // right

    for i in 0..32 {
        let ch = ('0' as u8 + i as u8) as char;
        video.draw_char_transparent(i * 10, 20, ch, &Fonts::font_8x8_px(), VgaVideoColor::BLUE);
        video.draw_line(i * 10, 0,i * 10,199, VgaVideoColor::BLACK);
    }

    for i in 0..20 {
        let ch = ('0' as u8 + i as u8) as char;
        video.draw_char_transparent(20, i * 10, ch, &Fonts::font_8x8_px(), VgaVideoColor::GREEN);
        video.draw_line(0, i * 10,319,i * 10, VgaVideoColor::BLACK);
    }
    video.draw_string(30,0,"Line every 10px grid", &Fonts::font_8x8_px(), VgaVideoColor::MAGENTA);

    // 1. Flat-bottom triangle (normal) PASSED ✅
    // video.fill_triangle(160, 40, 100, 120, 220, 120, VgaVideoColor::YELLOW);

    // 2. Flat-top triangle PASSED ✅
    // video.fill_triangle(100, 120, 220, 120, 160, 180, VgaVideoColor::GREEN);

    // 3. Tall narrow triangle (steep sides)    PASSED ✅
    // video.fill_triangle(160, 30, 150, 180, 170, 180, VgaVideoColor::CYAN);
    //
    // // 4. Wide shallow triangle (flat-ish)   PASSED ✅ (sort of)
    // video.fill_triangle(50, 150, 270, 170, 160, 160, VgaVideoColor::RED); //impossible to draw properly on such low resolution
    // video.fill_triangle(1, 3, 180, 115, 318, 198, VgaVideoColor::RED);
    //
    // // 5. Left-leaning triangle  PASSED ✅
    // video.fill_triangle(100, 50, 60, 180, 140, 160, VgaVideoColor::YELLOW);
    //
    // // 6. Right-leaning triangle PASSED ✅
    // video.fill_triangle(220, 60, 260, 180, 180, 160, VgaVideoColor::BLUE);
    //
    // // 7. Small triangle (fine detail check) PASSED ✅
    // video.fill_triangle(50, 30, 60, 40, 40, 45, VgaVideoColor::WHITE);
    // video.draw_line(50,30,60,40, VgaVideoColor::RED);
    // video.draw_line(50,30,40,45, VgaVideoColor::RED);
    // video.draw_line(60,40,40,45, VgaVideoColor::RED);

    //
    // // 8. test if triangles are symetrical PASSED ✅
    // video.fill_triangle(160, 180, 100, 100, 220, 98, VgaVideoColor::RED);
    // video.fill_triangle(160, 20, 100, 99, 220, 97, VgaVideoColor::GREEN);
    //
    // // 9. Nearly vertical triangle (tests slope rounding)    PASSED ✅
    // video.fill_triangle(300, 50, 295, 180, 305, 185, VgaVideoColor::GREEN);

    // video.fill_triangle(
    //     160, 40,
    //     100, 120,
    //     220, 120,
    //     VgaVideoColor::YELLOW
    // );
    //
    video.fill_triangle(
        270,160,  // top
        310,100,   // bottom-left
        310,170,  // bottom-right
        VgaVideoColor::YELLOW
    );
    video.draw_triangle(
        100,160,  // top
        70,100,   // bottom-left
        50,170,  // bottom-right
        VgaVideoColor::CYAN);
    // video.draw_line(200,160,170,100, VgaVideoColor::RED);
    // video.draw_line(200,160,250,170, VgaVideoColor::RED);
    // video.draw_line(170,100,250,170, VgaVideoColor::RED);

    //
    // video.fill_triangle(
    //     70,100,
    //     100,160,  // top
    //     150,170,  // bottom-right
    //     VgaVideoColor::YELLOW
    // );
    //
    // video.fill_triangle(
    //     150,70,  // bottom-right
    //     100,60,  // top
    //     70,0,   // bottom-left
    //     VgaVideoColor::YELLOW
    // );
    //
    // video.fill_triangle(
    //     100,160,  // top
    //     109,132,   // bottom-left
    //     111,29,  // bottom-right
    //     VgaVideoColor::YELLOW
    // );
    //
    // video.draw_line(100,40,200, 80, VgaVideoColor::GREEN);
    // video.fill_triangle(
    //     10,10,  // top
    //     40,10,   // bottom-left
    //     250,170,  // bottom-right
    //     VgaVideoColor::GREEN
    // );


    // let bitmap: Bitmap<16000> = Bitmap::new(160, 100, DATA_TRIMMED);
    // video.draw_bitmap(159,99, bitmap);
    //
    // video.fill_rect(100,40,20,20, VgaVideoColor::from_u24_rgb_to_u8(40, 117, 223));
    // video.fill_rect(0, 179, 20, 20, VgaVideoColor::RED);
    // video.fill_rect(20, 179, 20, 20, VgaVideoColor::GREEN);
    // video.fill_rect(40, 179, 20, 20, VgaVideoColor::BLUE);
    // video.fill_rect(60, 179, 20, 20, VgaVideoColor::YELLOW);
    // video.fill_rect(80, 179, 20, 20, VgaVideoColor::CYAN);
    // video.fill_rect(100, 179, 20, 20, VgaVideoColor::MAGENTA);
    // video.fill_rect(120, 179, 20, 20, VgaVideoColor::WHITE);
    // video.fill_rect(140, 179, 20, 20, VgaVideoColor::BLACK);
    // video.draw_line(200,10,240,100, VgaVideoColor::from_u24_rgb_to_u8(166, 184, 102));
    // video.draw_rect(220, 20, 50, 60, VgaVideoColor::from_u24_rgb_to_u8(255, 171, 0));
    //
    // // video.draw_char_transparent(200, 100, 'f', &Font::<768>::font_8px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 10, "8 pixel height:", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 20, "abcdefghijklmnoprstuwxyz", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 30, "ABCDEFGHIJKLMNOPRSTUWXYZ", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 40, "1234567890", &Fonts::font_8x8_px(), VgaVideoColor::WHITE);
    //
    // video.draw_string(0, 60, "16 pixel height:", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 80, "abcdefghijklmnoprstuwxyz", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 100, "ABCDEFGHIJKLMNOPRSTUWXYZ", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 120, "1234567890!@#$%^&*()+-=[]{}<>?,/;':\"", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);
    // video.draw_string(0, 190, "ąę©ąąśðæðśæ„ćź", &Fonts::font_8x16_px(), VgaVideoColor::WHITE);

    // let mut video_12h = VgaVideoMode::<64000>::new_vga_0x12_640x480_16color_mode();
    // video_12h.init_mode_0x12();

    // for i in 0..480 {
    //     video_12h.put_pixel_12h(10,i,VgaVideoColor::from_u8(0xD));
    // }
    // video_12h.put_pixel(6,0,VgaVideoColor(0xD));
    // video_12h.put_pixel(6,1,VgaVideoColor(0xD));

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

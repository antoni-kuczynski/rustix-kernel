#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use crate::graphics::graphics::UPoint;
use crate::graphics::graphics::Rectangle;
use crate::drivers::vga::vga_text::{Color, VgaTextMode, VGAWRITER};
use crate::graphics::graphics::Graphics;
use crate::memory::mapping::BootInfoFrameAllocator;
use crate::memory::pages;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use crate::drivers::vga::CURRENT_VGA_MODE;
use crate::graphics::color::U8Color;
use crate::interrupts::hardware::pic8259::{get_current_time_millis, get_ticks, sleep};
use crate::test_bitmap::{get_drawn_house_bitmap, get_my_cat_bitmap};

mod drivers;
mod interrupts;
mod memory;
mod bootinfo;
mod graphics;
mod test_bitmap;
mod asm;

entry_point!(_start);
fn _start(boot_info: &'static BootInfo) -> ! {
    bootinfo::show_vitals(&boot_info);

    interrupts::init_idt();
    interrupts::gdt::init_gdt();
    interrupts::hardware::pic8259::init_pics();
    interrupts::enable();

    let mut _offset_page_table = pages::init(&boot_info);

    let mut _fa = BootInfoFrameAllocator::init(&boot_info.memory_map);

    memory::gallocator::init(&mut _offset_page_table,&mut _fa)
        .expect("heap init failed");

    CURRENT_VGA_MODE.lock().switch_to(0x03);

    // let mut g: Graphics = Graphics::new();
    // let mut coords1: UPoint = point!(50,50);
    //
    // let mut dx: isize = 5;
    // let mut dy: isize = 5;
    // let radius: isize = 20;
    //
    // g.set_color(U8Color::MAGENTA);
    // loop {
    //     // draw the rectangle
    //     g.fill_rect(rect!(coords1.x, coords1.y, 20, 20));
    //     // g.fill_elipse(&point!(coords1.x, coords1.y), (radius * 2) as usize, (radius * 2) as usize);
    //
    //     // bounce horizontally
    //     if coords1.x as isize + dx + radius >= g.get_video_width() as isize || coords1.x as isize + dx < 0 {
    //         dx = -dx;
    //     }
    //
    //     // bounce vertically
    //     if coords1.y as isize + dy + radius >= g.get_video_height() as isize|| coords1.y as isize + dy < 0 {
    //         dy = -dy;
    //     }
    //
    //     // update position
    //     coords1.x = (coords1.x as isize + dx) as usize;
    //     coords1.y = (coords1.y as isize + dy) as usize;
    //
    //     g.update();
    //     g.clear();
    //     sleep(16);
    // }

    let mut g: Graphics = Graphics::new();
    let radius: isize = 20;

    // Initial positions
    let mut coords = [
        point!(50, 50),
        point!(100, 80),
        point!(150, 120),
        point!(200, 160),
        point!(250, 179),
    ];

    // Velocities for each square
    let mut velocities = [
        (1, 0),
        (-1, 0),
        (1, -0),
        (-1, -0),
        (1, -0),
    ];



    let colors = [U8Color::GREEN, U8Color::BLUE, U8Color::MAGENTA, U8Color::RED, U8Color::YELLOW];

    let mut previous_time = get_current_time_millis();
    loop {
        let current_time = get_current_time_millis();
        let delta_time = current_time - previous_time + 1;

        let fps = 1_000_000 / delta_time;
        let fps_str: String = format!("FPS: {}", fps);
        let d_time_str = format!("D_TIME: {}", delta_time);
        g.set_color(U8Color::MAGENTA);
        // g.fill_rect(rect!(0,0,319,199));
        // g.draw_bitmap(&point!(0,0), &get_my_cat_bitmap().unwrap());

        for i in 0..coords.len() {
            let (dx, dy) = velocities[i];
            let mut x = coords[i].x as isize;
            let mut y = coords[i].y as isize;

            g.set_color(colors[i]);
            // Draw square
            // g.draw_rect(rect!(x as usize, y as usize, 20, 20));
            let x_u = x as usize;
            let y_u = y as usize;
            // g.draw_triangle(point!(x_u, y_u), point!(x_u+20, y_u+10), point!(x_u+10, y_u+20));
            g.draw_elipse(&point!(x_u + 10, y_u), 10, 10);

            // Bounce horizontally
            if x + dx + radius >= g.get_video_width() as isize || x + dx < 0 {
                velocities[i].0 = -dx;
            }

            // Bounce vertically
            if y + dy + radius >= g.get_video_height() as isize || y + dy < 0 {
                velocities[i].1 = -dy;
            }

            // Update position
            coords[i].x = (x + velocities[i].0) as usize;
            coords[i].y = (y + velocities[i].1) as usize;
            // coords[i].y = (y + velocities[i].1) as usize;
        }

        g.draw_str(&point!(10,10), fps_str.as_str());
        g.draw_str(&point!(10,20), d_time_str.as_str());

        g.update();
        g.set_color(U8Color::CYAN);
        g.clear();
        previous_time = current_time;
        // sleep(16);
    }

//     let mut video = VgaVideoMode::<64000>::new_vga_mode_X_320x200_256color();
// video.vga_320_200_mode_X_init();
// video.vga_320_200_X_clear_front_buffer();
//     // video.vga_320_200_X_fill_rect(10,10,50,50,0xFF);
//     video.vga_320_200_X_put_pixel(100,100,U8Color::MAGENTA.0);

    loop{
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    VGAWRITER.lock().init_vga_text_mode_03h();  //on panic switch to text mode
    VGAWRITER.lock().change_foreground_color(Color::LightRed);
    vgaprintln!("=!==============================!=");
    vgaprintln!("Kernel panic! \n{}", _info);
    vgaprintln!("=!==============================!=");
    loop{
        x86_64::instructions::hlt();
    }
}

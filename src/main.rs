#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;
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
use crate::interrupts::hardware::pic8259::sleep;

mod drivers;
mod interrupts;
mod memory;
mod bootinfo;
mod graphics;
mod test_bitmap;

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
        (5, 5),
        (-4, 3),
        (3, -4),
        (-5, -3),
        (4, -5),
    ];



    let colors = [U8Color::GREEN, U8Color::BLUE, U8Color::MAGENTA, U8Color::RED, U8Color::YELLOW];

    loop {
        for i in 0..coords.len() {
            let (dx, dy) = velocities[i];
            let mut x = coords[i].x as isize;
            let mut y = coords[i].y as isize;

            g.set_color(colors[i]);
            // Draw square
            g.fill_rect(rect!(x as usize, y as usize, 20, 20));

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
        }

        g.update();
        g.clear();
        sleep(16);
    }



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

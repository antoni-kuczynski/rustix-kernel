#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
/*
 * Created by Oskar Przybylski
 * 22/09/2025
 */

extern crate alloc;

use crate::graphics::graphics::UPoint;
use core::{panic::PanicInfo};
use alloc::{boxed::Box, vec};
use bootloader::{entry_point, BootInfo};
use crate::{drivers::vga_text::{Color, VGAWRITER}, memory::{mapping::BootInfoFrameAllocator, pages} };
use crate::drivers::vga_graphics::VgaFont;
use crate::graphics::bitmap::Bitmap;
use crate::graphics::color::U8Color;
use crate::graphics::graphics::Graphics;
use crate::test_bitmap::{get_my_cat_bitmap};

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

    // let x = Box::new(5);
    // let v = vec![1,2,3];
    //
    // vgaprintln!("{}, {:#?}",x,v);
    //
    // vgaprintln!("nie wyjebalo sie jupi");

    let mut graphics = Graphics::new();

    // graphics.set_color(U8Color::MAGENTA);
    // graphics.fill_elipse(point!(100,100),90,50);

    graphics.set_color(U8Color::WHITE);
    graphics.draw_str(point!(10,10), "abcdefghijklmnoprstuwxyz");
    graphics.draw_str(point!(10,25), "ABCDEFGHIJKLMNOPRSTUWXYZ");
    graphics.draw_str(point!(10,40), "1234567890!@#$%^&*()-=_+");
    let bmp = Bitmap::new_u8_bitmap(4, 1, vec![0xFF, 0xFF, 0xFF, 0xFF]);
    match bmp {
        None => {}
        Some(_) => {
            graphics.draw_bitmap(point!(0,0), &bmp.unwrap());
        }
    }


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
    loop{
        x86_64::instructions::hlt();
    }
}

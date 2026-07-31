use core::sync::atomic::Ordering;
use spin::MutexGuard;
use crate::drivers::input::mouse::mouse::GLOBAL_MOUSE;
use crate::video::framebuffer::{fb_bpp, fb_height, fb_pitch, fb_width, Framebuffer};

pub mod mouse;

const T: u8 = 0; //transparent
const B: u8 = 1; //black
const W: u8 = 2; //white

pub const CURSOR_DIMENSIONS: isize = 16;
pub const CURSOR_BYTE_SIZE: usize = (CURSOR_DIMENSIONS * CURSOR_DIMENSIONS) as usize;

#[rustfmt::skip]
pub const MOUSE_POINTER: [u8; (CURSOR_DIMENSIONS * CURSOR_DIMENSIONS) as usize] = [
    W, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T,
    W, W, T, T, T, T, T, T, T, T, T, T, T, T, T, T,
    W, B, W, T, T, T, T, T, T, T, T, T, T, T, T, T,
    W, B, B, W, T, T, T, T, T, T, T, T, T, T, T, T,
    W, B, B, B, W, T, T, T, T, T, T, T, T, T, T, T,
    W, B, B, B, B, W, T, T, T, T, T, T, T, T, T, T,
    W, B, B, B, B, B, W, T, T, T, T, T, T, T, T, T,
    W, B, B, B, B, B, B, W, T, T, T, T, T, T, T, T,
    W, B, B, B, B, B, B, B, W, T, T, T, T, T, T, T,
    W, B, B, B, B, B, B, B, B, W, T, T, T, T, T, T,
    W, B, B, B, B, B, W, W, W, W, W, T, T, T, T, T,
    W, B, B, W, B, B, W, T, T, T, T, T, T, T, T, T,
    W, B, W, T, W, B, B, W, T, T, T, T, T, T, T, T,
    W, W, T, T, W, B, B, W, T, T, T, T, T, T, T, T,
    W, T, T, T, T, W, B, B, W, T, T, T, T, T, T, T,
    T, T, T, T, T, T, W, W, W, T, T, T, T, T, T, T,
];

pub fn draw_cursor(mut fb: MutexGuard<Option<Framebuffer>>,) {
    let fb_width = fb_width();
    let fb_height = fb_height();

    let mouse = GLOBAL_MOUSE.get().unwrap();
    let mouse_x = mouse.x_pos.load(Ordering::SeqCst);
    let mouse_y = mouse.y_pos.load(Ordering::SeqCst);


    let start_x = mouse_x.max(0);
    let start_y = mouse_y.max(0);

    let end_x = (mouse_x + CURSOR_DIMENSIONS).min(fb_width as isize);
    let end_y = (mouse_y + CURSOR_DIMENSIONS).min(fb_height as isize);

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let bmp_offset_x = start_x - mouse_x;
    let bmp_offset_y = start_y - mouse_y;

    let draw_width = end_x - start_x;
    let draw_height = end_y - start_y;

    fb.as_mut().unwrap().swap_buffers();
    
    let bpp_bytes = fb_bpp() as usize >> 3;
    for cy in 0..draw_height {
        let screen_y = start_y + cy;
        let bmp_y = bmp_offset_y + cy;

        let fb_row_start = (screen_y * fb_pitch() as isize) as usize;
        let bmp_row_start = (bmp_y * CURSOR_DIMENSIONS) as usize;

        for cx in 0..draw_width {
            let bmp_x = bmp_offset_x + cx;
            let pixel = MOUSE_POINTER[bmp_row_start + bmp_x as usize];

            if pixel != T {
                let screen_x = start_x + cx;
                let color: u32 = if pixel == B { 0x000000 } else { 0xFFFFFF };

                unsafe {
                    fb.as_mut().unwrap().fb_primary_write_raw_pixel_24(
                        fb_row_start + (screen_x as usize * bpp_bytes),
                        color,
                        bpp_bytes
                    );
                }
            }
        }
    }

}
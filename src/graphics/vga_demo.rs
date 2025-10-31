use crate::Rectangle;
use crate::PointUnsigned;
use alloc::format;
use alloc::string::String;
use crate::graphics::color::ColorU8;
use crate::graphics::graphics::Graphics;
use crate::interrupts::hardware::pic8259::get_current_time_millis;
use crate::{point, rect};
use crate::drivers::vga::vga_fonts::VgaFont;
use crate::graphics::test_bitmap::get_my_cat_bitmap;

pub fn vga_demo(mut g: Graphics) {
    let radius: isize = 20;
    let cat_bitmap = get_my_cat_bitmap().unwrap();

    let mut coords = [
        point!(30, 30),   // filled rect
        point!(80, 50),   // rect outline
        point!(130, 70),  // filled triangle
        point!(180, 90),  // triangle outline
        point!(230, 110), // filled ellipse
        point!(280, 130), // ellipse outline
        point!(330, 150), // line
        point!(100, 180), // char
        point!(200, 40),  // string
    ];

    let mut velocities = [
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
        (1, 1),
        (-1, 1),
    ];

    let colors = [
        ColorU8::GREEN,
        ColorU8::BLUE,
        ColorU8::MAGENTA,
        ColorU8::RED,
        ColorU8::YELLOW,
        ColorU8::CYAN,
        ColorU8::WHITE,
        ColorU8::from_u24_rgb_to_u8(103, 48, 103),
        ColorU8::from_u24_rgb_to_u8(228, 96, 24),
    ];

    let mut previous_time = get_current_time_millis();

    loop {
        let current_time = get_current_time_millis();
        let mut delta_time = current_time - previous_time;
        if delta_time == 0 {
            delta_time = 1;
        }


        let fps = 1_000_000 / delta_time;
        let fps_str: String = format!("FPS: {}", fps);
        let d_time_str = format!("D_TIME: {}", delta_time);

        g.draw_bitmap(&point!(0, 0), &cat_bitmap);

        for i in 0..coords.len() {
            let (dx, dy) = velocities[i];
            let mut x = coords[i].x as isize;
            let mut y = coords[i].y as isize;

            g.set_color(colors[i]);
            g.set_font(VgaFont::FONT_16PX);
            let x_u = x as usize;
            let y_u = y as usize;

            match i {
                0 => g.fill_rect(&rect!(x_u, y_u, 20, 20)),
                1 => g.draw_rect(&rect!(x_u, y_u, 20, 20)),
                2 => g.fill_triangle(
                    &point!(x_u, y_u),
                    &point!(x_u + 20, y_u + 10),
                    &point!(x_u + 10, y_u + 20),
                ),
                3 => g.draw_triangle(
                    &point!(x_u, y_u),
                    &point!(x_u + 20, y_u + 10),
                    &point!(x_u + 10, y_u + 20),
                ),
                4 => g.fill_elipse(&point!(x_u + 10, y_u + 10), 10, 10),
                5 => g.draw_elipse(&point!(x_u + 10, y_u + 10), 10, 10),
                6 => g.draw_line(&point!(x_u, y_u), &point!(x_u + 20, y_u + 20)),
                7 => g.draw_char(&point!(x_u, y_u), 'A'),
                8 => g.draw_str(&point!(x_u, y_u), "Welcome to VGA!"),
                _ => {}
            }

            if x + dx + radius >= g.get_video_width() as isize || x + dx < 0 {
                velocities[i].0 = -dx;
            }

            if y + dy + radius >= g.get_video_height() as isize || y + dy < 0 {
                velocities[i].1 = -dy;
            }

            coords[i].x = (x + velocities[i].0) as usize;
            coords[i].y = (y + velocities[i].1) as usize;
        }

        g.set_font(VgaFont::FONT_8PX);
        g.set_color(ColorU8::WHITE);
        g.draw_str(&point!(10, 10), fps_str.as_str());
        g.draw_str(&point!(10, 20), d_time_str.as_str());

        g.update();
        previous_time = current_time;
        g.set_color(ColorU8::BLACK);
        g.clear();
    }
}
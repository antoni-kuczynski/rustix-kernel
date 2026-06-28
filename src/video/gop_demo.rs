/*
 * Created by Antoni Kuczyński
 * 18/06/2026
 */
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use crate::drivers::apic::apic::timer_lapic_uptime_ms;
use crate::misc::prng::prng_next_isize;
use crate::video::framebuffer::{fb_height, fb_width, Framebuffer, FramebufferColor, FRAMEBUFFER};


struct Ball {
    pub x: f32,
    pub y: f32,
    radius: f32,
    color: FramebufferColor,
    pub direction_x: f32,
    pub direction_y: f32,
    speed: f32
}

impl Ball {
    fn new_random() -> Self {
        let mut dir_x = prng_next_isize(-5, 5);
        let mut dir_y = prng_next_isize(-5, 5);

        while dir_x == 0 {
            dir_x = prng_next_isize(-5, 5);
        }

        while dir_y == 0 {
            dir_y = prng_next_isize(-5, 5);
        }

        let radius = prng_next_isize(10, 15);
        let x = prng_next_isize(radius, fb_width_isize() - radius);
        let y = prng_next_isize(-1600, -1500);

        Self {
            radius: radius as f32,
            color: FramebufferColor::from_rgb(
                prng_next_isize(0, 255) as u32,
                prng_next_isize(0, 255) as u32,
                prng_next_isize(0, 255) as u32,
            ),
            direction_x: dir_x as f32,
            direction_y: dir_y as f32,
            x: x as f32,
            y: y as f32,
            speed: prng_next_isize(200, 200) as f32
        }
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.x += x;
        self.y += y;
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.draw_filled_circle(self);
    }

    fn scale(&self, val: isize, delta_time: f32) -> f32 {
        val as f32 * self.speed * delta_time
    }

    fn collides_with(&self, other: &Ball) -> bool {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let distance_squared = dx*dx + dy*dy;
        let radius_times_two = self.radius + other.radius;
        let target_distance_squared = radius_times_two * radius_times_two;

        distance_squared < target_distance_squared
    }

    fn update(&mut self, delta_time: f32) {
        let gravity = 980.0;
        let dampening = 0.75;
        let max_velocity = 1500.0;

        let resting_threshold = 60.0;

        self.direction_y += gravity * delta_time;

        let width = fb_width_isize() as f32;
        let height = fb_height_isize() as f32;
        let radius_f = self.radius;

        if self.x - radius_f <= 0.0 {
            self.direction_x = self.direction_x.abs() * dampening;
            self.x = radius_f;
        }

        if self.x + radius_f >= width {
            self.direction_x = -self.direction_x.abs() * dampening;
            self.x = width - radius_f;
        }

        if self.y - radius_f <= 0.0 {
            self.direction_y = self.direction_y.abs() * dampening;
            self.y = radius_f;
        }

        if self.y + radius_f >= height {
            self.y = height - radius_f;

            if self.direction_y < resting_threshold {
                self.direction_y = 0.0;
                self.direction_x *= 0.90;
            } else {
                self.direction_y = -self.direction_y.abs() * dampening;
            }
        }

        if self.direction_x > max_velocity { self.direction_x = max_velocity; }
        if self.direction_x < -max_velocity { self.direction_x = -max_velocity; }

        if self.direction_y > max_velocity { self.direction_y = max_velocity; }
        if self.direction_y < -max_velocity { self.direction_y = -max_velocity; }

        self.x += self.direction_x * delta_time;
        self.y += self.direction_y * delta_time;
    }
}

fn fb_width_isize() -> isize{
    fb_width() as isize
}

fn fb_height_isize() -> isize{
    fb_height() as isize
}

fn update(delta_time: f32, balls: &mut Vec<Ball>, fb: &Framebuffer) {
    for ball in balls.iter_mut() {
        ball.update(delta_time);
    }

    for i in 0..balls.len() {
        let (left, right) = balls.split_at_mut(i + 1);
        let ball_a = &mut left[i];

        for ball_b in right.iter_mut() {
            let dx = ball_b.x - ball_a.x;
            let dy = ball_b.y - ball_a.y;
            let distance_squared = dx * dx + dy * dy;
            let min_distance = ball_a.radius + ball_b.radius;

            if distance_squared >= min_distance * min_distance || distance_squared == 0.0 {
                continue;
            }

            let distance = sqrt_f32(distance_squared);

            let normal_x = dx / distance;
            let normal_y = dy / distance;

            let overlap = min_distance - distance;

            let separation_amount = overlap * 0.5;

            ball_a.x -= normal_x * separation_amount;
            ball_a.y -= normal_y * separation_amount;

            ball_b.x += normal_x * separation_amount;
            ball_b.y += normal_y * separation_amount;

            let vax = ball_a.direction_x;
            let vay = ball_a.direction_y;
            let vbx = ball_b.direction_x;
            let vby = ball_b.direction_y;

            let rel_vel_x = vbx - vax;
            let rel_vel_y = vby - vay;

            let velocity_along_normal = rel_vel_x * normal_x + rel_vel_y * normal_y;

            if velocity_along_normal > 0.0 {
                continue;
            }

            let dot_a = vax * normal_x + vay * normal_y;
            let dot_b = vbx * normal_x + vby * normal_y;

            ball_a.direction_x = vax - (dot_a - dot_b) * normal_x;
            ball_a.direction_y = vay - (dot_a - dot_b) * normal_y;
            ball_b.direction_x = vbx - (dot_b - dot_a) * normal_x;
            ball_b.direction_y = vby - (dot_b - dot_a) * normal_y;
        }
    }
}

fn draw(delta_time: f32, balls: &Vec<Ball>, fb_copy: &mut Vec<u8>, fb_length: usize, fb: &mut Framebuffer) {
    for ball in balls {
        ball.draw(fb);
    }
    fb.swap_buffers();
    unsafe { ptr::copy_nonoverlapping(fb_copy.as_mut_ptr(), fb.back_buffer.as_mut_ptr(), fb_length) };

    // fb.clear();
}

pub static IS_DEMO_RUNNING: bool = true;

pub fn demo() {
    let mut lock = FRAMEBUFFER.lock();
    let fb = lock.as_mut().unwrap();

    let amount_of_balls = fb.width() / 10;
    let mut balls: Vec<Ball> = vec![];
    for i in 0..amount_of_balls {
        balls.push(Ball::new_random());
    }


    let fb_length = fb.height() * fb.pitch();
    let mut fb_copy: Vec<u8> = Vec::with_capacity(fb_length);
    unsafe { ptr::copy_nonoverlapping(fb.base, fb_copy.as_mut_ptr(), fb_length) };

    let running = true;
    let mut counter = 0;
    let counter_max = 15;
    let mut prev_fps = 0;

    fb.current_foreground = FramebufferColor::from_rgb(255,255,255);
    fb.current_background = FramebufferColor::from_rgb(0,0,0);

    let str = "FPS: ";
    let str1 = "DTIME: ";
    let mode = fb.width().to_string() + "x" + &*fb.height().to_string() + "x" + &*fb.bpp().to_string();
    let mut previous_time = timer_lapic_uptime_ms();
    while running {
        let current_time = timer_lapic_uptime_ms();
        let delta_time: f32 = (current_time - previous_time) as f32 / 1000f32;

        if counter < counter_max {
            counter += 1;
        } else {
            prev_fps = (1.0 / delta_time) as usize; //on qemu with kvm delta_time = 0 most of the time, it's always nice having infinite FPS
            counter = 0;
        }

        fb.put_string_no_bg(fb.width() - 55 - 80,5, str);
        fb.put_string_no_bg(fb.width() - 85,5, prev_fps.to_string().as_str());

        fb.put_string_no_bg(fb.width() - 55 - 80,20, str1);
        fb.put_string_no_bg(fb.width() - 75,20, delta_time.to_string().as_str());

        fb.put_string_no_bg(fb.width() - 55 - 80,36, mode.as_str());

        update(delta_time, &mut balls, &fb);
        draw(delta_time, &balls, &mut fb_copy, fb_length, fb);

        previous_time = current_time;
    }


}

pub fn sqrt_f32(n: f32) -> f32 {
    if n <= 0.0 {
        return 0.0;
    }
    let mut x = n;
    for _ in 0..10 {
        x = 0.5 * (x + n / x);
    }
    x
}

fn round_f32_to_isize(n: f32) -> isize {
    if n >= 0.0 {
        (n + 0.5) as isize
    } else {
        (n - 0.5) as isize
    }
}


pub fn point_at_distance(x1: f32, y1: f32, x2: f32, y2: f32, d: f32) -> (f32, f32) {
    let fx1 = x1;
    let fy1 = y1;
    let fx2 = x2;
    let fy2 = y2;

    let dx = fx2 - fx1;
    let dy = fy2 - fy1;

    let total_distance = sqrt_f32(dx * dx + dy * dy);

    if total_distance == 0.0 {
        return (x1, y1);
    }

    let ratio = d / total_distance;

    let final_x = fx1 + (ratio * dx);
    let final_y = fy1 + (ratio * dy);

    (final_x, final_y)
}

pub fn min_isize(a: isize, b: isize) -> isize {
    if a < b { a } else { b }
}

pub fn max_isize(a: isize, b: isize) -> isize {
    if a > b { a } else { b }
}

pub fn min_f32(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

pub fn max_f32(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

impl Framebuffer {
    fn draw_filled_circle(&mut self, ball: &Ball) {
        let color = &ball.color;
        let cx = ball.x as isize;
        let cy = ball.y as isize;
        let r = ball.radius as isize;

        if r <= 0 {
            return;
        }

        let mut x = 0;
        let mut y = r;
        let mut d = 3 - 2 * r;

        while x <= y {
            self.draw_h_line(cx - x, cx + x, cy + y, color);
            self.draw_h_line(cx - x, cx + x, cy - y, color);
            self.draw_h_line(cx - y, cx + y, cy + x, color);
            self.draw_h_line(cx - y, cx + y, cy - x, color);

            if d < 0 {
                d = d + 4 * x + 6;
            } else {
                d = d + 4 * (x - y) + 10;
                y -= 1;
            }
            x += 1;
        }
    }

    fn draw_h_line(&mut self, mut x1: isize, mut x2: isize, y: isize, color: &FramebufferColor) {
        if x1 > x2 {
            core::mem::swap(&mut x1, &mut x2);
        }

        if y < 0 || y >= self._pixel_info.height as isize {
            return;
        }

        let start_x = x1.max(0);
        let end_x = x2.min((self._pixel_info.width - 1) as isize);

        if start_x > end_x {
            return;
        }

        let safe_y = y as usize;
        let bpp_bytes = (self.bpp() as usize) >> 3;
        let mut current_offset = safe_y * self.pitch() + (start_x as usize) * bpp_bytes;
        let end_offset = safe_y * self.pitch() + (end_x as usize) * bpp_bytes;

        while current_offset <= end_offset {
            unsafe {
                self.write_raw_pixel_24(
                    current_offset,
                    color.data,
                    self.bpp() as usize
                );
            };

            current_offset += bpp_bytes;
        }
    }
}

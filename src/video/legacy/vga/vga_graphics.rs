#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 26/09/2025
 */
use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use core::{mem, ptr};
use crate::memory::_P2V_kernel;
use crate::video::legacy::vga::CURRENT_VGA_MODE;
use crate::video::legacy::vga::registers::vga_io::*;
use crate::video::legacy::vga::vga_fonts::VgaFont;

pub struct VgaVideoMode<const BUF_SIZE: usize> {
    pub video_width_px: usize, //res width
    pub video_height_px: usize, //res height
    pub color_depth_bits: usize, //color depth
    pitch: usize, //how many bytes of VRAM you should skip to go one pixel down
    pixel_width: usize, //how many bytes of VRAM you should skip to go one pixel right
    mode_value: u8, //the mode value in hex
    back_buffer_heap: Vec<u8>,
    video_buffer_vga1: &'static mut [u8; BUF_SIZE],
    video_buffer_vga2: &'static mut [u8; BUF_SIZE],
    current_write_plane: u8,
    active_buf: &'static mut[u8; BUF_SIZE]
}

impl<const BUF_SIZE: usize> VgaVideoMode<BUF_SIZE> {

    //=============================================================================================
    //
    //  MODE 0x13 320x200px 256 colors
    //
    //=============================================================================================

    pub fn vga13h_update(&mut self) {
        self.back_buffer_heap.resize(BUF_SIZE, 0);
        self.video_buffer_vga1.copy_from_slice(&*self.back_buffer_heap);
    }

    pub fn vga13h_put_pixel(&mut self, pos_x: usize, pos_y: usize, color: u8) {
        let location = self.video_width_px * pos_y + pos_x;
        self.video_buffer_vga1[location] = color;
    }

    pub fn _vga13h_draw_char_transparent(
        &mut self,
        x: usize,
        y: usize,
        c: char,
        font: &VgaFont,
        foreground: u8,
    ) {
        if x >= self.video_width_px || y >= self.video_height_px {
            return;
        }

        let mut char_index = c as usize;
        if char_index < font.lochar || char_index > font.hichar {
            char_index = '?' as usize; //unknown character
        }

        // in bitmap fonts each pixel is represented by 1bit
        // if 1 → draw foreground color, if 0 → don't draw
        let mut source_char_byte: usize = (char_index - font.lochar) * font.bytes_per_char;

        for row in 0..font.height {
            let dest_y = y as isize + row as isize;
            if dest_y < 0 || dest_y >= self.video_height_px as isize {
                source_char_byte += 1;
                continue;
            }

            let mut dest_x = x as isize;
            for w in (0..font.width).rev() {
                if font.mem[source_char_byte] & (0x80 >> w) != 0 {
                    if dest_x >= 0 && dest_x < self.video_width_px as isize {
                        let offset = (dest_y as usize) * self.pitch + (dest_x as usize);
                        self.back_buffer_heap[offset] = foreground;
                    }
                }
                dest_x += 1;
            }

            source_char_byte += 1;
        }
    }


    pub fn _vga13h_draw_string(
        &mut self,
        x: usize, y: usize,
        text: &str,
        font: &VgaFont,
        foreground: u8)
    {
        if x >= self.video_width_px || y >= self.video_height_px {
            return;
        }

        for (i, c) in text.chars().enumerate() {
            self._vga13h_draw_char_transparent(x + i * font.width, y, c, font, foreground);
        }
    }

    pub fn _vga13h_draw_bitmap(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        mem: &Vec<u8>,
    ) {
        if x >= self.video_width_px || y >= self.video_height_px {
            return;
        }

        let draw_w = if x + width > self.video_width_px {
            self.video_width_px - x
        } else {
            width
        };
        let draw_h = if y + height > self.video_height_px {
            self.video_height_px - y
        } else {
            height
        };

        let mut mem_ptr = 0;
        let mut pixel_ptr = y * self.pitch + x;

        for _j in 0..draw_h {
            //copy only the visible part of this row
            let row_slice = &mem[mem_ptr..mem_ptr + draw_w];
            let dest_slice = &mut self.back_buffer_heap[pixel_ptr..pixel_ptr + draw_w];
            dest_slice.copy_from_slice(row_slice);

            mem_ptr += width; // advance by full source row
            pixel_ptr += self.pitch;
        }
    }


    pub fn _vga13h_draw_line(&mut self,
                             mut x0: usize, mut y0: usize,
                             mut x1: usize, mut y1: usize,
                             color: u8)
    {
        if (x0 >= self.video_width_px && x1 >= self.video_width_px) || (y0 >= self.video_height_px && y1 >= self.video_height_px) {
            return;
        }

        let width  = self.video_width_px  as isize;
        let height = self.video_height_px as isize;

        //Cohen–Sutherland outcode constants
        const INSIDE: u8 = 0; // 0000
        const LEFT:   u8 = 1; // 0001
        const RIGHT:  u8 = 2; // 0010
        const BOTTOM: u8 = 4; // 0100
        const TOP:    u8 = 8; // 1000

        fn compute_outcode(x: isize, y: isize, w: isize, h: isize) -> u8 {
            let mut code = INSIDE;
            if x < 0     { code |= LEFT; }
            else if x >= w { code |= RIGHT; }
            if y < 0     { code |= TOP; }
            else if y >= h { code |= BOTTOM; }
            code
        }

        let mut x0_i = x0 as isize;
        let mut y0_i = y0 as isize;
        let mut x1_i = x1 as isize;
        let mut y1_i = y1 as isize;

        let mut outcode0 = compute_outcode(x0_i, y0_i, width, height);
        let mut outcode1 = compute_outcode(x1_i, y1_i, width, height);

        loop {
            if (outcode0 | outcode1) == 0 {
                // both inside
                break;
            } else if (outcode0 & outcode1) != 0 {
                // both outside same region
                return;
            } else {
                // at least one endpoint is outside
                let outcode_out = if outcode0 != 0 { outcode0 } else { outcode1 };
                let mut x = 0;
                let mut y = 0;

                if (outcode_out & TOP) != 0 {
                    x = x0_i + (x1_i - x0_i) * (0 - y0_i) / (y1_i - y0_i);
                    y = 0;
                } else if (outcode_out & BOTTOM) != 0 {
                    x = x0_i + (x1_i - x0_i) * (height - 1 - y0_i) / (y1_i - y0_i);
                    y = height - 1;
                } else if (outcode_out & RIGHT) != 0 {
                    y = y0_i + (y1_i - y0_i) * (width - 1 - x0_i) / (x1_i - x0_i);
                    x = width - 1;
                } else if (outcode_out & LEFT) != 0 {
                    y = y0_i + (y1_i - y0_i) * (0 - x0_i) / (x1_i - x0_i);
                    x = 0;
                }

                if outcode_out == outcode0 {
                    x0_i = x; y0_i = y;
                    outcode0 = compute_outcode(x0_i, y0_i, width, height);
                } else {
                    x1_i = x; y1_i = y;
                    outcode1 = compute_outcode(x1_i, y1_i, width, height);
                }
            }
        }

        // clamp back to usize
        x0 = x0_i as usize;
        y0 = y0_i as usize;
        x1 = x1_i as usize;
        y1 = y1_i as usize;

        //both endpoints are guaranteed inside the screen

        if x0 == x1 {
            let (y_start, y_end) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            let mut pixel = y_start * self.pitch + x0;
            for _ in y_start..=y_end {
                self.back_buffer_heap[pixel] = color;
                pixel += self.pitch;
            }
            return;
        }

        if y0 == y1 {
            let (x_start, x_end) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
            let base = y0 * self.pitch;
            self.back_buffer_heap[base + x_start ..= base + x_end].fill(color);
            return;
        }

        //TODO: use VGA write mode 3 for better performance
        //bresenham's line drawing algorithm
        let mut pos = (x0 as isize, y0 as isize);
        let dx: isize = (x1 as isize - x0 as isize).abs();
        let dy: isize = -(y1 as isize - y0 as isize).abs();
        let step_x: isize = if x0 < x1 { 1 } else { -1 };
        let step_y: isize = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;

        loop {
            let index = pos.1 * width + pos.0;
            self.back_buffer_heap[index as usize] = color;

            if pos.0 == x1 as isize && pos.1 == y1 as isize {
                break;
            }

            let error_2 = error * 2;
            if error_2 >= dy {
                error += dy;
                pos.0 += step_x;
            }
            if error_2 <= dx {
                error += dx;
                pos.1 += step_y;
            }
        }
    }


    pub fn _vga13h_fill_triangle(&mut self,
                                 x0: usize, y0: usize,
                                 x1: usize, y1: usize,
                                 x2: usize, y2: usize,
                                 color: u8)
    {
        let mut x0_m = x0;
        let mut y0_m = y0;
        let mut x1_m = x1;
        let mut y1_m = y1;
        let mut x2_m = x2;
        let mut y2_m = y2;

        if (x0 >= self.video_width_px || y0 >= self.video_height_px)
            && (x1 >= self.video_width_px || y1 >= self.video_height_px)
            && (x2 >= self.video_width_px || y2 >= self.video_height_px)
        {
            return;
        }

        //sort the vertices by Y
        if y0_m > y2_m { (x0_m, y0_m, x2_m, y2_m) = (x2_m, y2_m, x0_m, y0_m); }
        if y1_m > y2_m { (x1_m, y1_m, x2_m, y2_m) = (x2_m, y2_m, x1_m, y1_m); }
        if y0_m > y1_m { (x0_m, y0_m, x1_m, y1_m) = (x1_m, y1_m, x0_m, y0_m); }

        //now the triangle should look like this:
        /*
            P0(x0, y0)
                *
               / \
              /   \
             /     \
            *-------*
         P1(x1, y1)   P2(x2, y2)
         */

        if y1_m == y2_m {   //if these coords are equal the triangle is bottom flat
            self.vga13h_fill_bottom_flat_triangle(x0_m, y0_m, x1_m, y1_m, x2_m, y2_m, color);
        } else if y0_m == y1_m {    //if these coords are equal the triangle is top flat
            self.vga13h_fill_top_flat_triangle(x0_m, y0_m, x1_m, y1_m, x2_m, y2_m, color);
        } else {    //every other triangle is made of the flat top and flat bottom triangles
            if y2_m == y0_m { return; }

            let dx = (x2_m as isize - x0_m as isize) * (y1_m as isize - y0_m as isize) / (y2_m as isize - y0_m as isize);
            let x_split = (x0_m as isize + dx) as usize;
            let y_split = y1_m;

            /*
            //the Psplit point divides the P0,P2 line so that theres a flat line there y=const
            P0 ●
                \
                 \
            ..----*  ← Psplit(x_split, y1), line dividing the bottom and top triangle
                   \
                    \
                     ● P2
             */

            //draw both triangles
            self.vga13h_fill_bottom_flat_triangle(x0_m, y0_m, x1_m, y1_m, x_split, y_split, color);
            self.vga13h_fill_top_flat_triangle(x1_m, y1_m, x_split, y_split, x2_m, y2_m, color);
        }
    }

    fn vga13h_fill_bottom_flat_triangle(&mut self,
                                        x0: usize, y0: usize,
                                        x1: usize, y1: usize,
                                        x2: usize, y2: usize,
                                        color: u8)
    {
        /*
            P0(x0, y0)
                *
               / \
              /   \
             /     \
            *-------*
         P1(x1, y1)   P2(x2, y2)
         */

        //stupid isize cast
        let (x0_i, x1_i, x2_i, y0_i, y1_i, y2_i) =
            (x0 as isize, x1 as isize, x2 as isize, y0 as isize, y1 as isize, y2 as isize);

        //Y distances calculation
        let dy1: isize = y1_i - y0_i;
        let dy2: isize = y2_i - y0_i;

        //division by zero check
        if dy1 == 0 || dy2 == 0 { return; }

        //all the bit shifts are to not use the floating point numbers - improves pixel coords rounding a bit

        //calculate the slope step values
        let mut slope1:isize = ((x1_i - x0_i) << 16) / dy1;
        let mut slope2: isize = ((x2_i - x0_i) << 16) / dy2;

        //current x axis values
        let mut line_x1: isize = x0_i << 16;
        let mut line_x2: isize = x0_i << 16;

        //slope sorting by x - assures that we always start at left and go to right
        if slope2 < slope1 { (slope1, slope2) = (slope2, slope1); }

        // clamp Y range
        let y_start = y0.max(0).min(self.video_height_px - 1);
        let y_end   = y2.min(self.video_height_px - 1);

        // adjust starting X if clipped at top
        line_x1 += slope1 * ((y_start as isize) - y0_i);
        line_x2 += slope2 * ((y_start as isize) - y0_i);

        let mut y_coord = y_start * self.pitch;
        for _y in y_start..=y_end {
            let mut start = (line_x1 + 0x8000) >> 16;
            let mut end   = (line_x2 + 0x8000) >> 16;

            // clamp X range
            if start < 0 { start = 0; }
            if end >= self.video_width_px as isize { end = self.video_width_px as isize - 1; }

            if start <= end {
                self.back_buffer_heap[y_coord + start as usize ..= y_coord + end as usize].fill(color);
            }

            y_coord += self.pitch;
            line_x1 += slope1;
            line_x2 += slope2;
        }
    }

    fn vga13h_fill_top_flat_triangle(&mut self,
                                     x0: usize, y0: usize,
                                     x1: usize, y1: usize,
                                     x2: usize, y2: usize,
                                     color: u8)
    {
        /*
        P0(x0,y0)     P1(x1,y1)
           *-----------*
             \       /
              \     /
               \   /
                \ /
                 *
               P2(x2,y2)
         */

        //stupid isize cast
        let (x0_i, x1_i, x2_i, y0_i, y1_i, y2_i) =
            (x0 as isize, x1 as isize, x2 as isize, y0 as isize, y1 as isize, y2 as isize);

        //Y distances calculation
        let dy1: isize = y2_i - y0_i;
        let dy2: isize = y2_i - y1_i;

        //division by zero check
        if dy1 == 0 || dy2 == 0 { return; }

        //all the bit shifts are to not use the floating point numbers - improves pixel coords rounding a bit

        //calculate the slope step values
        let mut slope1: isize = ((x2_i - x0_i) << 16) / dy1;
        let mut slope2: isize = ((x2_i - x1_i) << 16) / dy2;

        //current x axis values
        let mut line_x1: isize = x2_i << 16;
        let mut line_x2: isize = x2_i << 16;

        //slope sorting by x - assures that we always start at left and go to right
        if slope2 > slope1 { (slope1, slope2) = (slope2, slope1); }

        // clamp Y range
        let y_start = y0.max(0).min(self.video_height_px - 1);
        let y_end   = y2.min(self.video_height_px - 1);

        // adjust starting X if clipped at bottom
        line_x1 -= slope1 * ((y2 as isize) - (y_end as isize));
        line_x2 -= slope2 * ((y2 as isize) - (y_end as isize));

        let mut y_coord = y_end * self.pitch;
        for _y in (y_start..=y_end).rev() {
            let mut start = (line_x1 + 0x8000) >> 16;
            let mut end   = (line_x2 + 0x8000) >> 16;

            // clamp X range
            if start < 0 { start = 0; }
            if end >= self.video_width_px as isize { end = self.video_width_px as isize - 1; }

            if start <= end {
                self.back_buffer_heap[y_coord + start as usize ..= y_coord + end as usize].fill(color);
            }

            y_coord -= self.pitch;
            line_x1 -= slope1;
            line_x2 -= slope2;
        }
    }


    pub fn _vga13h_draw_triangle(&mut self,
                                 x0: usize, y0: usize,
                                 x1: usize, y1: usize,
                                 x2: usize, y2: usize,
                                 color: u8)
    {
        //as simple as that (draw_line already optimized for drawing straight lines)
        self._vga13h_draw_line(x0, y0, x1, y1, color);
        self._vga13h_draw_line(x0, y0, x2, y2, color);
        self._vga13h_draw_line(x1, y1, x2, y2, color);
    }

    pub fn _vga13h_fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: u8,
    ) {
        let mut width = w;
        let mut height = h;

        if x >= self.video_width_px || y >= self.video_height_px {
            return;
        }

        if x + w > self.video_width_px {
            width = self.video_width_px - x;
        }
        if y + h > self.video_height_px {
            height = self.video_height_px - y;
        }

        unsafe {
            let mut line_ptr = self.back_buffer_heap.as_mut_ptr().add(y * self.pitch + x);

            for _ in 0..height {
                // Fill one horizontal line
                ptr::write_bytes(line_ptr, color, width);
                line_ptr = line_ptr.add(self.pitch);
            }
        }
    }


    pub fn _vga13h_draw_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: u8,
    ) {
        //WARNING: SHITTY CODE BELOW
        //I genuinely hate this mess of a code :((( - pls don't touch it as it can possibly destabilize
        //our universe's harmony (and make me mad)...

        if x >= self.video_width_px || y >= self.video_height_px {
            return;
        }

        if w == 0 || h == 0 {
            return;
        }

        let mut right_line_draw: bool = true;
        let width = if x + w > self.video_width_px {
            right_line_draw = false;
            self.video_width_px - x
        } else {
            w
        };

        let height = if y + h > self.video_height_px {
            self.video_height_px - y
        } else {
            let start_bottom = (y + h - 1) * self.pitch + x;
            self.back_buffer_heap[start_bottom..start_bottom + width].fill(color);

            h
        };

        let pixel_ptr = y * self.pitch + x;
        self.back_buffer_heap[pixel_ptr..pixel_ptr + width].fill(color);


        if right_line_draw {
            for row in (y + 1)..y + height {
                let left = row * self.pitch + x;
                let right = left + width - 1;

                self.back_buffer_heap[left] = color;
                self.back_buffer_heap[right] = color;
            }
        } else {
            for row in (y + 1)..y + height {
                let left = row * self.pitch + x;
                self.back_buffer_heap[left] = color;
            }
        }
    }


    //helper for draw&fill_elipse
    #[inline(always)]
    fn draw_hline_safe(&mut self, y: isize, x1: isize, x2: isize, color: u8) {
        if y < 0 || y >= self.video_height_px as isize {
            return;
        }

        let start = x1.max(0);
        let end = x2.min(self.video_width_px as isize - 1);
        if start > end {
            return;
        }

        let offset = (y as usize) * self.pitch;
        let buf = &mut self.back_buffer_heap[offset..offset + self.video_width_px];
        for x in start as usize..=end as usize {
            buf[x] = color;
        }
    }

    pub fn _vga13h_fill_ellipse(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: u8,
    ) {
        let (xi, yi, wi, hi) = (x as isize, y as isize, width as isize, height as isize);

        if xi + wi < 0 || xi - wi >= self.video_width_px as isize || yi + hi < 0 || yi - hi >= self.video_height_px as isize {
            return;
        }


        let wi2 = wi * wi;
        let hi2 = hi * hi;
        let hi2_2 = hi2 << 1;
        let wi2_2 = wi2 << 1;

        let mut xp = 0;
        let mut yp = hi;
        let mut dx = xp * hi2_2;
        let mut dy = yp * wi2_2;

        // region 1
        let mut d = hi2 - (wi2 * hi) + (wi2 / 4);
        while dx < dy {
            self.draw_hline_safe(yi + yp, xi - xp, xi + xp, color);
            self.draw_hline_safe(yi - yp, xi - xp, xi + xp, color);

            if d < 0 {
                xp += 1;
                dx += hi2_2;
                d += dx + hi2;
            } else {
                xp += 1;
                yp -= 1;
                dx += hi2_2;
                dy -= wi2_2;
                d += dx - dy + hi2;
            }
        }

        // region 2
        d = hi2 * (xp + 1).pow(2) + wi2 * (yp - 1).pow(2) - (wi2 * hi2);
        while yp >= 0 {
            self.draw_hline_safe(yi + yp, xi - xp, xi + xp, color);
            self.draw_hline_safe(yi - yp, xi - xp, xi + xp, color);

            if d > 0 {
                yp -= 1;
                dy -= wi2_2;
                d += wi2 - dy;
            } else {
                yp -= 1;
                xp += 1;
                dx += hi2_2;
                dy -= wi2_2;
                d += dx - dy + wi2;
            }
        }
    }

    pub fn _vga13h_draw_ellipse(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: u8,
    ) {
        let (xi, yi, wi, hi) = (x as isize, y as isize, width as isize, height as isize);

        if xi + wi < 0 || xi - wi >= self.video_width_px as isize || yi + hi < 0 || yi - hi >= self.video_height_px as isize {
            return;
        }
        
        let wi_squared = wi * wi;
        let hi_squared = hi * hi;
        let hi_times_two = hi_squared << 1;
        let wi_times_two = wi_squared << 1;

        let mut xp: isize = 0;
        let mut yp: isize = hi;
        let mut dx: isize = xp * hi_times_two;
        let mut dy: isize = yp * wi_times_two;

        //-=-=-=Region 1-=-=-=-=
        let mut d_region = hi_squared - (wi_squared * hi) + (wi_squared / 4);
        while dx < dy {
            // use draw_hline_safe with x1==x2 to plot single pixels, clipping included
            self.draw_hline_safe(yi + yp, xi + xp, xi + xp, color);
            self.draw_hline_safe(yi + yp, xi - xp, xi - xp, color);
            self.draw_hline_safe(yi - yp, xi + xp, xi + xp, color);
            self.draw_hline_safe(yi - yp, xi - xp, xi - xp, color);

            if d_region < 0 {
                xp += 1;
                dx += hi_times_two;
                d_region += dx + hi_squared;
            } else {
                xp += 1;
                yp -= 1;
                dx += hi_times_two;
                dy -= wi_times_two;
                d_region += dx - dy + hi_squared;
            }
        }

        //-=-=-=-=Region 2=-=-=-=-=
        d_region = hi_squared * (xp + 1).pow(2)
            + wi_squared * (yp - 1).pow(2) - (wi_squared * hi_squared);
        while yp >= 0 {
            self.draw_hline_safe(yi + yp, xi + xp, xi + xp, color);
            self.draw_hline_safe(yi + yp, xi - xp, xi - xp, color);
            self.draw_hline_safe(yi - yp, xi + xp, xi + xp, color);
            self.draw_hline_safe(yi - yp, xi - xp, xi - xp, color);

            if d_region > 0 {
                yp -= 1;
                dy -= wi_times_two;
                d_region += wi_squared - dy;
            } else {
                yp -= 1;
                xp += 1;
                dx += hi_times_two;
                dy -= wi_times_two;
                d_region += dx - dy + wi_squared;
            }
        }
    }


    pub fn _vga13h_clear_back_buffer(&mut self, color: u8) {
        self.back_buffer_heap.fill(color);
    }

    pub fn _vga13h_clear_front_buffer(&mut self, color: u8) {
        for i in 0..BUF_SIZE {
            self.video_buffer_vga1[i] = color;
        }
    }

    pub fn new_vga_0x13_320x200_256color_mode() -> VgaVideoMode<64000> {
        VgaVideoMode {
            video_width_px: 320,
            video_height_px: 200,
            color_depth_bits: 8,
            pitch: 320,
            pixel_width: 1,
            mode_value: 0x13,
            video_buffer_vga1: unsafe { &mut *(_P2V_kernel(0xA0000) as *mut [u8; 64000]) },
            back_buffer_heap: vec![0x00; 64000],
            video_buffer_vga2: unsafe { &mut *(_P2V_kernel(0xA0000) as *mut [u8; 64000]) }, //not used here
            current_write_plane: 0x00, //not used here,
            active_buf: unsafe { &mut *(_P2V_kernel(0xA0000) as *mut [u8; 64000]) } //not used here
        }
    }

    #[allow(non_snake_case)]
    pub fn new_vga_mode_X_320x200_256color() -> VgaVideoMode<64000> {
        VgaVideoMode {
            video_width_px: 320,
            video_height_px: 200,
            color_depth_bits: 8,
            pitch: 0,
            pixel_width: 0,
            mode_value: 0x16,
            video_buffer_vga1: unsafe { &mut *(0xA0000 as *mut [u8; 64000]) },
            back_buffer_heap: vec![0;0], //not used in this mode
            video_buffer_vga2: unsafe {
                &mut *(0xB0000 as *mut [u8; 64000])
            },
            current_write_plane: 0x00,
            active_buf: unsafe { &mut *(0xA0000 as *mut [u8; 64000]) }
        }
    }

    pub fn vga13h_init(&mut self) {
        if CURRENT_VGA_MODE.lock().get() == Some(0x13) {
            return;
        }
        unsafe {
            asm!("cli");
            set_13h_mode_regs();

            //Setting the color pallete
            load_8bit_color_pallet_into_dac();
            asm!("sti");
        }
        self._vga13h_clear_front_buffer(0x00);
        self._vga13h_clear_back_buffer(0x00);
        CURRENT_VGA_MODE.lock().switch_to(0x13);
    }

    //==========================================================================
    //
    //  UNFINISHED MODE X AND 12h FUNCTIONS
    //  maybe will finish later idk
    //
    //==========================================================================

    #[allow(non_snake_case)]
    pub fn vga_320_200_mode_X_init(&mut self) {
        if CURRENT_VGA_MODE.lock().get() == Some(0x16) {
            return;
        }
        unsafe {
            asm!("cli");
            set_320_200_mode_x_mode_regs();

            //Setting the color pallete
            load_8bit_color_pallet_into_dac();
            asm!("sti");
        }
        // self._vga13h_clear_front_buffer();
        // self._vga13h_clear_back_buffer();
        CURRENT_VGA_MODE.lock().switch_to(0x16);
    }

    #[allow(non_snake_case)]
    pub fn _vga_320_200_X_clear_front_buffer(&mut self) {
        unsafe {
            set_write_planes(0b1111);
            for i in 0..= 16_000 {
                self.video_buffer_vga1[i] = 0x00;
            }

        }
    }

    #[allow(non_snake_case)]
    pub fn _vga_320_200_X_next_plane(&mut self) {
        // (self.current_write_plane << 1) | (self.current_write_plane >> 0x03);
    }

    #[allow(non_snake_case)]
    pub fn _vga_320_200_X_swap_buffers(&mut self) {
        mem::swap(&mut self.video_buffer_vga1, &mut self.video_buffer_vga2);    }

    #[allow(non_snake_case)]
    pub fn _vga_320_200_X_put_pixel(&mut self, x: usize, y: usize, color: u8) {
        let ptr = (y<<6) + (y<<4) + (x>>2);
        // let r = (color >> 5) & 0b00000111;
        // let g = (color >> 2) & 0b00000111;
        // let b = color & 0b00000011;
        unsafe {
            set_write_planes(0b1000);
            self.video_buffer_vga1[ptr] = color;

        }
    }

    #[allow(non_snake_case)]
    pub fn _vga_320_200_X_fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        unsafe {
            set_write_planes(0b1111);
            let mut ptr = (y<<6) + (y<<4) + (x>>2);
            for _j in y..=y+height {
                for _i in x..= x+width {
                    self.video_buffer_vga1[ptr] = color;
                    ptr += 1;
                }
                ptr = (y<<6) + (y<<4) + (x>>2)
            }

        }
    }

    pub fn new_vga_0x12_640x480_16color_mode() -> VgaVideoMode<64000> {
        VgaVideoMode {
            video_width_px: 640,
            video_height_px: 480,
            color_depth_bits: 4,
            pitch: 80,
            pixel_width: 0, //doesnt do anything in planar mode
            mode_value: 0x12,
            video_buffer_vga1: unsafe { &mut *(0xA0000 as *mut [u8; 64000]) },
            back_buffer_heap: vec![0x00; 64000],
            video_buffer_vga2: unsafe { //not used in this mode
                &mut *(0xA0000 as *mut [u8; 64000])
            },
            current_write_plane: 0x00,
            active_buf: unsafe { &mut *(0xA0000 as *mut [u8; 64000]) }
        }
    }

    pub fn vga12h_init(&mut self) {
        if CURRENT_VGA_MODE.lock().get() == Some(0x12) {
            return;
        }
        unsafe {
            asm!("cli");

            set_12h_mode_regs();

            //Setting the color pallete
            load_4bit_color_palette_into_dac();
            asm!("sti");
        }
        self._vga12h_clear_buffer();
        CURRENT_VGA_MODE.lock().switch_to(0x12);
    }

    pub fn _vga12h_clear_buffer(&mut self) {
        let buf_ptr = self.video_buffer_vga1.as_mut_ptr();

        unsafe {
            sequcencer_write(0x02,0x0F);    //Map Mask enable all planes
            graphics_controller_write(0x00,0x05);   //set the write mode to 0
            graphics_controller_write(0x08,0xFF);   //Bit Mask enable all bits
            // outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0005);
            // outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0xFF08);

            // for i in 0..640*480/8 {
            //     outw(VGA_SEQUENCER_INDEX, 0x0102);
            //     ptr::write_volatile(buf_ptr.add(i), 0xF);
            //
            //     outw(VGA_SEQUENCER_INDEX, 0x0202);
            //     ptr::write_volatile(buf_ptr.add(i), 0x0);
            //
            //     outw(VGA_SEQUENCER_INDEX, 0x0402);
            //     ptr::write_volatile(buf_ptr.add(i), 0x0);
            //
            //     outw(VGA_SEQUENCER_INDEX, 0x0802);
            //     ptr::write_volatile(buf_ptr.add(i), 0x0);
            // }

            for y in 0..480 {
                let line_offset = (y & 3) * 0x2000 + (y >> 2) * 80;
                let addr = buf_ptr.add(line_offset);
                for x in 0..80 {
                    sequcencer_write(0x02, 0x01);
                    ptr::write_volatile(addr.add(x), 0xFF);
                    sequcencer_write(0x02,0x02);
                    ptr::write_volatile(addr.add(x), 0x00);
                    sequcencer_write(0x02,0x04);
                    ptr::write_volatile(addr.add(x), 0x00);
                    sequcencer_write(0x02,0x08);
                    ptr::write_volatile(addr.add(x), 0x00);
                }
            }
        }
    }

    /*
    Notes for mode 0x12:
    - can use Map Mask register to write to only a specific VGA memory plane
    VGA memory layout for mode 0x12:
    MAP_0     |   MAP_1    |   MAP_2   |   MAP_3
    blue bit  |  green bit |  red bit  |  intensity

    (https://www.phatcode.net/res/224/files/html/ch23/23-05.html)
    Beware, however, of writing to an area of memory that is not zeroed.
    Planes that are disabled by the Map Mask register are not altered by CPU writes, so old and new images can mix on the screen,
    producing unwanted color effects as, say, three planes from the old image mix with one plane from the new image.
    You can solve this by ensuring that the memory written to is zeroed.
    A better way to set all planes at once is provided by the set/reset capabilities of the VGA.

    The sample program writes the image of the colored ball to VGA memory by enabling one plane at a time
    and writing the image of the ball for that plane. Each image is written to the same VGA addresses;
    only the destination plane, selected by the Map Mask register, is different.
    You might think of the ball’s image as consisting of four colored overlays, which together make up a multicolored image.
    The sample program writes a blank image to VGA memory by enabling all planes and writing a block of zero bytes;
    the zero bytes are written to all four VGA planes simultaneously.
     */
    //TODO: fixme
    pub fn vga12h_put_pixel(&mut self, pos_x: usize, pos_y: usize, color: u8) {
        let offset = pos_y * 80 + (pos_x >> 3);
        let mask = 0x80 >> (pos_x & 7);
        let buf_ptr = self.video_buffer_vga1.as_mut_ptr();

        unsafe {
            graphics_controller_write(0x05, 0x01);   //set write mode 1
            graphics_controller_write(0x03, 0x08);   //set the function operated on data in system latches to AND

            ptr::read_volatile(buf_ptr.add(offset));    //loading the VGA latches by reading the destination byte in video buffer
            ptr::write_volatile(buf_ptr.add(offset), !mask); //write the !mask to clear the target bit

            // gc_write(0x05, 0x00);   //set the write mode back to 0
            graphics_controller_write(0x00, color & 0x0F); //set the Set/Reset to the color's lower 4 bits
            /*
            Set/Reset register:
            7   6   5   4   3   2   1   0
            -   -   -   -   SR3 SR2 SR1 SR0
            SRn - set/reset for map n
            In write mode 0, the system writes the value of SRn to the nth memory map
             */
            graphics_controller_write(0x01, 0x0F);   //enable set/reset for all 4 planes
            graphics_controller_write(0x08, mask);   //set the bit mask register to select the pixel within the byte
            sequcencer_write(0x02, 0x0F);   //enable all planes in sequencer

            //write the color
            //in this case the data value is ignored and the pixel is determined by
            //Set/reset and Bit mask
            ptr::write_volatile(buf_ptr.add(offset), 0xFF);
            graphics_controller_write(0x03, 0x00);
        }
    }
}

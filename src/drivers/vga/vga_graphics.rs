#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 26/09/2025
 */
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr;
use crate::drivers::vga::CURRENT_VGA_MODE;
use crate::drivers::vga::vga_fonts::*;
use crate::drivers::vga::registers::vga_io::*;

pub struct VgaVideoMode<const BUF_SIZE: usize> {
    pub video_width_px: usize, //res width
    pub video_height_px: usize, //res height
    pub color_depth_bits: usize, //color depth
    pitch: usize, //how many bytes of VRAM you should skip to go one pixel down
    pixel_width: usize, //how many bytes of VRAM you should skip to go one pixel right
    mode_value: u8, //the mode value in hex
    video_buffer: &'static mut [u8; BUF_SIZE]
}

impl<const BUF_SIZE: usize> VgaVideoMode<BUF_SIZE> {
    pub fn vga13h_put_pixel(&mut self, pos_x: usize, pos_y: usize, color: u8) {
        let location = self.video_width_px * pos_y + pos_x;
        self.video_buffer[location] = color;
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
        let buf_ptr = self.video_buffer.as_mut_ptr();

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


    pub fn _vga13h_draw_char_transparent(
        &mut self,
        x: usize, y: usize,
        c: char,
        font: &VgaFont,
        foreground: u8)
    {
        assert!(x + font.width < self.video_width_px);
        assert!(y + font.height < self.video_height_px);
        let mut char_index = c as usize;
        if char_index < font.lochar || char_index > font.hichar {
            char_index = '?' as usize; //unknown character
        }

        //in bitmap fonts each pixel is represented by 1bit if 1 draw foreground color if 0 dont draw
        let mut source_char_byte: usize = (char_index - font.lochar) * font.bytes_per_char;
        let mut dest: usize = y * self.pitch + x;

        for _h in 0..font.height {
            for w in (0..font.width).rev() {
                if font.mem[source_char_byte] & (0x80 >> w) != 0 {  //check if a bit is 1 and we should draw
                    self.video_buffer[dest] = foreground;
                }
                dest += 1;
            }
            dest += self.pitch - font.width;
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
        //asserts are already in draw_char_transparent
        for (i, c) in text.chars().enumerate() {
            self._vga13h_draw_char_transparent(x + i * font.width, y, c, font, foreground);
        }
    }

    pub fn _vga13h_draw_bitmap(&mut self,
         x: usize, y: usize,
         width: usize, height: usize, mem: &Vec<u8>)
    {
        assert!(x + width <= self.video_width_px);
        assert!(y + height <= self.video_height_px);

        let mut j = 0;
        for l in 0..height {
            for i in 0..width {
                self.vga13h_put_pixel(x + i, y + l, mem[j]);
                j += 1;
            }
        }
    }

    pub fn _vga13h_draw_line(&mut self,
                             mut x0: usize, mut y0: usize,
                             mut x1: usize, mut y1: usize,
                             color: u8)
    {

        if x0 >= self.video_width_px ||
            y0 >= self.video_height_px ||
            x1 >= self.video_width_px ||
            y1 >= self.video_height_px
        {
            return;
        }

        if !(x0 < x1 && y0 < y1) {
            (x0,y0,x1,y1) = (x1,y1,x0,y0);
        }

        if x0 == x1 {
            let pos: (usize, usize) = if y0 < y1 {(y0, y1)} else {(y1, y0)};
            let mut pixel = pos.0 * self.pitch + x0;
            for _ in pos.0..=pos.1 {
                self.video_buffer[pixel] = color;
                pixel += self.pitch;
            }
            return;
        }

        if y0 == y1 {
            let pos: (usize, usize) = if x0 < x1 {(x0, x1)} else {(x1, x0)};
            let base = y0 * self.pitch;
                self.video_buffer[base + pos.0..=base + pos.1].fill(color);
            return;
        }

        //TODO: use VGA write mode 3 for better performance
        //bresengam's line drawing algorithm
        //error = amount that drawn pixel deviates from the actual vector (true) line
        // As the drawing of the line progresses from one pixel to the next, the error can be used to tell when,
        // given the resolution of the display, a more accurate approximation of the line can be drawn by placing a given pixel
        // one unit of screen resolution away from its predecessor in either the horizontal or the vertical direction, or both.
        let x0_isize: isize = x0 as isize;
        let y0_isize: isize = y0 as isize;
        let x1_isize: isize = x1 as isize;
        let y1_isize: isize = y1 as isize;
        let width: isize = self.video_width_px as isize;
        let mut pos: (isize, isize) = (x0_isize, y0_isize);
        let dx: isize = (x1_isize - x0_isize).abs();   //distance between x0 and x1
        let dy: isize = -(y1_isize - y0_isize).abs();
        let step_x: isize = if x0 < x1 {1} else {-1};  //direction the line is drawn
        let step_y: isize = if y0 < y1 {1} else {-1};
        let mut error = dx + dy;    //the accumulated error, used to determine


        loop {
            //fill the buffer with one step
            let index = pos.1 * width + pos.0;
            self.video_buffer[index as usize ..(index + 1) as usize].fill(color);

            let error_2 = error * 2;

            if dy <= error_2 { //if the error is less than dy move horizontaly
                error += dy;
                pos.0 += step_x;
            }

            if error_2 <= dx {
                error += dx;
                pos.1 += step_y;
            }

            //manually draw the last pixel
            if pos.0 == x1_isize && pos.1 == y1_isize {
                self.video_buffer[(pos.1 * width + pos.0) as usize] = color;
                break;
            }
        }
    }

    pub fn _vga13h_fill_triangle(&mut self,
                                 mut x0: usize, mut y0: usize,
                                 mut x1: usize, mut y1: usize,
                                 mut x2: usize, mut y2: usize,
                                 color: u8)
    {
        if x0 > self.video_width_px || y0 > self.video_height_px ||
            x1 > self.video_width_px || y1 > self.video_height_px ||
            x2 > self.video_width_px || y2 > self.video_height_px
        {
            return;
        }

        //sort the vertices by Y
        if y0 > y2 {
            (x0, y0, x2, y2) = (x2, y2, x0, y0);
        }

        if y1 > y2 {
            (x1, y1, x2, y2) = (x2, y2, x1, y1);
        }

        if y0 > y1 {
            (x0, y0, x1, y1) = (x1, y1, x0, y0);
        }

        //now the triangle should look likt this:
        /*
            P0(x0, y0)
                *
               / \
              /   \
             /     \
            *-------*
         P1(x1, y1)   P2(x2, y2)
         */

        if y1 == y2 {   //if these coords are equal the triangle is bottom flat
            self.vga13h_fill_bottom_flat_triangle(x0, y0, x1, y1, x2, y2, color);
        } else if y0 == y1 {    //if these coords are equal the triangle is top flat
            self.vga13h_fill_top_flat_triangle(x0, y0, x1, y1, x2, y2, color);
        } else {    //every other triangle is made of the flat top and flat bottom triangles
            if y2 - y0 == 0 {
                return;
            }

            let dx = (x2 as isize - x0 as isize) * (y1 as isize - y0 as isize) / (y2 as isize - y0 as isize);
            let x_split = (x0 as isize + dx) as usize;
            let y_split = y1;

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
            self.vga13h_fill_bottom_flat_triangle(x0, y0, x1, y1, x_split, y_split, color);
            self.vga13h_fill_top_flat_triangle(x1, y1, x_split, y_split, x2, y2, color);
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
        if dy1 == 0 || dy2 == 0 {
            return;
        }

        //all the bit shifts are to not use the floating point numbers - improves pixel coords rounding a bit

        //calculate the slope step values
        let mut slope1:isize = ((x1_i - x0_i) << 16) / dy1;
        let mut slope2: isize = ((x2_i - x0_i) << 16) / dy2;

        //current x axis values
        let mut line_x1: isize = x0_i << 16;
        let mut line_x2: isize = x0_i << 16;

        //slope sorting by x - assures that we always start at left and go to right
        if slope2 < slope1 {
            (slope1, slope2) = (slope2, slope1);
        }

        for y in y0..=y2 {
            let start = (line_x1 as usize + 0x8000) >> 16;  //0x8000 - pixel rounding logic
            let end = (line_x2 as usize + 0x8000) >> 16;
            self.video_buffer[y * self.pitch + start ..= y * self.pitch + end]  //fill the straight line from start to end
                .fill(color);

            //advance to the next line in the X axis
            //then advance to next line in the Y axis by incrementing y
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
        if dy1 == 0 || dy2 == 0 {
            return;
        }

        //all the bit shifts are to not use the floating point numbers - improves pixel coords rounding a bit

        //calculate the slope step values
        let mut slope1: isize = ((x2_i - x0_i) << 16) / dy1;
        let mut slope2: isize = ((x2_i - x1_i) << 16) / dy2;

        //current x axis values
        let mut line_x1: isize = x2_i << 16;
        let mut line_x2: isize = x2_i << 16;

        //slope sorting by x - assures that we always start at left and go to right
        if slope2 > slope1 {
            (slope1, slope2) = (slope2, slope1);
        }

        for y in (y0..=y2).rev() {
            let start = (line_x1 as usize + 0x8000) >> 16;  //0x8000 - pixel rounding logic
            let end = (line_x2 as usize + 0x8000) >> 16;
            self.video_buffer[y * self.pitch + start ..= y * self.pitch + end]
                .fill(color);   //fill the straight line from start to end

            //advance to the next line in the X axis
            //then advance to next line in the Y axis by incrementing y
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


    pub fn _vga13h_fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        assert!(x + width < self.video_width_px);
        assert!(y + height < self.video_height_px);
        let mut location: *mut u8 = self.video_buffer.as_mut_ptr();

        //minimize pointer calculation optimizations
        //dont recalculate every pixel, rather every line
        unsafe {
            location = location.add(self.pitch * y + x);
            for _j in y..y + height {
                let mut current_pixel_ptr = location;
                for _i in x..x + width {
                    ptr::write_volatile(current_pixel_ptr, color);
                    current_pixel_ptr = current_pixel_ptr.add(1);
                }
                location = location.add(self.pitch);
            }
        }
    }

    pub fn _vga13h_draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        assert!(x + width < self.video_width_px);
        assert!(y + height < self.video_height_px);
        //top line
        let start_top = y * self.pitch + x;
        self.video_buffer[start_top..(start_top + width)].fill(color);

        //bottom line
        let start_bottom = (y + height) * self.pitch + x;
        self.video_buffer[start_bottom..(start_bottom + width)].fill(color);

        //vertical lines
        for j in y..=y+height {
            let pixel_index = j * self.pitch + x;
            self.video_buffer[pixel_index] = color;
            self.video_buffer[pixel_index + width] = color;
        }
    }

    pub fn _vga13h_fill_elipse(&mut self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        let (xi,wi,hi) = (x as isize, width as isize, height as isize);

        let wi_squared = wi * wi;
        let hi_squared = hi * hi;

        let mut xp: isize = 0;
        let mut yp: isize = hi;
        let mut dx: isize = xp * (hi_squared << 1);
        let mut dy: isize = yp * (wi_squared << 1);

        //-=-=-=Region 1-=-=-=-=

        //decision parameter for region 1
        let mut d_region = hi_squared + ((xi * xi) >> 2) - (wi_squared * hi);
        while dx < dy {
            for i in x - xp as usize..=x + xp as usize {
                self.video_buffer[(yp as usize + y) * self.pitch + i] = color;
                self.video_buffer[(y - yp as usize) * self.pitch + i] = color;
            }

            if d_region < 0 {
                xp += 1;
                dx += hi_squared << 1;
                d_region += dx;
                d_region += hi_squared;
            } else {
                xp += 1;
                yp -= 1;
                dx += hi_squared << 1;
                dy -= wi_squared << 1;
                d_region += dx;
                d_region -= dy;
                d_region += hi_squared;
            }
        }

        //-=-=-=-=Region 2=-=-=-=-=
        d_region = hi_squared * ((xp + 1/2) * (xp + 1/2))
            + wi_squared * ((yp - 1) * (yp - 1)) - (wi_squared * hi_squared);
        while yp >= 0 {
            for i in x - xp as usize..=x + xp as usize {
                self.video_buffer[(yp as usize + y) * self.pitch + i] = color;
                self.video_buffer[(y - yp as usize) * self.pitch + i] = color;

            }

            if d_region > 0 {
                yp -= 1;
                dy -= wi_squared << 1;
                d_region += wi_squared;
                d_region -= dy;
            } else {
                yp -= 1;
                xp += 1;
                dx += hi_squared << 1;
                dy -= wi_squared << 1;
                d_region += dx;
                d_region -= dy;
                d_region += xi * xi;
            }
        }

    }

    pub fn _vga13h_draw_elipse(&mut self, x: usize, y: usize, width: usize, height: usize, color: u8) {
        let (xi,wi,hi) = (x as isize, width as isize, height as isize);

        let wi_squared = wi * wi;
        let hi_squared = hi * hi;

        let mut xp: isize = 0;
        let mut yp: isize = hi;
        let mut dx: isize = xp * (hi_squared << 1);
        let mut dy: isize = yp * (wi_squared << 1);

        //-=-=-=Region 1-=-=-=-=

        //decision parameter for region 1
        let mut d_region = hi_squared + ((xi * xi) >> 2) - (wi_squared * hi);
        while dx < dy {
            self.video_buffer[(yp as usize + y) * self.pitch + (xp as usize + x)] = color;
            self.video_buffer[(yp as usize + y) * self.pitch + (x - xp as usize)] = color;
            self.video_buffer[(y - yp as usize) * self.pitch + (x - xp as usize)] = color;
            self.video_buffer[(y - yp as usize) * self.pitch + (xp as usize + x)] = color;

            if d_region < 0 {
                xp += 1;
                dx += hi_squared << 1;
                d_region += dx;
                d_region += hi_squared;
            } else {
                xp += 1;
                yp -= 1;
                dx += hi_squared << 1;
                dy -= wi_squared << 1;
                d_region += dx;
                d_region -= dy;
                d_region += hi_squared;
            }
        }

        //-=-=-=-=Region 2=-=-=-=-=
        d_region = hi_squared * ((xp + 1/2) * (xp + 1/2))
            + wi_squared * ((yp - 1) * (yp - 1)) - (wi_squared * hi_squared);
        while yp >= 0 {
            self.video_buffer[(yp as usize + y) * self.pitch + (xp as usize + x)] = color;
            self.video_buffer[(yp as usize + y) * self.pitch + (x - xp as usize)] = color;
            self.video_buffer[(y - yp as usize) * self.pitch + (x - xp as usize)] = color;
            self.video_buffer[(y - yp as usize) * self.pitch + (xp as usize + x)] = color;

            if d_region > 0 {
                yp -= 1;
                dy -= wi_squared << 1;
                d_region += wi_squared;
                d_region -= dy;
            } else {
                yp -= 1;
                xp += 1;
                dx += hi_squared << 1;
                dy -= wi_squared << 1;
                d_region += dx;
                d_region -= dy;
                d_region += xi * xi;
            }
        }
    }

    pub fn _vga13h_clear_buffer(&mut self) {
        for i in 0..BUF_SIZE {
            self.video_buffer[i] = 0x00;
        }
    }

    pub fn vga12h_clear_buffer(&mut self) {
        let buf_ptr = self.video_buffer.as_mut_ptr();

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

    pub fn new_vga_0x13_320x200_256color_mode() -> VgaVideoMode<64000> {
        VgaVideoMode {
            video_width_px: 320,
            video_height_px: 200,
            color_depth_bits: 8,
            pitch: 320,
            pixel_width: 1,
            mode_value: 0x13,
            video_buffer: unsafe {
                &mut *(0xA0000 as *mut [u8; 64000])
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
            video_buffer: unsafe {
                &mut *(0xA0000 as *mut [u8; 64000])
            }
        }
    }

    pub fn vga_init_mode(&mut self) {
        match self.mode_value {
            0x13 => self.vga13h_init(),
            0x12 => self.vga12h_init(),
            _ => {}
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
        self.vga12h_clear_buffer();
        CURRENT_VGA_MODE.lock().switch_to(0x12);
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
        self._vga13h_clear_buffer();
        CURRENT_VGA_MODE.lock().switch_to(0x13);
    }
}

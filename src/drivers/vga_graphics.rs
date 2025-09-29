#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 26/09/2025
 */
use core::arch::asm;
use core::ptr;

#[inline(always)]
pub unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in ("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    unsafe {
        let value: u8;
        asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
        );
        value
    }
}

fn abs(x: isize) -> isize {
    if x < 0 {
        return -x
    }
    x
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h as u32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let (r, g, b) = (r1 + m, g1 + m, b1 + m);
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 200;
const BUFFER_LENGTH_BYTES: usize = VIDEO_WIDTH * VIDEO_HEIGHT;

#[derive(Clone, Copy)]
pub struct VgaVideoColor(pub u8);

pub struct VgaVideoMode {
    video_width_px: usize, //res width
    video_height_px: usize, //res height
    color_depth_bits: usize, //color depth
    pitch: usize, //how many bytes of VRAM you should skip to go one pixel down
    pixel_width: usize, //how many bytes of VRAM you should skip to go one pixel right
    mode_value: u8, //the mode value in hex
    video_buffer: &'static mut [VgaVideoColor; VIDEO_WIDTH * VIDEO_HEIGHT]
}

impl VgaVideoMode {
    pub fn put_pixel(&mut self, pos_x: usize, pos_y: usize, color: VgaVideoColor) {
        let location = self.video_width_px * pos_y + pos_x;
        self.video_buffer[location] = color;
    }

    pub fn draw_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, color: VgaVideoColor) {
        //TODO: use VGA write mode 3 for better performance
        //bresengam's line drawing algorithm
        //error = amount that drawn pixel deviates from the actual vector (true) line
        // As the drawing of the line progresses from one pixel to the next, the error can be used to tell when,
        // given the resolution of the display, a more accurate approximation of the line can be drawn by placing a given pixel
        // one unit of screen resolution away from its predecessor in either the horizontal or the vertical direction, or both.
        let width: isize = self.video_width_px as isize;
        let mut pos: (isize, isize) = (x0, y0);
        let dx: isize = abs(x1 - x0);   //distance between x0 and x1
        let dy: isize = -abs(y1 - y0);
        let step_x: isize = if x0 < x1 {1} else {-1};  //direction the line is drawn
        let step_y: isize = if y0 < y1 {1} else {-1};
        let mut error = dx + dy;    //the accumulated error, used to determine

        while pos.0 != x1 || pos.1 != y1 {
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
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: VgaVideoColor) {
        let mut location: *mut VgaVideoColor = self.video_buffer.as_mut_ptr();

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

    pub fn draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: VgaVideoColor) {
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

    pub fn clear_buffer(&mut self) {
        for i in 0..BUFFER_LENGTH_BYTES {
            self.video_buffer[i] = VgaVideoColor(0x0);
        }
    }

    pub fn new_vga_320x200_256_mode() -> Self {
        VgaVideoMode {
            video_width_px: VIDEO_WIDTH,
            video_height_px: VIDEO_HEIGHT,
            color_depth_bits: 8,
            pitch: 320,
            pixel_width: 1,
            mode_value: 0x13,
            video_buffer: unsafe {
                &mut *(0xA0000 as *mut [VgaVideoColor; VIDEO_WIDTH * VIDEO_HEIGHT])
            }
        }
    }

    pub fn init_mode(&mut self) {
        unsafe {
            asm!("cli");

            //Miscellaneous Output Register
            outb(0x03C2, 0x43);

            //CRT Control registers
            //Vertical Retrace End Register (index 0x11, set first to unlock registers 0x0 to 0x07)
            outw(0x03D4, 0x0E11);

            //Horizontal total register (index 0x00)
            outw(0x03D4, 0x5F00);

            //Horizontal Display-Enable End Register (index 0x01)
            outw(0x03D4, 0x4F01);

            //Start Horizontal Blanking Register (index 0x02)
            outw(0x03D4, 0x5002);

            //End Horizontal Blanking Register (index 0x03)
            outw(0x03D4, 0x8203);

            //Start Horizontal Retrace Pulse Register (index 0x04)
            outw(0x03D4, 0x5404);

            //End Horizontal Retrace Register (index 0x05)
            outw(0x03D4, 0x8005);

            //Offset Register (0x13)
            outw(0x03D4, 0x2813);

            //Vertical Total Register (index 0x06)
            outw(0x03D4, 0xBF06);

            //Overflow Register (0x07)
            outw(0x03D4, 0x1F07);

            //Maximum scan line regisyer (index 0x09)
            outw(0x03D4, 0x4109);

            //Vertical Retrace Start Register (index 0x10)
            outw(0x03D4, 0x9C10);

            //Vertical Retrace End Register (0x11) again
            outw(0x03D4, 0x8E11);

            //Vertical Display-Enable End Register (0x12)
            outw(0x03D4, 0x8F12);

            //Start Vertical Blanking Register (0x15)
            outw(0x03D4, 0x9615);

            //End Vertical Blanking Register (0x16)
            outw(0x03D4, 0xB916);

            //Preset row scan register (index 0x08)
            outw(0x03D4, 0x0008);

            //Underline Location Register (index 0x14)
            outw(0x03D4, 0x4014);

            //CRT Mode Control Register (0x17)
            outw(0x03D4, 0xA317);


            //Sequencer registers (0x03C4):
            //Memory mode register (index 0x04)
            outw(0x03C4, 0xE04);

            //Clocking mode register (index 0x01)
            outw(0x03C4, 0x0B01);

            //Map mask register (index 0x02)
            outw(0x03C4, 0x0F02);

            //Graphics controller registers (0xCE):
            outw(0x03CE, 0x4005);

            //Miscellaneous Register (index 0x06)
            outw(0x03CE, 0x0506);


            //Attribute controller registers:
            inb(0x03DA);
            outb(0x03C0, 0x30);
            outb(0x03C0, 0x41);
            outb(0x03C0, 0x33);
            outb(0x03C0, 0x00);


            //Setting the color pallete
            Self::load_u8_color_pallete();
            asm!("sti");
        }
        self.clear_buffer();
    }

    unsafe fn load_u8_color_pallete() {
        unsafe {
            //Unmask DAC palette
            outb(0x03C6, 0xFF);

            //Set the color start index to 0
            outb(0x03C8, 0x00);

            //First 16 EGA colors
            let ega_palette: [(u8, u8, u8); 16] = [
                (0x00, 0x00, 0x00), //0: black
                (0x00, 0x00, 0xAA), //1: blue
                (0x00, 0xAA, 0x00), //2: green
                (0x00, 0xAA, 0xAA), //3: cyan
                (0xAA, 0x00, 0x00), //4: red
                (0xAA, 0x00, 0xAA), //5: magenta
                (0xAA, 0x55, 0x00), //6: brown
                (0xAA, 0xAA, 0xAA), //7: light gray
                (0x55, 0x55, 0x55), //8: dark gray
                (0x55, 0x55, 0xFF), //9: light blue
                (0x55, 0xFF, 0x55), //10: light green
                (0x55, 0xFF, 0xFF), //11: light cyan
                (0xFF, 0x55, 0x55), //12: light red
                (0xFF, 0x55, 0xFF), //13: light magenta
                (0xFF, 0xFF, 0x55), //14: yellow
                (0xFF, 0xFF, 0xFF), //15: white
            ];

            //Write the ega colors into DAC
            for &(r, g, b) in ega_palette.iter() {
                outb(0x03C9, r / 4);
                outb(0x03C9, g / 4);
                outb(0x03C9, b / 4);
            }

            //6x6x6 RGB color cube, indices 16–231
            let steps = [0x00, 0x33, 0x66, 0x99, 0xCC, 0xFF];
            for r in &steps {
                for g in &steps {
                    for b in &steps {
                        outb(0x03C9, r / 4);
                        outb(0x03C9, g / 4);
                        outb(0x03C9, b / 4);
                    }
                }
            }

            //grayscale values, indices 232–255
            for i in 0..24 {
                let gray = (i * 255 / 23) as u8;
                outb(0x03C9, gray / 4);
                outb(0x03C9, gray / 4);
                outb(0x03C9, gray / 4);
            }
        }
    }
}



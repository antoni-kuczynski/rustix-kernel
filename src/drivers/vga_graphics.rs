#![allow(dead_code)]
/*
 * Created by Antek Kuczyński
 * 26/09/2025
 */
use core::arch::asm;
use core::ptr;

//  **VGA PORTS**
//  *CRT CONTROLLER*
const VGA_CRT_CONTROL_INDEX: u16 = 0x03D4;
//------------------------------------------
//  *ATTRIBUTE CONTROLLER*
const VGA_AC_INDEX: u16 = 0x3C0;
const VGA_AC_WRITE: u16 = 0x3C0;
const VGA_INSTAT_READ: u16 = 0x3DA;
const VGA_NUM_AC_REGS: usize = 21;
static AC_REGS: [u8; VGA_NUM_AC_REGS] = [
    0x00, 0x01, 0x02, 0x03,
    0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B,
    0x0C, 0x0D, 0x0E, 0x0F,
    0x41, 0x00, 0x0F, 0x00,
    0x00,
];
//---------------------------------------
//  *SEQUENCER*
const VGA_SEQUENCER_INDEX: u16 = 0x03C4;
//-----------------------------------------
//  *MISC OUTPUT REGISTER*
const VGA_MISC_OUTPUT_INDEX: u16 = 0x03C2;
//-------------------------------------------
//  *GRAPHICS CONTROLLER*
const VGA_GRAPHICS_CONTROLLER_INDEX: u16 = 0x03CE;
//---------------------------------------------------
//  **VIDEO BUFFER VARIABLES**
const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 200;
const BUFFER_LENGTH_BYTES: usize = VIDEO_WIDTH * VIDEO_HEIGHT;
//-------------------------------------------------------------
//  **FONTS**
//  *8px FONT*
const LOCHAR_8PX: usize = 32;
const HICHAR_8PX: usize = 127;
const BYTES_PER_CHAR_8PX: usize = 8;
const FONT_HEIGHT_8PX_PX: usize = 8;
const FONT_8PX: [u8; 760] = [
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x02,0x02,0x00,0x02,
    0x00,0x00,0x00,0x00,0x05,0x05,0x00,0x00,
    0x00,0x00,0x00,0x00,0x06,0x0F,0x0F,0x06,
    0x00,0x00,0x00,0x00,0x02,0x07,0x07,0x02,
    0x00,0x00,0x00,0x00,0x09,0x04,0x02,0x09,
    0x00,0x00,0x00,0x00,0x03,0x0D,0x07,0x0B,
    0x00,0x00,0x00,0x00,0x04,0x02,0x00,0x00,
    0x00,0x00,0x00,0x00,0x04,0x02,0x02,0x04,
    0x00,0x00,0x00,0x00,0x02,0x04,0x04,0x02,
    0x00,0x00,0x00,0x00,0x02,0x07,0x02,0x05,
    0x00,0x00,0x00,0x00,0x00,0x02,0x07,0x02,
    0x00,0x00,0x00,0x00,0x00,0x00,0x06,0x04,
    0x00,0x00,0x00,0x00,0x00,0x00,0x07,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x02,
    0x00,0x00,0x00,0x00,0x04,0x04,0x02,0x02,
    0x00,0x00,0x00,0x00,0x07,0x05,0x05,0x07,
    0x00,0x00,0x00,0x00,0x03,0x02,0x02,0x07,
    0x00,0x00,0x00,0x00,0x07,0x04,0x03,0x07,
    0x00,0x00,0x00,0x00,0x07,0x06,0x04,0x07,
    0x00,0x00,0x00,0x00,0x05,0x05,0x07,0x04,
    0x00,0x00,0x00,0x00,0x07,0x03,0x04,0x07,
    0x00,0x00,0x00,0x00,0x03,0x01,0x07,0x07,
    0x00,0x00,0x00,0x00,0x07,0x04,0x02,0x02,
    0x00,0x00,0x00,0x00,0x06,0x07,0x05,0x07,
    0x00,0x00,0x00,0x00,0x07,0x07,0x04,0x06,
    0x00,0x00,0x00,0x00,0x00,0x02,0x00,0x02,
    0x00,0x00,0x00,0x00,0x02,0x00,0x06,0x04,
    0x00,0x00,0x00,0x00,0x00,0x04,0x02,0x04,
    0x00,0x00,0x00,0x00,0x00,0x07,0x00,0x07,
    0x00,0x00,0x00,0x00,0x00,0x02,0x04,0x02,
    0x00,0x00,0x00,0x00,0x07,0x06,0x00,0x02,
    0x00,0x00,0x00,0x00,0x0F,0x09,0x0A,0x0F,
    0x00,0x00,0x00,0x00,0x06,0x05,0x07,0x05,
    0x00,0x00,0x00,0x00,0x03,0x07,0x05,0x07,
    0x00,0x00,0x00,0x00,0x06,0x01,0x01,0x07,
    0x00,0x00,0x00,0x00,0x03,0x05,0x05,0x03,
    0x00,0x00,0x00,0x00,0x07,0x03,0x01,0x07,
    0x00,0x00,0x00,0x00,0x07,0x03,0x01,0x01,
    0x00,0x00,0x00,0x00,0x06,0x01,0x05,0x07,
    0x00,0x00,0x00,0x00,0x05,0x05,0x07,0x05,
    0x00,0x00,0x00,0x00,0x07,0x02,0x02,0x07,
    0x00,0x00,0x00,0x00,0x06,0x04,0x05,0x07,
    0x00,0x00,0x00,0x00,0x05,0x03,0x03,0x05,
    0x00,0x00,0x00,0x00,0x01,0x01,0x01,0x07,
    0x00,0x00,0x00,0x00,0x07,0x07,0x07,0x05,
    0x00,0x00,0x00,0x00,0x07,0x05,0x05,0x05,
    0x00,0x00,0x00,0x00,0x07,0x05,0x05,0x07,
    0x00,0x00,0x00,0x00,0x07,0x05,0x07,0x01,
    0x00,0x00,0x00,0x00,0x07,0x05,0x07,0x0F,
    0x00,0x00,0x00,0x00,0x07,0x05,0x03,0x05,
    0x00,0x00,0x00,0x00,0x07,0x01,0x06,0x07,
    0x00,0x00,0x00,0x00,0x07,0x02,0x02,0x02,
    0x00,0x00,0x00,0x00,0x05,0x05,0x05,0x07,
    0x00,0x00,0x00,0x00,0x05,0x05,0x03,0x01,
    0x00,0x00,0x00,0x00,0x05,0x07,0x07,0x07,
    0x00,0x00,0x00,0x00,0x05,0x02,0x05,0x05,
    0x00,0x00,0x00,0x00,0x05,0x07,0x02,0x02,
    0x00,0x00,0x00,0x00,0x07,0x04,0x02,0x07,
    0x00,0x00,0x00,0x00,0x06,0x02,0x02,0x06,
    0x00,0x00,0x00,0x00,0x02,0x02,0x04,0x04,
    0x00,0x00,0x00,0x00,0x06,0x04,0x04,0x06,
    0x00,0x00,0x00,0x00,0x02,0x05,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x0F,0x00,0x00,
    0x00,0x00,0x01,0x02,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x06,0x07,0x07,0x00,0x00,
    0x00,0x00,0x01,0x07,0x05,0x07,0x00,0x00,
    0x00,0x00,0x00,0x06,0x01,0x07,0x00,0x00,
    0x00,0x00,0x04,0x07,0x05,0x07,0x00,0x00,
    0x00,0x00,0x00,0x07,0x07,0x03,0x00,0x00,
    0x00,0x00,0x06,0x02,0x07,0x02,0x00,0x00,
    0x00,0x00,0x06,0x07,0x04,0x03,0x00,0x00,
    0x00,0x00,0x01,0x07,0x05,0x05,0x00,0x00,
    0x00,0x00,0x02,0x00,0x02,0x02,0x00,0x00,
    0x00,0x00,0x02,0x00,0x02,0x03,0x00,0x00,
    0x00,0x00,0x01,0x05,0x03,0x05,0x00,0x00,
    0x00,0x00,0x03,0x02,0x02,0x06,0x00,0x00,
    0x00,0x00,0x00,0x07,0x07,0x05,0x00,0x00,
    0x00,0x00,0x00,0x07,0x05,0x05,0x00,0x00,
    0x00,0x00,0x00,0x07,0x05,0x07,0x00,0x00,
    0x00,0x00,0x02,0x05,0x03,0x01,0x00,0x00,
    0x00,0x00,0x02,0x05,0x06,0x04,0x00,0x00,
    0x00,0x00,0x00,0x07,0x01,0x01,0x00,0x00,
    0x00,0x00,0x00,0x06,0x02,0x03,0x00,0x00,
    0x00,0x00,0x02,0x06,0x02,0x06,0x00,0x00,
    0x00,0x00,0x00,0x05,0x05,0x07,0x00,0x00,
    0x00,0x00,0x00,0x05,0x07,0x02,0x00,0x00,
    0x00,0x00,0x00,0x05,0x07,0x07,0x00,0x00,
    0x00,0x00,0x00,0x05,0x02,0x05,0x00,0x00,
    0x00,0x00,0x00,0x05,0x06,0x03,0x00,0x00,
    0x00,0x00,0x00,0x03,0x02,0x06,0x00,0x00,
    0x00,0x00,0x06,0x03,0x02,0x06,0x00,0x00,
    0x00,0x00,0x02,0x02,0x02,0x02,0x00,0x00,
    0x00,0x00,0x03,0x06,0x02,0x03,0x00,0x00,
    0x00,0x00,0x04,0x07,0x01,0x00,0x00,0x00
];
//--------------------------------------------------
//  **UTIL FUNCTIONS**
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
//-------------------------------------

#[derive(Clone, Copy)]
pub struct VgaVideoColor(pub u8);

impl VgaVideoColor {
    pub const WHITE: Self = Self(0xFF);
    pub const BLACK: Self = Self(0x00);
    pub const RED: Self = Self(0b11100000);
    pub const GREEN: Self = Self(0b00011100);
    pub const BLUE: Self = Self(0b00000011);
    pub const YELLOW: Self = Self(0b11111100); //mix of green and red
    pub const CYAN: Self = Self(0b00011111); //mix of green and blue
    pub const MAGENTA: Self = Self(0b11100011); //mix of red and blue


    pub fn from_u24_rgb(r: u8, g: u8, b: u8) -> Self {
        //Returns "compressed" color from 24bit to 8bit
        /*
        7   6   5   4   3   2   1   0
        R   R   R   G   G   G   B   B
         */
        let r_dac =  (r / 32) << 5; //3 bytes
        let g_dac =  (g / 32) << 2; //3 bytes
        let b_dac = b / 64; //2 bytes
        let u8_value = r_dac + g_dac + b_dac;
        VgaVideoColor(u8_value)
    }

    pub fn from_u8(value: u8) -> Self {
        Self(value)
    }
}

pub struct Bitmap<const LENGTH: usize> {
    mem: [u8; LENGTH],
    width: usize,
    height: usize,
}

impl<const LENGTH: usize> Bitmap<LENGTH> {
    pub fn new(width: usize, height: usize, data: [u8; LENGTH]) -> Self {
        assert_eq!(LENGTH, width * height);
        Bitmap {
            mem: data,
            width,
            height,
        }
    }
}

//TODO: fix font struct awkwardness
pub struct Font<const MEM_LENGTH: usize> {
    mem: &'static [u8; MEM_LENGTH],
    lochar: usize,
    hichar: usize,
    bytes_per_char: usize,
    height: usize
}

const FONT_8PX_OBJ: Font<760> =
    Font::new(
        &FONT_8PX,
        LOCHAR_8PX,
        HICHAR_8PX,
        BYTES_PER_CHAR_8PX,
        FONT_HEIGHT_8PX_PX,
    );

impl<const MEM_LENGTH: usize> Font<MEM_LENGTH> {
    pub const fn new(mem: &'static [u8; MEM_LENGTH], lochar: usize, hichar: usize, width_bytes: usize, height: usize, ) -> Self {
        Self {
            mem,
            lochar,
            hichar,
            bytes_per_char: width_bytes,
            height,
        }
    }

    pub fn font_8px() -> Font<760> {
        FONT_8PX_OBJ
    }
}

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

    //TODO: draw the char but not reversed.....
    pub fn draw_char_transparent<const U8_PER_CHAR: usize>(&mut self, x: usize, y: usize, c: char, font: &Font<U8_PER_CHAR>, foreground: VgaVideoColor) {
        let mut char_index = c as usize;
        if(char_index < font.lochar || char_index > font.hichar) {
            char_index = '?' as usize; //unknown character
        }


        let mut source_char_byte: usize = (char_index - font.lochar) * font.bytes_per_char;
        let mut dest: usize = y * self.pitch + x;

        for l in 0..font.height {
            for i in 0..font.bytes_per_char {
                if font.mem[source_char_byte] & (0x80 >> i) != 0 {
                    self.video_buffer[dest] = foreground;
                }
                dest += 1;
            }
            dest += self.pitch - font.bytes_per_char;
            source_char_byte += 1;
        }
    }

    pub fn draw_string(&mut self, x: usize, y: usize, text: &str, font: &Font<760>, foreground: VgaVideoColor) {
        //asserts are already in draw_char_transparent
        for (i, c) in text.chars().enumerate() {
            self.draw_char_transparent(x + i * font.bytes_per_char, y, c, font, foreground);
        }
    }
    pub fn draw_bitmap<const LENGTH_BYTES: usize>(&mut self, x: usize, y: usize, bitmap: Bitmap<LENGTH_BYTES>) {
        assert!(x + bitmap.width < self.video_width_px);
        assert!(y + bitmap.height < self.video_height_px);

        let mut j = 0;
        for l in 0..bitmap.height {
            for i in 0..bitmap.width {
                self.put_pixel(x + i, y + l, VgaVideoColor(bitmap.mem[j]));
                j += 1;
            }
        }
    }

    pub fn draw_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, color: VgaVideoColor) {
        assert!(x0 > 0 && x0 < self.video_width_px as isize);
        assert!(y0 > 0 && y0 < self.video_height_px as isize);
        assert!(x1 > 0 && x1 < self.video_width_px as isize);
        assert!(y1 > 0 && y1 < self.video_height_px as isize);
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
        assert!(x + width < self.video_width_px);
        assert!(y + height < self.video_height_px);
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
            outb(VGA_MISC_OUTPUT_INDEX, 0x63);

            //CRT Control registers
            //Vertical Retrace End Register (index 0x11, set first to unlock registers 0x0 to 0x07)
            outw(VGA_CRT_CONTROL_INDEX, 0x0E11);

            //Horizontal total register (index 0x00)
            outw(VGA_CRT_CONTROL_INDEX, 0x5F00);

            //Horizontal Display-Enable End Register (index 0x01)
            outw(VGA_CRT_CONTROL_INDEX, 0x4F01);

            //Start Horizontal Blanking Register (index 0x02)
            outw(VGA_CRT_CONTROL_INDEX, 0x5002);

            //End Horizontal Blanking Register (index 0x03)
            outw(VGA_CRT_CONTROL_INDEX, 0x8203);

            //Start Horizontal Retrace Pulse Register (index 0x04)
            outw(VGA_CRT_CONTROL_INDEX, 0x5404);

            //End Horizontal Retrace Register (index 0x05)
            outw(VGA_CRT_CONTROL_INDEX, 0x8005);

            //Vertical Total Register (index 0x06)
            outw(VGA_CRT_CONTROL_INDEX, 0xBF06);

            //Overflow Register (0x07)
            outw(VGA_CRT_CONTROL_INDEX, 0x1F07);

            //Preset row scan register (index 0x08)
            outw(VGA_CRT_CONTROL_INDEX, 0x0008);

            //Maximum scan line regisyer (index 0x09)
            outw(VGA_CRT_CONTROL_INDEX, 0x4109);

            //Vertical Retrace Start Register (index 0x10)
            outw(VGA_CRT_CONTROL_INDEX, 0x9C10);

            //Vertical Display-Enable End Register (0x12)
            outw(VGA_CRT_CONTROL_INDEX, 0x8F12);

            //Offset Register (0x13)
            outw(VGA_CRT_CONTROL_INDEX, 0x2813);

            //Underline Location Register (index 0x14)
            outw(VGA_CRT_CONTROL_INDEX, 0x4014);

            //Start Vertical Blanking Register (0x15)
            outw(VGA_CRT_CONTROL_INDEX, 0x9615);

            //End Vertical Blanking Register (0x16)
            outw(VGA_CRT_CONTROL_INDEX, 0xB916);

            //CRT Mode Control Register (0x17)
            outw(VGA_CRT_CONTROL_INDEX, 0xA317);


            //Sequencer registers (0x03C4):
            outw(VGA_SEQUENCER_INDEX, 0x0100);

            //Clocking mode register (index 0x01)
            outw(VGA_SEQUENCER_INDEX, 0x0101);

            //Map mask register (index 0x02)
            outw(VGA_SEQUENCER_INDEX, 0x0F02);

            //register 0x03
            outw(VGA_SEQUENCER_INDEX, 0x0003);

            //Memory mode register (index 0x04)
            outw(VGA_SEQUENCER_INDEX, 0x0E04);

            //Graphics controller registers (0xCE):
            //Empty registers 0x00 to 0x04
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0000);
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0001);
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0002);
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0003);
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0004);

            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x4005);

            //Miscellaneous Register (index 0x06)
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0506);

            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0x0F07);
            outw(VGA_GRAPHICS_CONTROLLER_INDEX, 0xFF08);


            //Attribute controller registers:
            write_attribute_controller();

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


            for r in 0..8 {        //3 bits for red
                for g in 0..8 {    //3 bits for green
                    for b in 0..4 { //2 bits for blue
                        //Scale to 0..63 (DAC range)
                        let r6 = (r * 63 / 7) as u8;
                        let g6 = (g * 63 / 7) as u8;
                        let b6 = (b * 63 / 3) as u8;

                        outb(0x03C9, r6);
                        outb(0x03C9, g6);
                        outb(0x03C9, b6);
                    }
                }
            }
        }
    }
}

pub unsafe fn write_attribute_controller() {
    unsafe {
        //Write attribute controller registers
        for (i, &val) in AC_REGS.iter().enumerate() {
            let _ = inb(VGA_INSTAT_READ);   //reset flip-flop
            outb(VGA_AC_INDEX, i as u8);    //select register
            outb(VGA_AC_WRITE, val);        //write value
        }

        //Lock 16-color palette and unblank display
        let _ = inb(VGA_INSTAT_READ);
        outb(VGA_AC_INDEX, 0x20);
    }
}

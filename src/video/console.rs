#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 13/06/2026
 */
use core::ops::Deref;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::video::bitmap_font::{BitmapFont, BYTES_PER_CHAR_14PX, FONT_14PX, FONT_HEIGHT_14PX_PX, FONT_WIDTH_14PX_PX, HICHAR_14PX, LOCHAR_14PX};
use crate::video::framebuffer::{fb_plot_pixel, FramebufferColor, Framebuffer, FRAMEBUFFER};

const CURSOR_X_START: usize = 2;
const CURSOR_Y_START: usize = 2;
const CURSOR_X_END_OFFSET: usize = 2;
const CURSOR_Y_END_OFFSET: usize = 2;
const ROW_SPACING: usize = 2;
const CHAR_OVERLAP: usize = 5;
const TAB_SPACES: usize = 4;

impl Framebuffer {
    pub fn init_text_cursor(&mut self) {
        let font = BitmapFont::new(
            &FONT_14PX,
            FONT_WIDTH_14PX_PX,
            FONT_HEIGHT_14PX_PX,
            LOCHAR_14PX,
            HICHAR_14PX,
            BYTES_PER_CHAR_14PX
        );
        self.font = font;
        self.cursor_pos_x_px = CURSOR_X_START;
        self.cursor_pos_y_px = CURSOR_Y_START;
    }

    pub fn putchar(
        &mut self,
        x: usize,
        y: usize,
        foreground: &FramebufferColor,
        background: &FramebufferColor,
        c: char,
        transparent: bool,
    ) {
        if x >= self._pixel_info.width || y >= self._pixel_info.height {
            return;
        }


        let mut char_index = c as usize;
        if char_index < self.font.lochar || char_index > self.font.hichar {
            char_index = '?' as usize;
        }

        let bpp = self.bpp() as usize >> 3;
        let pitch = self.pitch();

        let bytes_per_row = (self.font.width + 7) >> 3;
        let mut source_char_byte = (char_index - self.font.lochar) * self.font.bytes_per_char;

        for row in 0..self.font.height {
            let dest_y = y as isize + row as isize;

            if dest_y < 0 || dest_y >= self._pixel_info.height as isize {
                source_char_byte += bytes_per_row;
                continue;
            }

            let row_ptr = unsafe { self.base.add((dest_y as usize) * pitch) };
            let mut dest_x = x as isize;

            for col in 0..self.font.width {
                if dest_x >= 0 && dest_x < self._pixel_info.width as isize {
                    let reversed_col = self.font.width - 1 - col;

                    let byte_idx = reversed_col >> 3;
                    let bit_idx = reversed_col & 7;

                    let byte = self.font.data[source_char_byte + byte_idx];

                    let is_set = (byte << bit_idx) & 0x80 != 0;

                    unsafe {
                        let pixel_ptr = row_ptr.add((dest_x as usize) * bpp);

                        let base_offset = (dest_y as usize * pitch) + (dest_x as usize* bpp);

                        if is_set {
                            self.write_raw_pixel(base_offset, foreground.data, bpp);
                        } else if !transparent {
                            self.write_raw_pixel(base_offset, background.data, bpp);
                        }
                    }
                }
                dest_x += 1;
            }

            source_char_byte += bytes_per_row;
        }
    }

    #[inline(always)]
    unsafe fn write_raw_pixel(&mut self, base_offset: usize, color_data: u32, bpp: usize) {
        //TODO: color bpp matching
        self.fb_write(base_offset, color_data as u8);
        self.fb_write(base_offset + 1, (color_data >> 8) as u8);
        self.fb_write(base_offset + 2, (color_data >> 16) as u8);
    }

    fn cursor_new_row(&mut self) {
        self.cursor_pos_x_px = CURSOR_X_START;
        self.cursor_pos_y_px += self.font.height + ROW_SPACING;
        if self.cursor_pos_y_px + self.font.height > self._pixel_info.height {
            self.scroll_up_by_one_row();
        }
    }

    fn cursor_push_by(&mut self, amount_of_chars: usize) {
        let future_cx = self.cursor_pos_x_px + (self.font.width - CHAR_OVERLAP) * amount_of_chars;
        if future_cx + self.font.width >= self._pixel_info.width {
            self.cursor_new_row();
            return;
        }

        self.cursor_pos_x_px = future_cx;
    }

    fn scroll_up_by_one_row(&mut self) {
        let scroll_amount = self.font.height + ROW_SPACING;
        let offset_bytes = scroll_amount * self.pitch();
        let total_bytes = self._pixel_info.height * self.pitch();

        if offset_bytes >= total_bytes {
            self.cursor_pos_y_px = 0;
            return;
        }

        let bytes_to_move = total_bytes - offset_bytes;

        //move memory upwards
        self.back_buffer.copy_within(offset_bytes..total_bytes, 0);

        //clear new bottom row
        self.back_buffer[bytes_to_move..total_bytes].fill(0);

        self.cursor_pos_x_px = CURSOR_X_START;

        if self.cursor_pos_y_px >= scroll_amount {
            self.cursor_pos_y_px -= scroll_amount;
        } else {
            self.cursor_pos_y_px = 0;
        }
    }

    pub fn put_string_at_cursor(&mut self, string: &str, foreground: &FramebufferColor, background: Option<&FramebufferColor>) {
        for c in string.chars() {
            self.put_char_at_cursor(c, foreground, background);
        }
    }

    pub fn put_char_at_cursor(&mut self, char: char, foreground: &FramebufferColor, background: Option<&FramebufferColor>) {
        let font_width = self.font.width;
        let font_height = self.font.height;

        //TODO: other control codes
        if char == '\n' {
            self.cursor_new_row();
            return;
        } else if char == '\t' {
            self.cursor_push_by(TAB_SPACES);
            return;
        }

        if let Some(bg) = background {
            self.putchar(
                self.cursor_pos_x_px,
                self.cursor_pos_y_px,
                foreground,
                bg,
                char,
                false
            );
        } else {
            self.putchar(
                self.cursor_pos_x_px,
                self.cursor_pos_y_px,
                foreground,
                &FramebufferColor::from_rgb(0,0,0), //unused
                char,
                true
            );
        }

        self.cursor_push_by(1);
    }
}

pub fn put_string(s: &str, foreground: &FramebufferColor, background: &FramebufferColor) {
    FRAMEBUFFER.lock().as_mut().unwrap().put_string_at_cursor(s, foreground, Some(background));
}

pub fn put_string_no_bg(s: &str, foreground: &FramebufferColor) {
    FRAMEBUFFER.lock().as_mut().unwrap().put_string_at_cursor(s, foreground, None);
}

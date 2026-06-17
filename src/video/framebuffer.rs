#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 14/06/2026
 */
use alloc::vec::Vec;
use core::{ptr};
use spin::{Mutex, Once};
use x86_64::{PhysAddr};
use crate::boot::multiboot::multiboot2_get_framebuffer_tag;
use crate::boot::multiboot_tag::{MultibootFramebufferColorInfo, MultibootFramebufferInfoTag, MultibootFramebufferRgbInfo};
use crate::memory::dir_mapping::physical_to_virtual;
use crate::video::bitmap_font::{BitmapFont};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferKind {
    Rgb,
    Indexed,
    EgaText,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBufferColorInfo {
    pub red_pos: u8,
    pub red_size: u8,
    pub red_mask: u32,

    pub green_pos: u8,
    pub green_size: u8,
    pub green_mask: u32,

    pub blue_pos: u8,
    pub blue_size: u8,
    pub blue_mask: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct FramebufferPixelInfo {
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
    pub bpp: u8,
}

pub struct Framebuffer {
    pub base: *mut u8,
    pub kind: FramebufferKind,

    /// Use fb_pixel_info() instead
    pub _pixel_info: FramebufferPixelInfo,

    /// Use fb_color_info() instead
    pub _color_info: FrameBufferColorInfo,

    pub cursor_pos_x_px: usize,
    pub cursor_pos_y_px: usize,
    pub font: BitmapFont,

    pub back_buffer: Vec<u8>
}

impl Framebuffer {
    pub unsafe fn new_rgb(fb_tag: &MultibootFramebufferInfoTag, rgb_info: &MultibootFramebufferRgbInfo, virt_addr: *mut u8) -> Self {
        Self {
            base: virt_addr,
            _pixel_info: FramebufferPixelInfo::new(
                fb_tag.framebuffer_pitch,
                fb_tag.framebuffer_width,
                fb_tag.framebuffer_height,
                fb_tag.framebuffer_bpp
            ),
            kind: FramebufferKind::Rgb,
            _color_info: FrameBufferColorInfo::new(rgb_info),
            cursor_pos_x_px: 0,
            cursor_pos_y_px: 0,
            font: BitmapFont::null_font(),
            back_buffer: Vec::new(),
        }
    }

    pub unsafe fn from_multiboot_tag(tag: &MultibootFramebufferInfoTag) -> Self {
        //TODO: temp
        let framebuffer_virt = physical_to_virtual(PhysAddr::new(tag.framebuffer_addr))
            .as_mut_ptr::<u8>();

        //TODO: a method to tell the framebuffer is invalid
        let info = match tag.color_info() {
            MultibootFramebufferColorInfo::Rgb { info } => info,
            MultibootFramebufferColorInfo::Indexed { .. } => {
                panic!("indexed framebuffer is not supported yet")
            }
            MultibootFramebufferColorInfo::EgaText => {
                panic!("EGA text framebuffer is not supported")
            }
            MultibootFramebufferColorInfo::Unknown(t) => {
                panic!("unknown framebuffer type: {}", t)
            }
        };

        let mut fb = Self::new_rgb(tag, &info, framebuffer_virt);
        fb.init_text_cursor();
        fb
    }

    //TODO: optimize
    pub fn plot_pixel(&mut self, x: usize, y: usize, color: &FramebufferColor) {
        unsafe {
            let bits_per_px = self.bpp() as usize;

            if bits_per_px == 0 || bits_per_px > 32 {
                return;
            }

            let row_bit_offset = x * bits_per_px;

            let mut byte_offset = y * self.pitch() + (row_bit_offset >> 3);
            let mut bit_in_byte = row_bit_offset & 7;

            let mut remaining_bits = bits_per_px;
            let mut color_shift = 0usize;

            let color_data = if bits_per_px == 32 {
                color.data
            } else {
                color.data & ((1u32 << bits_per_px) - 1)
            };

            while remaining_bits > 0 {
                let bits_available_in_byte = 8 - bit_in_byte;
                let bits_to_write = core::cmp::min(remaining_bits, bits_available_in_byte);

                let mask_part = ((1u16 << bits_to_write) - 1) as u8;
                let mask = mask_part << bit_in_byte;

                let color_part = ((color_data >> color_shift) as u8) << bit_in_byte;
                let color_part = color_part & mask;

                let ptr = self.base.add(byte_offset);

                let old = ptr::read_volatile(ptr);
                let new = (old & !mask) | color_part;

                self.fb_write(byte_offset, new);

                remaining_bits -= bits_to_write;
                color_shift += bits_to_write;
                byte_offset += 1;
                bit_in_byte = 0;
            }
        }
    }

    pub fn fb_write(&mut self, base_offset: usize, val: u8) {
        let is_double_buffered = !self.back_buffer.is_empty();

        if is_double_buffered {
            self.back_buffer[base_offset] = val;
        } else {
            unsafe { ptr::write_volatile(self.base.add(base_offset), val) };
        }
    }

    pub fn swap_buffers(&mut self) {
        let total_bytes = self._pixel_info.height * self.pitch();

        unsafe {
            ptr::copy_nonoverlapping(
                self.back_buffer.as_ptr(),
                self.base,
                total_bytes
            );
        }
    }

    fn width(&self) -> usize {
        self._pixel_info.width
    }

    fn height(&self) -> usize {
        self._pixel_info.height
    }

    pub(crate) fn pitch(&self) -> usize {
        self._pixel_info.pitch
    }

    pub(crate) fn bpp(&self) -> u8 {
        self._pixel_info.bpp
    }
}

impl FrameBufferColorInfo {
    fn new(rgb_info: &MultibootFramebufferRgbInfo) -> FrameBufferColorInfo {
        FrameBufferColorInfo {
            red_pos: rgb_info.red_pos,
            red_size: rgb_info.red_mask_size,
            red_mask: Self::get_color_mask(rgb_info.red_pos, rgb_info.red_mask_size),
            green_pos: rgb_info.green_pos,
            green_size: rgb_info.green_mask_size,
            green_mask: Self::get_color_mask(rgb_info.green_pos, rgb_info.green_mask_size),
            blue_pos: rgb_info.blue_pos,
            blue_size: rgb_info.blue_mask_size,
            blue_mask: Self::get_color_mask(rgb_info.blue_pos, rgb_info.blue_mask_size),
        }
    }

    fn get_color_mask(pos: u8, size: u8) -> u32 {
        if size == 0 {
            0
        } else if size >= 32 {
            u32::MAX
        } else {
            ((1u32 << size) - 1) << pos
        }
    }
}

impl FramebufferPixelInfo {
    fn new(pitch: u32, width: u32, height: u32, bpp: u8) -> FramebufferPixelInfo {
        Self {
            pitch: pitch as usize,
            width: width as usize,
            height: height as usize,
            bpp,
        }
    }
}

//==================================================================================================
//==================================================================================================
pub struct FramebufferColor {
    pub data: u32 // default is 1 byte per each RGB color, 4byte is unused
}

impl FramebufferColor {
    pub fn new_raw(val: u32) -> FramebufferColor {
        FramebufferColor {
            data: val
        }
    }

    /// Constructs a 24bit color, based on the framebuffer's parameters
    pub fn from_rgb(r: u32, g: u32, b: u32) -> FramebufferColor {
        let fb = fb_color_info();

        let red = (r << fb.red_pos) & fb.red_mask;
        let green = (g << fb.green_pos) & fb.green_mask;
        let blue = (b << fb.blue_pos) & fb.blue_mask;

        Self::new_raw(red | green | blue)
    }
}
//==================================================================================================
unsafe impl Send for Framebuffer {}

pub fn framebuffer_init() {
    let fb_tag = multiboot2_get_framebuffer_tag()
        .expect("framebuffer tag not found");

    let framebuffer_view = unsafe { Framebuffer::from_multiboot_tag(fb_tag) };

    if framebuffer_view.base as u64 == 0 {
        panic!("Framebuffer's ptr is null!");
    }

    let mut fb = FRAMEBUFFER.lock();
    let color_info = framebuffer_view._color_info;
    let pixel_info = framebuffer_view._pixel_info;

    *fb = Some(framebuffer_view);
    FRAMEBUFFER_COLOR_INFO.call_once(|| color_info);
    FRAMEBUFFER_PIXEL_INFO.call_once(|| pixel_info);
}

pub fn double_buffering_init() {
    let mut guard = FRAMEBUFFER.lock();
    let fb = guard.as_mut().unwrap();
    fb.back_buffer.reserve(fb_height() * fb_pitch());

    for i in 0..fb_height() * fb_pitch() {
        fb.back_buffer.push(0x0u8);
    }
}

pub fn fb_plot_pixel(x: usize, y: usize, color: &FramebufferColor) {
    FRAMEBUFFER
        .lock()
        .as_mut()
        .unwrap()
        .plot_pixel(x, y, color);
}

pub fn fb_swap_buffers() {
    FRAMEBUFFER
        .lock()
        .as_mut()
        .unwrap()
        .swap_buffers();
}

pub fn fb_color_info() -> &'static FrameBufferColorInfo {
    match FRAMEBUFFER_COLOR_INFO.get() {
        None => { panic!("Framebuffer color info not initialized!") }
        Some(x) => { x }
    }
}

pub fn fb_pixel_info() -> &'static FramebufferPixelInfo {
    match FRAMEBUFFER_PIXEL_INFO.get() {
        None => { panic!("Framebuffer pixel info not initialized!") }
        Some(x) => { x }
    }
}

pub fn fb_width() -> usize {
    fb_pixel_info().width
}

pub fn fb_height() -> usize {
    fb_pixel_info().height
}

pub fn fb_pitch() -> usize {
    fb_pixel_info().pitch
}

pub fn fb_bpp() -> u8 {
    fb_pixel_info().bpp
}

pub static FRAMEBUFFER: Mutex<Option<Framebuffer>> = Mutex::new(None);
pub static FRAMEBUFFER_COLOR_INFO: Once<FrameBufferColorInfo> = Once::new();
pub static FRAMEBUFFER_PIXEL_INFO: Once<FramebufferPixelInfo> = Once::new();
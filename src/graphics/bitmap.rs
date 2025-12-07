/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */
use alloc::vec::Vec;

#[allow(dead_code)]
pub struct Bitmap {
    pub mem: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pixel_width: usize
}

impl Bitmap {
    fn new(width: usize, height: usize, pixel_width: usize, data: Vec<u8>) -> Option<Self> {
        if data.len() * pixel_width != width * height {
            return None;
        }
        
        let tmp = Bitmap {
            mem: data,
            width,
            height,
            pixel_width
        };
        Some(tmp)
    }

    pub fn new_u8_bitmap(width: usize, height: usize, data: Vec<u8>) -> Option<Self> {
        Self::new(width, height, 1, data)
    }
}
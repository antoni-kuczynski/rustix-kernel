/*
 * Created by Antek Kuczyński
 * 12/10/2025
 */

pub struct Bitmap<const LENGTH: usize> {
    pub mem: [u8; LENGTH],
    pub width: usize,
    pub height: usize,
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
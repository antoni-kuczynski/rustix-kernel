use alloc::boxed::Box;
use lazy_static::lazy_static;
use spin::Mutex;

mod registers;
pub mod vga_fonts;
pub mod vga_graphics;
pub mod vga_text;

// ============================================================
//                     **CURRENT VGA OPERATION MODE**
// ============================================================
pub struct CurrentVgaMode {
    val: Box<Option<u8>>,
}
impl CurrentVgaMode {
    fn new() -> Self {
        CurrentVgaMode {
            val: Box::new(None),
        }
    }

    pub fn switch_to(&mut self, val: u8) {
        *self.val = Some(val);
    }

    pub fn get(&mut self) -> Option<u8> {
        *self.val
    }
}

lazy_static! {
    pub static ref CURRENT_VGA_MODE: Mutex<CurrentVgaMode> = Mutex::new(CurrentVgaMode::new());
}

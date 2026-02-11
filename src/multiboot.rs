use core::arch::asm;
use core::ptr;
use crate::{panic, print_ok_msg, vgaprintln};

//==============================================
//Multiboot information structures
#[repr(C, packed)]
pub struct MultibootTag {
    tag_type: u32,
    size: u32
}
//==============================================
#[repr(C, packed)]
pub struct MultibootInfo {
    total_size: u32,
    reserved: u32
}
//==============================================
pub struct MultibootInfoView {
    base: &'static MultibootInfo,
    tags_size_bytes: usize,
    tags: *const u32
}
//==============================================
impl MultibootInfoView {
    pub fn new(addr: u32) -> MultibootInfoView {
        unsafe {
            let base = MultibootInfo::new(addr);
            let tags_size_bytes = base.total_size as usize - (2 * size_of::<u32>());
            let addr = base as *const MultibootInfo as *const u32;
            let tags = addr.add(2);

            let view = Self {
                base,
                tags_size_bytes,
                tags
            };

            view
        }
    }

    pub fn print(&self) {
        unsafe {
            let total_size = self.base.total_size;
            let mut tags = self.tags as *const u8;
            let tags_end = tags.add(self.tags_size_bytes);

            vgaprintln!("Multiboot info structure:");
            vgaprintln!("===================================");
            vgaprintln!("Total size: {}", total_size);

            while tags < tags_end {
                let val = ptr::read_volatile(tags);
                vgaprintln!("{:#02x}", val);

                tags = tags.add(1);
            }
            vgaprintln!("end");
        }
    }

    pub fn base(&self) -> &'static MultibootInfo {
        self.base
    }

    pub fn tags_size_bytes(&self) -> usize {
        self.tags_size_bytes
    }

    pub fn tags(&self) -> *const u32 {
        self.tags
    }
}

impl MultibootInfo {
    fn new(addr: u32) -> &'static Self {
        unsafe {
            vgaprintln!("Reading multiboot info struct (addr={:#06x})...", addr);

            let ptr = addr as usize as *const MultibootInfo;

            match ptr.as_ref() {
                Some(x) => x,
                None => panic!("Could not reference MultibootInfo struct!")
            }
        }
    }

    pub fn get_multiboot_address_from_ebx() -> u32 {
        unsafe {
            let addr: u32;
            asm!(
                "mov {0:e}, ebx",
                out(reg) addr,
            );
            // vgaprintln!("{:#06}", addr);
            addr
        }
    }

}


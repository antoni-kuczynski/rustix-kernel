use core::arch::asm;

pub mod paging;
pub mod pmm;

pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}


struct Cr3();

impl Cr3 {
    pub fn cr3_read() -> u64 {
        unsafe {
            let val: u64;
            asm!(
                "mov {}, cr3",
                out(reg) val,
            );
            val
        }
    }

    pub fn cr3_page_table_base() -> PhysicalAddress {
        PhysicalAddress::new(Cr3::cr3_read() & 0x000FFFFFFFFFF000)
    }
}

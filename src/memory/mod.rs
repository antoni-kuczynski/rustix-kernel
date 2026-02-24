use core::arch::asm;
use crate::endKernel;

pub mod paging;
pub mod pmm;

//==================================================================
pub const PHYS_BASE: u32 = 0x00100000;
pub const VIRT_BASE: u64 = 0xFFFFFFFF80000000;

pub fn V2P(virt_address: u64) -> u64 {
    virt_address - VIRT_BASE
}

pub fn P2V(phys_address: u64) -> u64 {
    phys_address + VIRT_BASE
}

pub fn kernel_end() -> u64 {
    unsafe {&endKernel as *const u32 as u64}
}
//==================================================================


pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}


pub struct Cr3();

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

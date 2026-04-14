#![allow(unused)]
#![allow(non_snake_case)]
use core::arch::asm;
use x86_64::{PhysAddr, VirtAddr};
use crate::endKernel;

pub mod paging;
pub mod pmm;
pub mod eba;
pub mod page_tables;

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
pub struct MemoryRange {
    pub start: u64,
    pub end: u64
}

impl MemoryRange {
    pub fn new(start: u64, end: u64) -> Self {
        Self {
            start, end
        }
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

    pub fn cr3_page_table_base() -> PhysAddr {
        PhysAddr::new(Cr3::cr3_read() & 0x000FFFFFFFFFF000)
    }
}

unsafe fn flush_tlb_single_page(virtual_address: VirtAddr) {
    unsafe {
        asm!("invlpg [{}]", in(reg) virtual_address.as_u64(), options(nostack, preserves_flags));
    }
}

pub enum SizeUnit {
    Byte = 1,
    Kilobyte = 1024,
    Megabyte = 1_048_576,
    Gigabyte = 1_073_741_824
}

impl SizeUnit {
    pub fn as_usize(&self) -> usize {
        match self {
            SizeUnit::Byte => {1}
            SizeUnit::Kilobyte => {1024}
            SizeUnit::Megabyte => {1_048_576}
            SizeUnit::Gigabyte => {1_073_741_824}
        }
    }
    
    pub fn as_u64(&self) -> u64 {
        self.as_usize() as u64 //lol
    }
}

pub const FRAME_SIZE: u64 = 4096;

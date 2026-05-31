#![allow(unused)]
#![allow(non_snake_case)]
use crate::endKernel;
use core::arch::asm;
use core::ops::{Div, Mul};
use x86_64::{PhysAddr, VirtAddr};

pub mod dir_mapping;
pub mod dma;
pub mod eba;
pub mod kheap;
pub mod kheap_test;
mod ll_allocator;
pub mod page_tables;
pub mod paging;
pub mod pmm;
pub mod secure_stack;
pub mod ioremap;

//==================================================================
pub const KERNEL_PHYS_BASE: u32 = 0x00100000;
pub const KERNEL_VIRT_BASE: u64 = 0xFFFFFFFF80000000;

pub fn _V2P_kernel(virt_address: u64) -> u64 {
    virt_address - KERNEL_VIRT_BASE
}

pub fn _P2V_kernel(phys_address: u64) -> u64 {
    phys_address + KERNEL_VIRT_BASE
}

pub fn kernel_end() -> u64 {
    unsafe { &endKernel as *const u32 as u64 }
}
//==================================================================
pub struct MemoryRange {
    pub start: u64,
    pub end: u64,
}

impl MemoryRange {
    pub fn new(start: u64, end: u64) -> Self {
        Self { start, end }
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
    Gigabyte = 1_073_741_824,
    Terabyte = 1_099_511_627_776,
}

impl SizeUnit {
    pub fn as_usize(&self) -> usize {
        match self {
            SizeUnit::Byte => 1,
            SizeUnit::Kilobyte => 1024,
            SizeUnit::Megabyte => 1_048_576,
            SizeUnit::Gigabyte => 1_073_741_824,
            SizeUnit::Terabyte => 1_099_511_627_776,
        }
    }

    pub fn as_u64(&self) -> u64 {
        self.as_usize() as u64 //lol
    }
}

impl Mul<i32> for SizeUnit {
    type Output = u64;

    fn mul(self, rhs: i32) -> Self::Output {
        self.as_u64() * rhs as u64
    }
}

impl Div<i32> for SizeUnit {
    type Output = u64;

    fn div(self, rhs: i32) -> Self::Output {
        self.as_u64() / rhs as u64
    }
}

impl Div<i32> for &SizeUnit {
    type Output = u64;

    fn div(self, rhs: i32) -> Self::Output {
        self.as_u64() / rhs as u64
    }
}

pub const FRAME_SIZE: u64 = 4096;

/// Align the given address `addr` upwards to alignment `align`.
pub fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use crate::ColorTextMode;
use crate::memory::ll_allocator::LinkedListAllocator;
use crate::memory::page_tables::PageSize;
use crate::memory::paging::{vmm_eba_map_page, vmm_map_page};
use crate::memory::pmm::{pmm_allocate_contiguous, pmm_allocate_frame};
use crate::memory::{FRAME_SIZE, SizeUnit};
use crate::{VGAWRITER, vgaprint};
use crate::{print_ok_msg, vgaprintln};
use core::alloc::{GlobalAlloc, Layout};
use core::ops::{Div, Mul};
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

const KHEAP_START: u64 = 0xffff_c200_0000_0000;
const KHEAP_LENGTH: u64 = 16 * 1_099_511_627_776; // 16tb
pub const KHEAP_END: u64 = KHEAP_START + KHEAP_LENGTH;

pub struct Locked<A> {
    inner: Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }
}

#[global_allocator]
pub static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub fn kheap_init() {
    vgaprint!("Initializing kernel heap...");

    unsafe {
        ALLOCATOR
            .lock()
            .init(KHEAP_START as usize, KHEAP_LENGTH as usize);
    }

    print_ok_msg!();
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().deallocate(ptr, layout)
    }
}

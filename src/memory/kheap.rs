#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use x86_64::{PhysAddr, VirtAddr};
use crate::{vgaprint, VGAWRITER};
use crate::ColorTextMode;
use crate::{print_ok_msg, vgaprintln};
use crate::memory::paging::{vmm_eba_map_page, vmm_map_page};
use crate::memory::ll_allocator::LinkedListAllocator;
use core::alloc::{GlobalAlloc, Layout};
use core::ops::{Div, Mul};
use spin::Mutex;
use crate::memory::page_tables::PageSize;
use crate::memory::pmm::pmm_allocate_frame;
use crate::memory::{SizeUnit, FRAME_SIZE};

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

    let heap_start = KHEAP_START;
    let heap_size = SizeUnit::Megabyte.as_u64() * 2; //2mb initial size

    let phys = pmm_allocate_frame().expect("failed to allocate frame for heap");
    unsafe {
        vmm_eba_map_page(
            VirtAddr::new(heap_start),
            phys,
            &PageSize::Size2Mb,
            true
        );
    }

    unsafe {
        ALLOCATOR.lock().init(heap_start as usize, heap_size as usize);
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


fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

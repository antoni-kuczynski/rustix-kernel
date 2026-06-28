#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
use core::sync::atomic::Ordering::Acquire;
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
//==================================================================================================
// This is a tem heap region to store early page tables (Early bump allocator)
// It's been already mapped during early init, so dont care about that
//==================================================================================================
use crate::memory::page_tables::PagingSetupError;
use crate::memory::{_P2V_kernel, MemoryRange};
use crate::{__kprintln_ok_buf, earlyHeapEnd, earlyHeapStart, memory};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PageTable;
//==================================================================================================
pub struct EarlyBumpAllocator {
    temp_range: MemoryRange,
    temp_ptr: AtomicPtr<u8>,
}
//==================================================================================================
impl EarlyBumpAllocator {
    fn empty() -> Self {
        Self {
            temp_range: MemoryRange { start: 0, end: 0 },
            temp_ptr: Default::default(),
        }
    }

    unsafe fn kmalloc_early<T>(&self, size: usize, align: usize) -> Option<*mut T> {
        let align_u64 = if align == 0 { 1 } else { align as u64 };
        let mut current_ptr = self.temp_ptr.load(Acquire);

        loop {
            let aligned_ptr =
                ((current_ptr.add((align_u64 - 1) as usize)) as u64 & !(align_u64 - 1)) as *mut u8;
            let next_ptr = aligned_ptr.add(size);

            if next_ptr as u64 > self.temp_range.end {
                return None;
            }

            match self.temp_ptr.compare_exchange_weak(
                current_ptr,
                next_ptr,
                Ordering::SeqCst,
                Acquire,
            ) {
                Ok(_) => {
                    return Some(aligned_ptr as *mut T);
                }
                Err(actual_ptr) => {
                    current_ptr = actual_ptr;
                }
            }
        }
    }
}
//==================================================================================================
pub fn eba_init() {
    let start = _P2V_kernel(unsafe { earlyHeapStart });
    let end = _P2V_kernel(unsafe { earlyHeapEnd });

    let mut eba = EARLY_BUMP_ALLOCATOR.lock();
    eba.temp_range = MemoryRange::new(start, end);
    eba.temp_ptr = AtomicPtr::new(start as *mut u8);
    __kprintln_ok_buf!("Initialized early bump allocator.");
}

lazy_static! {
    pub static ref EARLY_BUMP_ALLOCATOR: Mutex<EarlyBumpAllocator> =
        Mutex::new(EarlyBumpAllocator::empty());
}

/// Early kmalloc for early bump allocator region
pub unsafe fn eba_kmalloc<T>(size: usize, align: usize) -> Option<*mut T> {
    unsafe { EARLY_BUMP_ALLOCATOR.lock().kmalloc_early(size, align) }
}

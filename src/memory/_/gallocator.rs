#![allow(static_mut_refs)]
/*
 * Made by Oskar Przybylski
 * 02/10/2025
 * */

use linked_list_allocator::LockedHeap;
use x86_64::{structures::paging::{mapper::{self, MapToError}, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB}, VirtAddr};

pub const GLOBAL_HEAP_START: usize = 0x_4444_4444_0000;
pub const GLOBAL_HEAP_SIZE : usize = 100 * 1024; // 100 KiB
static mut HEAP_ALLOC: [u8;GLOBAL_HEAP_SIZE] = [0;GLOBAL_HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init(
    mapper: &mut impl Mapper<Size4KiB>, 
    frame_allocator: &mut impl FrameAllocator<Size4KiB>
    ) -> Result<(), MapToError<Size4KiB>> {

    unsafe {
        ALLOCATOR.lock().init(HEAP_ALLOC.as_mut_ptr(),GLOBAL_HEAP_SIZE);
    }

    let page_range = {
        let heap_start = VirtAddr::new(GLOBAL_HEAP_START as u64);
        let heap_end   = heap_start + GLOBAL_HEAP_SIZE as u64 - 1u64;
        let heap_start_page : Page<Size4KiB> = Page::containing_address(heap_start);
        let heap_end_page   : Page<Size4KiB> = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    for page in page_range {
        let frame = frame_allocator.allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            let _ = mapper.map_to(page, frame, flags, frame_allocator)?;
        }
    }

    Ok(())
}



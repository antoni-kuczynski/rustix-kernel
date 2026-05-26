#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use core::alloc::Layout;
use core::cmp::PartialEq;
use core::{mem, ptr};
use core::ptr::null_mut;
use x86_64::VirtAddr;
use crate::memory::{SizeUnit, FRAME_SIZE};
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::memory::paging::{vmm_map_page, vmm_map_page_ext, vmm_map_range_ext, vmm_unmap_page};
use crate::memory::pmm::{pmm_allocate_frame, pmm_allocate_contiguous, pmm_free_frame};
use crate::vgaprintln;

pub struct ListNode {
    pub(crate) size: usize,
    pub(crate) next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    pub(crate) fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    pub(crate) fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

impl PartialEq for ListNode {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

pub struct LinkedListAllocator {
    pub(crate) head: ListNode,
    pub(crate) heap_end: usize,     //4kb aligned current end of heap

    //the top region is the last region if its start+size is equal to heap_end, otherwise its 0,0
    pub(crate) top_start: usize,
    pub(crate) top_size: usize,

    //the whole region allocator works in
    pub(crate) region_start: usize,
    pub(crate) region_length: usize,

    pub flags: u64,
    pub is_contiguous: bool,
    pub allow_growth: bool,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
            heap_end: 0,
            top_start: 0,
            top_size: 0,
            region_start: 0,
            region_length: 0,
            flags: PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
            is_contiguous: false,
            allow_growth: true,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
        self.heap_end = heap_start + heap_size;
        self.region_start = heap_start;
        self.region_length = heap_size;
        self.top_start = heap_start;
        self.top_size = heap_size;
    }

    pub fn set_flags(&mut self, flags: u64) {
        self.flags = flags;
    }

    pub fn set_contiguous(&mut self, contiguous: bool) {
        self.is_contiguous = contiguous;
    }

    pub fn set_allow_growth(&mut self, allow_growth: bool) {
        self.allow_growth = allow_growth;
    }

    fn update_top_if_needed(&mut self, region_start: usize, region_size: usize) {
        let region_end = region_start + region_size;

        if region_end == self.heap_end {
            self.top_start = region_start;
            self.top_size = region_size;
        }
    }

    fn clear_top(&mut self) {
        self.top_start = 0;
        self.top_size = 0;
    }

    fn top_end(&self) -> usize {
        self.top_start + self.top_size
    }

    fn region_contains_top(&self, region_start: usize, region_size: usize) -> bool {
        if self.top_size == 0 {
            return false;
        }

        let region_end = region_start + region_size;
        let top_end = self.top_end();

        region_start <= self.top_start && region_end >= top_end
    }

    /// Adds a free region with the given address and size to the front of the list
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        let aligned_addr = align_up(addr, align_of::<ListNode>());
        let padding = aligned_addr - addr;

        let Some(size) = size.checked_sub(padding) else {
            return;
        };

        let size = size & !(align_of::<ListNode>() - 1);

        if size < size_of::<ListNode>() {
            return;
        }

        let addr = aligned_addr;
        assert!(size >= size_of::<ListNode>());

        let mut previous_node = &mut self.head;

        //find correct place inside list
        while let Some(ref next) = previous_node.next {
            if next.start_addr() > addr {
                break;
            }
            previous_node = previous_node.next.as_mut().unwrap();
        }

        let mut new_node = ListNode::new(size);
        let old_next = previous_node.next.take();

        new_node.next = old_next;
        let new_node_ptr = addr as *mut ListNode;
        new_node_ptr.write_volatile(new_node);
        let new_node_ref = &mut *new_node_ptr;

        //merge forwards
        while let Some(mut next_node) = new_node_ref.next.take() {
            if new_node_ref.end_addr() != next_node.start_addr() {
                new_node_ref.next = Some(next_node);
                break;
            }

            new_node_ref.size += next_node.size;
            new_node_ref.next = next_node.next.take();
        }

        let final_region_start;
        let final_region_size;

        //merge backwards
        if previous_node.size != 0 && previous_node.end_addr() == new_node_ref.start_addr() {
            previous_node.size += new_node_ref.size;
            previous_node.next = new_node_ref.next.take();

            final_region_start = previous_node.start_addr();
            final_region_size = previous_node.size;
        } else {
            final_region_start = new_node_ref.start_addr();
            final_region_size = new_node_ref.size;

            previous_node.next = Some(new_node_ref);
        }

        self.update_top_if_needed(final_region_start, final_region_size);
    }

    /// Looks for a free region with the given size and alignment and removes it from the list
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        loop {
            let mut current = &mut self.head;
            while let Some(ref mut next) = current.next {
                if let Ok(alloc_start) = Self::alloc_from_region(next, size, align) {
                    let region_start = next.start_addr();
                    let region_size = next.size;

                    let mut region = current.next.take().unwrap();
                    current.next = region.next.take();

                    if self.region_contains_top(region_start, region_size) {
                        self.clear_top();
                    }

                    return Some((region, alloc_start));
                } else {
                    current = current.next.as_mut().unwrap();
                }
            }

            if !self.allow_growth {
                return None;
            }

            // we didnt find a region, so allocate new pages to make room for that
            unsafe {
                let map_start = align_up(self.heap_end, PageSize::SIZE_4KB as usize);

                let alloc_start = align_up(map_start, align);
                let alloc_end = alloc_start
                    .checked_add(size)
                    .expect("allocation overflow");

                let map_end = align_up(alloc_end, PageSize::SIZE_4KB as usize);
                let map_size = map_end - map_start;
                let frame_count = (map_size / PageSize::SIZE_4KB as usize) as u64;

                vgaprintln!("Growth: start: {:#011x}, end: {:#011x}, size: {}, contiguous: {}", map_start, map_end, map_size, self.is_contiguous);
                
                if self.is_contiguous {
                    let frame_addr = pmm_allocate_contiguous(frame_count);
                    if frame_addr.is_none() {
                        return None;
                    }
                    
                    vmm_map_range_ext(
                        VirtAddr::new(map_start as u64),
                        frame_addr.unwrap(),
                        map_size as u64,
                        &PageSize::Size4Kb,
                        self.flags
                    );
                } else {
                    let mut page_addr = map_start;
                    while page_addr < map_end {
                        let frame_addr = pmm_allocate_frame();

                        if frame_addr.is_none() {
                            return None;
                        }

                        if !vmm_map_page_ext(VirtAddr::new(page_addr as u64), frame_addr.unwrap(), &PageSize::Size4Kb, self.flags) {
                            return None;
                        }

                        page_addr += PageSize::SIZE_4KB as usize;
                    }
                }

                let new_region_size = map_end - map_start;
                self.heap_end = map_end;
                self.add_free_region(map_start, new_region_size);
            }
        }
    }

    /// Try to use the given region for an allocation with given size and alignment
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        let front_excess_size = alloc_start - region.start_addr();
        let back_excess_size = region.end_addr() - alloc_end;

        if front_excess_size > 0 && front_excess_size < size_of::<ListNode>() {
            return Err(());
        }

        if back_excess_size > 0 && back_excess_size < size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    unsafe fn remove_free_region_by_start(&mut self, region_start: usize) {
        let mut current = &mut self.head;

        //some O(n) garbage, but it's for an edge case so it's fine
        while let Some(ref mut next) = current.next {
            if next.start_addr() == region_start {
                let mut removed = current.next.take().unwrap();
                current.next = removed.next.take();
                return;
            }

            current = current.next.as_mut().unwrap();
        }
    }

    unsafe fn try_shrink_top(&mut self) {
        if self.top_size == 0 {
            return;
        }

        let page_size = PageSize::SIZE_4KB as usize;

        let top_start = self.top_start;
        let top_end = self.top_start + self.top_size;

        if top_end != self.heap_end {
            // invalid top
            self.top_start = 0;
            self.top_size = 0;
            return;
        }

        let protected_size = SizeUnit::Megabyte.as_usize() * 2;
        let protected_end = align_up(self.region_start + protected_size, page_size);

        let mut unmap_start = align_up(top_start, page_size);

        //first 2mb cant be unmapped
        if unmap_start < protected_end {
            unmap_start = protected_end;
        }

        if unmap_start >= self.heap_end {
            return;
        }

        let old_heap_end = self.heap_end;
        let remaining_size = unmap_start - top_start;

        //region smaller than listnode's size cant exist
        if remaining_size < size_of::<ListNode>() {
            self.remove_free_region_by_start(top_start);
            self.top_start = 0;
            self.top_size = 0;
        } else {
            let top_node = &mut *(top_start as *mut ListNode);
            top_node.size = remaining_size;

            self.top_size = remaining_size;
        }

        let mut addr = unmap_start;

        while addr < old_heap_end {
            let frame = vmm_unmap_page(VirtAddr::new(addr as u64));
            pmm_free_frame(frame);

            addr += page_size;
        }

        self.heap_end = unmap_start;
    }

    /// Adjusts the layout so that it can also store listnode
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(size_of::<ListNode>());
        (size, layout.align())
    }

    pub unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let (size, align) = Self::size_align(layout);

        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start + size;
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                self.add_free_region(alloc_end, excess_size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    pub unsafe fn deallocate(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _align) = Self::size_align(layout);
        self.add_free_region(ptr as usize, size);
        self.try_shrink_top();
    }
}

/// Align the given address `addr` upwards to alignment `align`.
pub fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

unsafe impl Send for LinkedListAllocator {}

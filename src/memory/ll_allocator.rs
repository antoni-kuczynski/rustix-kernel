#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use crate::drivers::vga::vga_text::{ColorTextMode, VGAWRITER};
use crate::memory::{align_up, FRAME_SIZE};
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::memory::paging::{
    virtual_to_physical, vmm_map_page, vmm_map_page_ext, vmm_map_range_ext, vmm_unmap_page,
};
use crate::memory::pmm::{
    pmm_allocate_contiguous, pmm_allocate_frame, pmm_free_frame, pmm_free_range,
};
use crate::vgaprintln;
use core::alloc::Layout;
use core::cmp::PartialEq;
use core::ops::Add;
use core::ptr::{addr_of_mut, null_mut};
use core::{mem, ptr};
use x86_64::VirtAddr;

pub struct ListNode {
    pub(crate) size: usize,
    pub(crate) next: *mut ListNode,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode {
            size,
            next: null_mut(),
        }
    }

    pub(crate) fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    pub(crate) fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }

    pub(crate) fn checked_end_addr(&self) -> Option<usize> {
        self.start_addr().checked_add(self.size)
    }
}

impl PartialEq for ListNode {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

pub struct LinkedListAllocator {
    pub(crate) head: ListNode,
    pub(crate) current_end: usize, //4kb aligned current end of heap

    //the whole region allocator works in
    pub(crate) global_start: usize,
    pub(crate) global_end: usize, // virtual end of the region
    pub(crate) global_length: usize,

    //the top region is the last region if its start+size is equal to heap_end, otherwise its 0,0
    pub(crate) top_start: usize,
    pub(crate) top_size: usize,

    pub flags: u64,
    pub is_contiguous: bool,

    debug_last_free_node: usize,
    debug_last_free_size: usize,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
            current_end: 0,
            global_end: 0,
            top_start: 0,
            top_size: 0,
            global_start: 0,
            global_length: 0,
            flags: PageTableEntry::PRESENT | PageTableEntry::WRITABLE,
            is_contiguous: false,
            debug_last_free_node: 0,
            debug_last_free_size: 0,
        }
    }

    /// Initializes the allocator with flags PRESENT and WRITEABLE
    pub unsafe fn init(&mut self, global_start: usize, global_length: usize) {
        self.global_start = global_start;
        self.global_length = global_length;
        self.global_end = self.global_start + global_length;

        self.current_end = global_start;

        self.top_start = 0;
        self.top_size = 0;
        self.debug_last_free_node = 0;
        self.debug_last_free_size = 0;
    }

    pub fn set_flags(&mut self, flags: u64) {
        self.flags = flags;
    }

    pub fn set_contiguous(&mut self, contiguous: bool) {
        self.is_contiguous = contiguous;
    }

    fn update_top_if_needed(&mut self, region_start: usize, region_size: usize) {
        let Some(region_end) = region_start.checked_add(region_size) else {
            return;
        };

        if region_end == self.current_end {
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

        let Some(region_end) = region_start.checked_add(region_size) else {
            return false;
        };
        let top_end = self.top_end();

        region_start <= self.top_start && region_end >= top_end
    }

    fn is_valid_free_node_ptr(&self, node: *mut ListNode) -> bool {
        if node.is_null() {
            return false;
        }

        let addr = node as usize;
        addr >= self.global_start && addr < self.current_end && addr % align_of::<ListNode>() == 0
    }

    unsafe fn validate_free_node_ptr(&self, node: *mut ListNode, context: &str) -> bool {
        if !self.is_valid_free_node_ptr(node) {
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::LightRed);
            vgaprintln!(
                " [ALLOCATOR] Invalid free-list pointer [{}]: ptr=0x{:X}, heap=0x{:X}..0x{:X}",
                context,
                node as usize,
                self.global_start,
                self.current_end
            );
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::White);
            return false;
        }

        let Some(end) = (*node).checked_end_addr() else {
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::LightRed);
            vgaprintln!(
                " [ALLOCATOR] Invalid free-list node overflow [{}]: ptr=0x{:X}, size={}, next=0x{:X}, last_write=0x{:X}/{}",
                context,
                node as usize,
                (*node).size,
                (*node).next as usize,
                self.debug_last_free_node,
                self.debug_last_free_size
            );
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::White);
            return false;
        };

        if end > self.current_end {
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::LightRed);
            vgaprintln!(
                " [ALLOCATOR] Invalid free-list node range [{}]: node=0x{:X}..0x{:X}, heap_end=0x{:X}, size={}, next=0x{:X}, last_write=0x{:X}/{}",
                context,
                node as usize,
                end,
                self.current_end,
                (*node).size,
                (*node).next as usize,
                self.debug_last_free_node,
                self.debug_last_free_size
            );
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::White);
            return false;
        }

        true
    }

    unsafe fn validate_debug_last_free_node(&self, context: &str) -> bool {
        if self.debug_last_free_node == 0 {
            return true;
        }

        let node = self.debug_last_free_node as *mut ListNode;
        if !self.is_valid_free_node_ptr(node) {
            return true;
        }

        if (*node).size != self.debug_last_free_size {
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::LightRed);
            vgaprintln!(
                " [ALLOCATOR] Last free node changed [{}]: node=0x{:X}, expected_size={}, actual_size={}, next=0x{:X}",
                context,
                self.debug_last_free_node,
                self.debug_last_free_size,
                (*node).size,
                (*node).next as usize
            );
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::White);
            return false;
        }

        true
    }

    fn clear_debug_last_free_node_if_inside(&mut self, region_start: usize, region_size: usize) {
        let Some(region_end) = region_start.checked_add(region_size) else {
            return;
        };

        if self.debug_last_free_node >= region_start && self.debug_last_free_node < region_end {
            self.debug_last_free_node = 0;
            self.debug_last_free_size = 0;
        }
    }

    /// Adds a free region with the given address and size to the front of the list
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        let aligned_addr = align_up(addr, align_of::<ListNode>());
        let padding = aligned_addr - addr;

        if addr < self.global_start || addr > self.global_end {
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::LightRed);
            vgaprintln!(" [ALLOCATOR] Invalid region address {:#011x}", addr);
            VGAWRITER
                .lock()
                .change_foreground_color(ColorTextMode::White);
            return;
        }

        let Some(size) = size.checked_sub(padding) else {
            return;
        };

        let size = size & !(align_of::<ListNode>() - 1);

        if size < size_of::<ListNode>() {
            return;
        }

        let addr = aligned_addr;
        assert!(size >= size_of::<ListNode>());

        let Some(region_end) = addr.checked_add(size) else {
            vgaprintln!(
                "Invalid region size overflow: addr={:#011x}, size={}",
                addr,
                size
            );
            return;
        };

        if region_end > self.global_end {
            vgaprintln!(
                "Invalid region range: addr={:#011x}, size={}, end={:#011x}",
                addr,
                size,
                region_end
            );
            return;
        }

        let mut previous_node = addr_of_mut!(self.head);

        //find correct place inside list
        while !(*previous_node).next.is_null() {
            let next = (*previous_node).next;
            if !self.validate_free_node_ptr(next, "add_free_region scan") {
                return;
            }

            if (*next).start_addr() > addr {
                break;
            }
            previous_node = next;
        }

        if (*previous_node).size != 0 {
            let Some(previous_end) = (*previous_node).checked_end_addr() else {
                vgaprintln!(
                    "Invalid previous free node overflow: addr={:#011x}, size={}",
                    (*previous_node).start_addr(),
                    (*previous_node).size
                );
                return;
            };

            if previous_end > addr {
                vgaprintln!(
                    "Free region overlaps previous node: prev=0x{:X}..0x{:X}, new=0x{:X}..0x{:X}",
                    (*previous_node).start_addr(),
                    previous_end,
                    addr,
                    region_end
                );
                return;
            }
        }

        if !(*previous_node).next.is_null() {
            let next = (*previous_node).next;
            if !self.validate_free_node_ptr(next, "add_free_region next overlap") {
                return;
            }

            let next_start = (*next).start_addr();

            if region_end > next_start {
                let next_end = (*next).checked_end_addr().unwrap_or(0);
                vgaprintln!(
                    "Free region overlaps next node: new=0x{:X}..0x{:X}, next=0x{:X}..0x{:X}",
                    addr,
                    region_end,
                    next_start,
                    next_end
                );
                return;
            }
        }

        let mut new_node = ListNode::new(size);
        let old_next = (*previous_node).next;

        new_node.next = old_next;
        let new_node_ptr = addr as *mut ListNode;
        new_node_ptr.write_volatile(new_node);

        if (*new_node_ptr).size != size {
            vgaprintln!(
                "Free node changed immediately after write: addr=0x{:X}, expected_size={}, actual_size={}, next=0x{:X}",
                new_node_ptr as usize,
                size,
                (*new_node_ptr).size,
                (*new_node_ptr).next as usize
            );
            return;
        }

        //merge forwards
        while !(*new_node_ptr).next.is_null() {
            let next_node = (*new_node_ptr).next;
            if !self.validate_free_node_ptr(next_node, "add_free_region forward merge") {
                return;
            }

            let Some(new_node_end) = (*new_node_ptr).checked_end_addr() else {
                vgaprintln!(
                    "Free node overflow before forward merge: addr={:#011x}, size={}",
                    (*new_node_ptr).start_addr(),
                    (*new_node_ptr).size
                );
                return;
            };

            if new_node_end != (*next_node).start_addr() {
                break;
            }

            let Some(merged_size) = (*new_node_ptr).size.checked_add((*next_node).size) else {
                vgaprintln!(
                    "Forward merge size overflow: left=0x{:X} size={}, right=0x{:X} size={}",
                    (*new_node_ptr).start_addr(),
                    (*new_node_ptr).size,
                    (*next_node).start_addr(),
                    (*next_node).size
                );
                return;
            };

            (*new_node_ptr).size = merged_size;
            (*new_node_ptr).next = (*next_node).next;
            (*next_node).next = null_mut();
        }

        let final_region_start;
        let final_region_size;

        //merge backwards
        if (*previous_node).size != 0
            && (*previous_node).checked_end_addr() == Some((*new_node_ptr).start_addr())
        {
            let Some(merged_size) = (*previous_node).size.checked_add((*new_node_ptr).size) else {
                vgaprintln!(
                    "Backward merge size overflow: left=0x{:X} size={}, right=0x{:X} size={}",
                    (*previous_node).start_addr(),
                    (*previous_node).size,
                    (*new_node_ptr).start_addr(),
                    (*new_node_ptr).size
                );
                return;
            };

            (*previous_node).size = merged_size;
            (*previous_node).next = (*new_node_ptr).next;
            (*new_node_ptr).next = null_mut();

            final_region_start = (*previous_node).start_addr();
            final_region_size = (*previous_node).size;
        } else {
            final_region_start = (*new_node_ptr).start_addr();
            final_region_size = (*new_node_ptr).size;

            (*previous_node).next = new_node_ptr;
        }

        self.debug_last_free_node = final_region_start;
        self.debug_last_free_size = final_region_size;

        if !self.validate_debug_last_free_node("add_free_region end") {
            return;
        }

        self.update_top_if_needed(final_region_start, final_region_size);
    }

    /// Looks for a free region with the given size and alignment and removes it from the list
    unsafe fn find_region(&mut self, size: usize, align: usize) -> Option<(usize, usize, usize)> {
        loop {
            let mut current = addr_of_mut!(self.head);

            while !(*current).next.is_null() {
                let next = (*current).next;
                if !self.validate_free_node_ptr(next, "find_region scan") {
                    return None;
                }

                if let Ok(alloc_start) = Self::alloc_from_region(&*next, size, align) {
                    let region_start = (*next).start_addr();
                    let region_size = (*next).size;

                    (*current).next = (*next).next;
                    (*next).next = null_mut();
                    self.clear_debug_last_free_node_if_inside(region_start, region_size);

                    if self.region_contains_top(region_start, region_size) {
                        self.clear_top();
                    }

                    return Some((region_start, region_size, alloc_start));
                } else {
                    current = next;
                }
            }

            // we didnt find a region, so allocate new pages to make room for that
            unsafe {
                let map_start = self.current_end.add(PageSize::SIZE_4KB as usize - 1)
                    & !(PageSize::SIZE_4KB as usize - 1);
                let alloc_start = map_start.add(align - 1) & !(align - 1);
                let alloc_end = alloc_start.add(size);
                let map_end = alloc_end.add(PageSize::SIZE_4KB as usize - 1)
                    & !(PageSize::SIZE_4KB as usize - 1);

                let map_size = map_end - map_start;
                let frame_count = (map_size / PageSize::SIZE_4KB as usize) as u64;

                if self.is_contiguous {
                    let frame_addr = pmm_allocate_contiguous(frame_count);
                    if frame_addr.is_none() {
                        return None;
                    }

                    if !vmm_map_range_ext(
                        VirtAddr::new(map_start as u64),
                        frame_addr.unwrap(),
                        map_size as u64,
                        &PageSize::Size4Kb,
                        self.flags,
                    ) {
                        pmm_free_range(frame_addr.unwrap(), map_size as u64).ok();
                        return None;
                    }
                } else {
                    let mut page_addr = map_start;
                    while page_addr < map_end {
                        let frame_addr = pmm_allocate_frame();
                        if frame_addr.is_none() {
                            //rollback already mapped pages
                            let mut rollback_addr = map_start;
                            while rollback_addr < page_addr {
                                let phys = vmm_unmap_page(VirtAddr::new(rollback_addr as u64));
                                pmm_free_frame(phys).ok();
                                rollback_addr += PageSize::SIZE_4KB as usize;
                            }
                            return None;
                        }

                        if !vmm_map_page_ext(
                            VirtAddr::new(page_addr as u64),
                            frame_addr.unwrap(),
                            &PageSize::Size4Kb,
                            self.flags,
                        ) {
                            //rollback already mapped pages
                            pmm_free_frame(frame_addr.unwrap()).ok();
                            let mut rollback_addr = map_start;
                            while rollback_addr < page_addr {
                                let phys = vmm_unmap_page(VirtAddr::new(rollback_addr as u64));
                                pmm_free_frame(phys).ok();
                                rollback_addr += PageSize::SIZE_4KB as usize;
                            }
                            return None;
                        }

                        page_addr += PageSize::SIZE_4KB as usize;
                    }
                }

                let new_region_size = map_end - map_start;
                self.current_end = map_end;
                self.add_free_region(map_start, new_region_size);
            }
        }
    }

    /// Try to use the given region for an allocation with given size and alignment
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        let region_end = region.checked_end_addr().ok_or(())?;

        if alloc_end > region_end {
            return Err(());
        }

        let front_excess_size = alloc_start - region.start_addr();
        let back_excess_size = region_end - alloc_end;

        if front_excess_size > 0 && front_excess_size < size_of::<ListNode>() {
            return Err(());
        }

        if back_excess_size > 0 && back_excess_size < size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    unsafe fn remove_free_region_by_start(&mut self, region_start: usize) {
        let mut current = addr_of_mut!(self.head);

        //some O(n) garbage, but it's for an edge case so it's fine
        while !(*current).next.is_null() {
            let next = (*current).next;
            if !self.validate_free_node_ptr(next, "remove_free_region_by_start scan") {
                return;
            }

            if (*next).start_addr() == region_start {
                let region_size = (*next).size;
                (*current).next = (*next).next;
                (*next).next = null_mut();
                self.clear_debug_last_free_node_if_inside(region_start, region_size);
                return;
            }

            current = next;
        }
    }

    unsafe fn try_shrink_top(&mut self) {
        if self.top_size == 0 {
            return;
        }

        let page_size = PageSize::SIZE_4KB as usize;

        let top_start = self.top_start;
        let top_end = self.top_start + self.top_size;

        if top_end != self.current_end {
            // invalid top
            self.top_start = 0;
            self.top_size = 0;
            return;
        }

        let unmap_start = align_up(top_start, page_size);

        if unmap_start >= self.current_end {
            return;
        }

        let old_heap_end = self.current_end;
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

        self.current_end = unmap_start;
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

        if !self.validate_debug_last_free_node("allocate entry") {
            return ptr::null_mut();
        }

        if let Some((region_start, region_size, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start + size;
            let Some(region_end) = region_start.checked_add(region_size) else {
                vgaprintln!(
                    "Invalid allocation region overflow: start=0x{:X}, size={}",
                    region_start,
                    region_size
                );
                return ptr::null_mut();
            };

            if alloc_end > region_end {
                vgaprintln!(
                    "Invalid allocation split: region=0x{:X}..0x{:X}, alloc=0x{:X}..0x{:X}",
                    region_start,
                    region_end,
                    alloc_start,
                    alloc_end
                );
                return ptr::null_mut();
            }

            let front_excess_size = alloc_start - region_start;

            if alloc_start < region_start {
                vgaprintln!("ALLOC START < REGION START");
            }

            if front_excess_size > 0 {
                self.add_free_region(region_start, front_excess_size);
            }

            let back_excess_size = region_end - alloc_end;

            if region_end < alloc_end {
                vgaprintln!("ALLOC END < REGION END");
            }

            if back_excess_size > 0 {
                self.add_free_region(alloc_end, back_excess_size);
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



unsafe impl Send for LinkedListAllocator {}

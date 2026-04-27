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
use crate::vgaprintln;

pub struct ListNode {
    pub(crate) size: usize,
    pub(crate) next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
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
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds a free region with the given address and size to the front of the list
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        let addr = align_up(addr, align_of::<ListNode>());
        let size = size & !(size_of::<ListNode>() - 1);
        assert!(size >= size_of::<ListNode>());

        let mut current = &mut self.head;

        //find correct place inside list
        while let Some(ref next) = current.next {
            if next.start_addr() > addr {
                break;
            }
            current = current.next.as_mut().unwrap();
        }

        let mut new_node = ListNode::new(size);
        new_node.next = current.next.take();
        let new_node_ptr = addr as *mut ListNode;
        new_node_ptr.write_volatile(new_node);
        let new_node_ref = &mut *new_node_ptr;

        //merge forwards
        let new_node_end = new_node_ref.end_addr();

        if let Some(ref mut next_node) = new_node_ref.next {
            if new_node_end == next_node.start_addr() {
                new_node_ref.size += next_node.size;
                new_node_ref.next = next_node.next.take();
            }
        }

        //merge backwards
        if current.size != 0 && current.end_addr() == addr {
            current.size += new_node_ref.size;
            current.next = new_node_ref.next.take();
        } else {
            current.next = Some(new_node_ref);
        }
    }

    /// Looks for a free region with the given size and alignment and removes it from the list
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;
        while let Some(ref mut next) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(next, size, align) {
                let next = current.next.take().unwrap();
                current.next = next.next.take();
                return Some((next, alloc_start));
            } else {
                current = current.next.as_mut().unwrap();
            }
        }
        None
    }

    /// Try to use the given region for an allocation with given size and alignment
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < size_of::<ListNode>() {
            return Err(()); // no space left for list node
        }

        Ok(alloc_start)
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
        self.add_free_region(ptr as usize, size)
    }
}

/// Align the given address `addr` upwards to alignment `align`.
pub fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

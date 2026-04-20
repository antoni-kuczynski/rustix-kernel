#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use core::alloc::Layout;
use core::ptr::null_mut;

struct ListNode {
    size: u64,
    next: Option<*mut ListNode>
}

struct LinkedListAllocator {
    head: ListNode
}

impl ListNode {
    const fn new(size: u64) -> Self {
        ListNode { size, next: None }
    }
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

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {

    }

    pub fn alloc_from_region(&mut self, layout: Layout) -> *mut u8 {

        null_mut()
    }

    pub unsafe fn dealloc_to_region(&mut self, ptr: *mut u8, layout: Layout) {

    }
}
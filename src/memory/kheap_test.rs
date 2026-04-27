/*
 * Created by Antoni Kuczyński
 * 27/04/2026
 */

use core::alloc::Layout;
use crate::memory::ll_allocator::LinkedListAllocator;
use crate::vgaprintln;

pub fn run_all_tests(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n--- STARTING KHEAP TEST SUITE ---");

    dump_debug(allocator);

    test_fragmentation_and_reclaim(allocator);

    // dump_debug(allocator);

    // test_basic_malloc_free(allocator);
    // test_coalescing(allocator);
    // test_fragmentation_and_reclaim(allocator);
    // test_overflow_protection(allocator);
    // test_node_integrity(allocator);

    vgaprintln!("--- ALL KHEAP TESTS COMPLETED ---\n");
}

fn dump_debug(allocator: &mut LinkedListAllocator) {
    vgaprintln!("[DEBUG] Current Free List:");
    unsafe {
        let mut current_ptr = core::ptr::addr_of!(allocator.head);
        let mut i = 0;

        let mut current_next = (*current_ptr).next.as_ref();

        while let Some(node) = current_next {
            vgaprintln!(
                "  Node {}: Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}",
                i,
                node as *const _ as usize,
                node.size,
                node as *const _ as usize + node.size
            );
            current_next = node.next.as_ref();
            i += 1;
        }
        if i == 0 {
            vgaprintln!("  (List is empty - heap fully allocated)");
        }
    }
}

fn test_basic_malloc_free(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n[TEST] Basic Malloc/Free");
    let layout = Layout::from_size_align(64, 8).unwrap();

    unsafe {
        let ptr = allocator.allocate(layout);
        assert!(!ptr.is_null(), "Allocation failed!");
        vgaprintln!("  Allocated 64 bytes at {:?}", ptr);

        ptr.write_bytes(0xAA, 64);

        allocator.deallocate(ptr, layout);
        vgaprintln!("  Deallocated successfully.");
    }
    dump_debug(allocator);
}

fn test_coalescing(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n[TEST] Block Coalescing (Merging)");
    let layout = Layout::from_size_align(1024, 8).unwrap();

    unsafe {
        let p1 = allocator.allocate(layout);
        let p2 = allocator.allocate(layout);
        let p3 = allocator.allocate(layout);

        vgaprintln!("  Allocated 3x 1KB. Freeing middle and neighbors...");
        allocator.deallocate(p2, layout);
        allocator.deallocate(p1, layout);
        allocator.deallocate(p3, layout);

    }
    dump_debug(allocator);
}

fn test_fragmentation_and_reclaim(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n[TEST] Fragmentation Reclaim");
    let layout_small = Layout::from_size_align(256, 8).unwrap();

    unsafe {
        let mut ptrs = [core::ptr::null_mut::<u8>(); 4];
        for i in 0..4 {
            ptrs[i] = allocator.allocate(layout_small);
        }

        vgaprintln!("  Creating 'swiss cheese' memory (freeing indices 0 and 2)...");
        allocator.deallocate(ptrs[0], layout_small);
        allocator.deallocate(ptrs[2], layout_small);
        dump_debug(allocator);

        vgaprintln!("  Reclaiming all...");
        allocator.deallocate(ptrs[1], layout_small);
        allocator.deallocate(ptrs[3], layout_small);
    }
    dump_debug(allocator);
}

fn test_overflow_protection(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n[TEST] Heap Overflow Protection");

    let layout_huge = Layout::from_size_align(1024 * 1024 * 1024, 8).unwrap();

    unsafe {
        let ptr = allocator.allocate(layout_huge);
        if ptr.is_null() {
            vgaprintln!("  OK: Allocation of 1GB failed as expected.");
        } else {
            vgaprintln!("  FAIL: Allocated impossible amount of memory!");
            allocator.deallocate(ptr, layout_huge);
        }
    }
}

fn test_node_integrity(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n[TEST] Node Integrity");
    let size = 512;
    let layout = Layout::from_size_align(size, 16).unwrap();

    unsafe {
        let p1 = allocator.allocate(layout);
        core::ptr::write_bytes(p1, 0xEE, size);

        let p2 = allocator.allocate(layout);
        assert!(!p2.is_null(), "Node metadata corrupted by previous allocation!");

        vgaprintln!("  Integrity check passed (allocated successfully after heavy write).");
        allocator.deallocate(p1, layout);
        allocator.deallocate(p2, layout);
    }
    dump_debug(allocator);
}
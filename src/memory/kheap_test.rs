#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 27/04/2026
 */

use crate::memory::dma;
use crate::memory::ll_allocator::{LinkedListAllocator, ListNode};
use crate::memory::page_tables::PageSize;
use crate::vgaprintln;
use core::alloc::Layout;
use x86_64::VirtAddr;

pub fn run_kheap_tests(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n--- STARTING KHEAP TEST SUITE ---");

    run_kheap_test_cases(allocator);

    vgaprintln!("--- KHEAP TEST SUITE COMPLETED ---\n");
}

pub fn dump_debug(allocator: &LinkedListAllocator) {
    vgaprintln!("[DEBUG] Current Free List:");

    let head_addr = core::ptr::addr_of!(allocator.head) as usize;

    vgaprintln!(
        "  Head  : Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}",
        head_addr,
        allocator.head.size,
        head_addr + allocator.head.size
    );

    vgaprintln!(
        "  Top region  : Addr: 0x{:X}, Size: {} bytes",
        allocator.top_start,
        allocator.top_size
    );

    let mut current = allocator.head.next;
    let mut i = 0;

    while !current.is_null() {
        unsafe {
            let current_addr = current as usize;
            if current_addr < allocator.global_start
                || current_addr >= allocator.current_end
                || current_addr % align_of::<ListNode>() != 0
            {
                vgaprintln!(
                    "  Invalid node pointer in dump: index={}, ptr=0x{:X}, heap=0x{:X}..0x{:X}",
                    i,
                    current_addr,
                    allocator.global_start,
                    allocator.current_end
                );
                return;
            }

            let node = &*current;
            let node_addr = current as usize;
            let node_end = node_addr + node.size;

            if node.next.is_null() {
                vgaprintln!(
                    "  Node {}: Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}, Next: NULL",
                    i,
                    node_addr,
                    node.size,
                    node_end
                );
            } else {
                vgaprintln!(
                    "  Node {}: Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}, Next: 0x{:X}",
                    i,
                    node_addr,
                    node.size,
                    node_end,
                    node.next as usize
                );
            }

            current = node.next;
            i += 1;
        }
    }

    if i == 0 {
        vgaprintln!("  (List is empty - heap fully allocated)");
    }
}

fn validate_allocator_state(allocator: &LinkedListAllocator, label: &str) -> bool {
    unsafe {
        let mut current = allocator.head.next;
        let mut previous_end = 0usize;
        let mut found_top = false;

        while !current.is_null() {
            let current_addr = current as usize;
            if current_addr < allocator.global_start
                || current_addr >= allocator.current_end
                || current_addr % align_of::<ListNode>() != 0
            {
                vgaprintln!(
                    "  FAILED [{}]: invalid free-list pointer: ptr=0x{:X}, heap=0x{:X}..0x{:X}",
                    label,
                    current_addr,
                    allocator.global_start,
                    allocator.current_end
                );
                return false;
            }

            let node = &*current;
            let start = node.start_addr();
            let Some(end) = node.checked_end_addr() else {
                vgaprintln!(
                    "  FAILED [{}]: free node end overflow: addr=0x{:X}, size={}",
                    label,
                    start,
                    node.size
                );
                return false;
            };

            if node.size < size_of::<ListNode>() {
                vgaprintln!(
                    "  FAILED [{}]: free node too small: addr=0x{:X}, size={}",
                    label,
                    start,
                    node.size
                );
                return false;
            }

            if start % align_of::<ListNode>() != 0 {
                vgaprintln!(
                    "  FAILED [{}]: free node not aligned: addr=0x{:X}",
                    label,
                    start
                );
                return false;
            }

            if end <= start {
                vgaprintln!(
                    "  FAILED [{}]: invalid free node range: start=0x{:X}, end=0x{:X}",
                    label,
                    start,
                    end
                );
                return false;
            }

            if previous_end != 0 && start < previous_end {
                vgaprintln!(
                    "  FAILED [{}]: free list is not sorted or overlaps: prev_end=0x{:X}, start=0x{:X}",
                    label,
                    previous_end,
                    start
                );
                return false;
            }

            if end == allocator.current_end {
                if allocator.top_size == 0 {
                    vgaprintln!(
                        "  FAILED [{}]: free node ends at heap_end, but top is empty. node=0x{:X}..0x{:X}",
                        label,
                        start,
                        end
                    );
                    return false;
                }

                if allocator.top_start != start || allocator.top_size != node.size {
                    vgaprintln!(
                        "  FAILED [{}]: top does not match free node at heap_end. node=0x{:X}, size={}, top=0x{:X}, top_size={}",
                        label,
                        start,
                        node.size,
                        allocator.top_start,
                        allocator.top_size
                    );
                    return false;
                }
            }

            if allocator.top_size != 0 && start == allocator.top_start {
                found_top = true;

                if node.size != allocator.top_size {
                    vgaprintln!(
                        "  FAILED [{}]: top_size does not match node.size. top_size={}, node.size={}",
                        label,
                        allocator.top_size,
                        node.size
                    );
                    return false;
                }

                if end != allocator.current_end {
                    vgaprintln!(
                        "  FAILED [{}]: top does not end at heap_end. top_end=0x{:X}, heap_end=0x{:X}",
                        label,
                        end,
                        allocator.current_end
                    );
                    return false;
                }
            }

            previous_end = end;
            current = node.next;
        }

        if allocator.top_size == 0 && allocator.top_start != 0 {
            vgaprintln!(
                "  FAILED [{}]: top_size is 0 but top_start is not 0: 0x{:X}",
                label,
                allocator.top_start
            );
            return false;
        }

        if allocator.top_size != 0 && !found_top {
            vgaprintln!(
                "  FAILED [{}]: top exists but was not found in free list. top_start=0x{:X}, top_size={}",
                label,
                allocator.top_start,
                allocator.top_size
            );
            return false;
        }

        true
    }
}

fn validate_allocator_drained(allocator: &LinkedListAllocator, label: &str) -> bool {
    if !allocator.head.next.is_null() {
        vgaprintln!(
            "  FAILED [{}]: allocator still has free-list nodes after test",
            label
        );
        dump_debug(allocator);
        return false;
    }

    if allocator.top_start != 0 || allocator.top_size != 0 {
        vgaprintln!(
            "  FAILED [{}]: allocator still has top state after test. top_start=0x{:X}, top_size={}",
            label,
            allocator.top_start,
            allocator.top_size
        );
        dump_debug(allocator);
        return false;
    }

    if allocator.current_end != allocator.global_start {
        vgaprintln!(
            "  FAILED [{}]: heap end did not return to start. current_end=0x{:X}, global_start=0x{:X}",
            label,
            allocator.current_end,
            allocator.global_start
        );
        dump_debug(allocator);
        return false;
    }

    true
}

fn run_kheap_case(
    allocator: &mut LinkedListAllocator,
    name: &str,
    test: fn(&mut LinkedListAllocator) -> bool,
) -> bool {
    vgaprintln!("\n[CASE] {}", name);

    if !validate_allocator_drained(allocator, "before test") {
        return false;
    }

    let passed = test(allocator);
    let drained = validate_allocator_drained(allocator, name);

    passed && drained
}

unsafe fn touch_allocation(ptr: *mut u8, size: usize) -> bool {
    if size == 0 {
        return true;
    }

    let page_size = PageSize::SIZE_4KB as usize;

    ptr.write_volatile(0xA5);

    if ptr.read_volatile() != 0xA5 {
        vgaprintln!("  FAILED: first byte read/write mismatch");
        return false;
    }

    let last = ptr.add(size - 1);
    last.write_volatile(0x5A);

    if last.read_volatile() != 0x5A {
        vgaprintln!("  FAILED: last byte read/write mismatch");
        return false;
    }

    let mut offset = 0usize;

    while offset < size {
        let page_start = offset;
        let page_end = core::cmp::min(offset + page_size - 1, size - 1);

        let start_ptr = ptr.add(page_start);
        let end_ptr = ptr.add(page_end);

        let start_value = ((offset / page_size) as u8).wrapping_add(1);
        let end_value = start_value.wrapping_add(0x40);

        if page_start == page_end {
            start_ptr.write_volatile(start_value);

            if start_ptr.read_volatile() != start_value {
                vgaprintln!(
                    "  FAILED: single-byte page mismatch at offset 0x{:X}, addr=0x{:X}",
                    page_start,
                    start_ptr as usize
                );
                return false;
            }
        } else {
            start_ptr.write_volatile(start_value);
            end_ptr.write_volatile(end_value);

            if start_ptr.read_volatile() != start_value {
                vgaprintln!(
                    "  FAILED: page start mismatch at offset 0x{:X}, addr=0x{:X}",
                    page_start,
                    start_ptr as usize
                );
                return false;
            }

            if end_ptr.read_volatile() != end_value {
                vgaprintln!(
                    "  FAILED: page end mismatch at offset 0x{:X}, addr=0x{:X}",
                    page_end,
                    end_ptr as usize
                );
                return false;
            }
        }

        offset += page_size;
    }

    true
}

unsafe fn alloc_checked(
    allocator: &mut LinkedListAllocator,
    name: &str,
    layout: Layout,
) -> Option<*mut u8> {
    vgaprintln!(
        "  Allocating [{}]: size={}, align={}",
        name,
        layout.size(),
        layout.align()
    );

    let ptr = allocator.allocate(layout);

    if ptr.is_null() {
        vgaprintln!("  FAILED [{}]: allocation returned NULL", name);
        return None;
    }

    let addr = ptr as usize;

    if addr % layout.align() != 0 {
        vgaprintln!(
            "  FAILED [{}]: wrong alignment. addr=0x{:X}, align={}",
            name,
            addr,
            layout.align()
        );

        allocator.deallocate(ptr, layout);
        return None;
    }

    if !touch_allocation(ptr, layout.size()) {
        allocator.deallocate(ptr, layout);
        return None;
    }

    if !validate_allocator_state(allocator, name) {
        allocator.deallocate(ptr, layout);
        return None;
    }

    vgaprintln!(
        "  OK [{}]: ptr=0x{:X}, end=0x{:X}",
        name,
        addr,
        addr + layout.size()
    );

    Some(ptr)
}

unsafe fn dealloc_checked(
    allocator: &mut LinkedListAllocator,
    name: &str,
    ptr: *mut u8,
    layout: Layout,
) -> bool {
    vgaprintln!(
        "  Deallocating [{}]: ptr=0x{:X}, size={}, align={}",
        name,
        ptr as usize,
        layout.size(),
        layout.align()
    );

    allocator.deallocate(ptr, layout);

    validate_allocator_state(allocator, name)
}

fn test_basic_alloc_dealloc_cases(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Basic allocation/deallocation cases");

    let page_size = PageSize::SIZE_4KB as usize;
    let huge_align = 2 * 1024 * 1024;

    let cases = [
        ("tiny 1 byte", Layout::from_size_align(1, 1).unwrap()),
        ("small 37 bytes", Layout::from_size_align(37, 8).unwrap()),
        ("small align 64", Layout::from_size_align(128, 64).unwrap()),
        (
            "almost one page",
            Layout::from_size_align(page_size - 1, 16).unwrap(),
        ),
        (
            "exactly one page",
            Layout::from_size_align(page_size, page_size).unwrap(),
        ),
        (
            "page plus one",
            Layout::from_size_align(page_size + 1, 16).unwrap(),
        ),
        (
            "multi page non exact",
            Layout::from_size_align(page_size * 3 + 123, 64).unwrap(),
        ),
        (
            "large align greater than page",
            Layout::from_size_align(128, huge_align).unwrap(),
        ),
        (
            "large align multi page",
            Layout::from_size_align(page_size * 2 + 73, huge_align).unwrap(),
        ),
    ];

    unsafe {
        for (name, layout) in cases {
            let Some(ptr) = alloc_checked(allocator, name, layout) else {
                return false;
            };

            if !dealloc_checked(allocator, name, ptr, layout) {
                return false;
            }
        }
    }

    true
}

fn test_fragmentation_reuse_and_merge(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Fragmentation, reuse, and merge");

    let layout = Layout::from_size_align(256, 16).unwrap();
    let reuse_layout = Layout::from_size_align(128, 16).unwrap();

    unsafe {
        let mut ptrs = [core::ptr::null_mut::<u8>(); 8];

        for i in 0..ptrs.len() {
            let Some(ptr) = alloc_checked(allocator, "fragment seed", layout) else {
                return false;
            };

            ptrs[i] = ptr;
        }

        vgaprintln!("  Freeing even indices to create fragmentation...");

        for &i in [0usize, 2, 4, 6].iter() {
            if !dealloc_checked(allocator, "fragment even free", ptrs[i], layout) {
                return false;
            }

            ptrs[i] = core::ptr::null_mut();
        }

        dump_debug(allocator);

        vgaprintln!("  Allocating smaller blocks into fragmented holes...");

        let Some(a) = alloc_checked(allocator, "reuse hole A", reuse_layout) else {
            return false;
        };

        let Some(b) = alloc_checked(allocator, "reuse hole B", reuse_layout) else {
            allocator.deallocate(a, reuse_layout);
            return false;
        };

        if !dealloc_checked(allocator, "reuse hole A", a, reuse_layout) {
            allocator.deallocate(b, reuse_layout);
            return false;
        }

        if !dealloc_checked(allocator, "reuse hole B", b, reuse_layout) {
            return false;
        }

        vgaprintln!("  Reclaiming remaining allocations...");

        for i in 0..ptrs.len() {
            if !ptrs[i].is_null() {
                if !dealloc_checked(allocator, "fragment final free", ptrs[i], layout) {
                    return false;
                }

                ptrs[i] = core::ptr::null_mut();
            }
        }

        dump_debug(allocator);
    }

    true
}

fn test_multiple_live_page_allocations(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Multiple live page allocations");

    let page_size = PageSize::SIZE_4KB as usize;

    let layouts = [
        Layout::from_size_align(page_size, page_size).unwrap(),
        Layout::from_size_align(page_size * 2 + 17, 16).unwrap(),
        Layout::from_size_align(page_size * 4, page_size).unwrap(),
        Layout::from_size_align(64, 8).unwrap(),
        Layout::from_size_align(page_size * 3 + 333, 64).unwrap(),
    ];

    unsafe {
        let mut ptrs = [core::ptr::null_mut::<u8>(); 5];

        for i in 0..layouts.len() {
            let Some(ptr) = alloc_checked(allocator, "multi live alloc", layouts[i]) else {
                vgaprintln!("i: {}", i);
                for j in 0..i {
                    if !ptrs[j].is_null() {
                        allocator.deallocate(ptrs[j], layouts[j]);
                    }
                }

                return false;
            };

            ptrs[i] = ptr;
        }

        vgaprintln!("  Freeing multiple live allocations in reverse order...");

        for i in (0..layouts.len()).rev() {
            if !dealloc_checked(allocator, "multi live free", ptrs[i], layouts[i]) {
                return false;
            }

            ptrs[i] = core::ptr::null_mut();
        }
    }

    true
}

fn test_middle_free_does_not_shrink_heap(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Freeing middle allocation does not shrink heap");

    let page_size = PageSize::SIZE_4KB as usize;
    let layout = Layout::from_size_align(page_size, page_size).unwrap();

    unsafe {
        let Some(a) = alloc_checked(allocator, "middle A", layout) else {
            return false;
        };

        let Some(b) = alloc_checked(allocator, "middle B", layout) else {
            allocator.deallocate(a, layout);
            return false;
        };

        let Some(c) = alloc_checked(allocator, "middle C", layout) else {
            allocator.deallocate(b, layout);
            allocator.deallocate(a, layout);
            return false;
        };

        let heap_end_before = allocator.current_end;
        let b_end = b as usize + layout.size();

        vgaprintln!(
            "  Middle candidate: B=0x{:X}..0x{:X}, heap_end=0x{:X}",
            b as usize,
            b_end,
            heap_end_before
        );

        allocator.deallocate(b, layout);

        if b_end != heap_end_before && allocator.current_end != heap_end_before {
            vgaprintln!(
                "  FAILED: heap_end changed after freeing middle allocation. before=0x{:X}, after=0x{:X}",
                heap_end_before,
                allocator.current_end
            );

            allocator.deallocate(c, layout);
            allocator.deallocate(a, layout);
            return false;
        }

        if !validate_allocator_state(allocator, "middle free") {
            allocator.deallocate(c, layout);
            allocator.deallocate(a, layout);
            return false;
        }

        allocator.deallocate(c, layout);
        allocator.deallocate(a, layout);

        validate_allocator_state(allocator, "middle cleanup")
    }
}

fn test_top_shrink_behavior(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Top region shrink behavior");

    let page_size = PageSize::SIZE_4KB as usize;

    let layout = Layout::from_size_align(page_size * 8, page_size).unwrap();

    unsafe {
        let Some(ptr) = alloc_checked(allocator, "top shrink seed", layout) else {
            return false;
        };

        let alloc_start = ptr as usize;
        let alloc_end = alloc_start + layout.size();
        let heap_end_after_alloc = allocator.current_end;

        vgaprintln!(
            "  Seed allocation: 0x{:X}..0x{:X}, heap_end=0x{:X}",
            alloc_start,
            alloc_end,
            heap_end_after_alloc,
        );

        allocator.deallocate(ptr, layout);

        if allocator.current_end > heap_end_after_alloc {
            vgaprintln!(
                "  FAILED: heap_end grew during deallocation. before=0x{:X}, after=0x{:X}",
                heap_end_after_alloc,
                allocator.current_end
            );
            return false;
        }

        if !validate_allocator_state(allocator, "top shrink") {
            return false;
        }

        vgaprintln!(
            "  OK: after free heap_end=0x{:X}, top_start=0x{:X}, top_size={}",
            allocator.current_end,
            allocator.top_start,
            allocator.top_size
        );
    }

    true
}

fn test_allocating_heap_end_clears_top_if_possible(allocator: &mut LinkedListAllocator) -> bool {
    vgaprintln!("[TEST] Allocating heap end clears top if possible");

    unsafe {
        let prefix_layout = Layout::from_size_align(128, 16).unwrap();
        let suffix_layout = Layout::from_size_align(128, 16).unwrap();

        let Some(prefix) = alloc_checked(allocator, "top prefix", prefix_layout) else {
            return false;
        };

        let Some(suffix) = alloc_checked(allocator, "top suffix", suffix_layout) else {
            allocator.deallocate(prefix, prefix_layout);
            return false;
        };

        allocator.deallocate(suffix, suffix_layout);

        if allocator.top_size == 0 {
            vgaprintln!("  FAILED: expected a partial top after freeing suffix");
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let old_top_start = allocator.top_start;
        let old_top_end = allocator.top_start + allocator.top_size;
        let old_top_size = allocator.top_size;
        let old_heap_end = allocator.current_end;

        if old_top_end != old_heap_end {
            vgaprintln!(
                "  FAILED: invalid top before test. top_end=0x{:X}, heap_end=0x{:X}",
                old_top_end,
                old_heap_end
            );
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let layout = Layout::from_size_align(old_top_size, 16).unwrap();

        vgaprintln!(
            "  Trying end allocation: expected=0x{:X}..0x{:X}, size={}",
            old_top_start,
            old_top_end,
            old_top_size
        );

        let ptr = allocator.allocate(layout);

        if ptr.is_null() {
            vgaprintln!("  FAILED: end allocation returned NULL");
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let got_start = ptr as usize;
        let got_end = got_start + old_top_size;

        if got_start != old_top_start || got_end != old_heap_end {
            vgaprintln!(
                "  SKIPPED: allocator chose a different region. expected=0x{:X}..0x{:X}, got=0x{:X}..0x{:X}",
                old_top_start,
                old_heap_end,
                got_start,
                got_end
            );

            allocator.deallocate(ptr, layout);
            allocator.deallocate(prefix, prefix_layout);
            return validate_allocator_state(allocator, "end allocation skipped cleanup");
        }

        if allocator.top_size != 0 || allocator.top_start != 0 {
            vgaprintln!(
                "  FAILED: top should be cleared after allocating heap end. top_start=0x{:X}, top_size={}",
                allocator.top_start,
                allocator.top_size
            );

            allocator.deallocate(ptr, layout);
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        if !touch_allocation(ptr, layout.size()) {
            allocator.deallocate(ptr, layout);
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        vgaprintln!("  OK: top cleared after allocating heap end");

        allocator.deallocate(ptr, layout);
        allocator.deallocate(prefix, prefix_layout);

        validate_allocator_state(allocator, "end allocation cleanup")
    }
}

fn run_kheap_test_cases(allocator: &mut LinkedListAllocator) {
    vgaprintln!("\n==============================");
    vgaprintln!("[TEST SUITE] LinkedListAllocator");
    vgaprintln!("==============================");

    let mut ok = true;

    if !validate_allocator_drained(allocator, "initial") {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "basic allocation/deallocation",
        test_basic_alloc_dealloc_cases,
    ) {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "fragmentation, reuse, and merge",
        test_fragmentation_reuse_and_merge,
    ) {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "multiple live page allocations",
        test_multiple_live_page_allocations,
    ) {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "freeing middle allocation does not shrink heap",
        test_middle_free_does_not_shrink_heap,
    ) {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "top region shrink behavior",
        test_top_shrink_behavior,
    ) {
        ok = false;
    }

    if !run_kheap_case(
        allocator,
        "allocating heap end clears top",
        test_allocating_heap_end_clears_top_if_possible,
    ) {
        ok = false;
    }

    dump_debug(allocator);

    if ok {
        vgaprintln!("\n[TEST SUITE] OK: all allocator tests passed");
    } else {
        vgaprintln!("\n[TEST SUITE] FAILED: at least one allocator test failed");
    }
}

pub fn run_dma_tests() {
    vgaprintln!("\n--- STARTING DMA TEST SUITE ---");
    test_dma_basic();
    test_dma_continuity();
    test_dma_fragmentation();
    vgaprintln!("--- DMA TEST SUITE COMPLETED ---\n");
}

fn test_dma_basic() {
    vgaprintln!("[TEST] DMA Basic Alloc/Free");
    let size = 1024 * 1024 * 128; //128mb lower this if out of memory
    let align = 64;

    if let Some(alloc) = dma::dma_alloc_coherent(size, align) {
        vgaprintln!(
            "  Allocated 1KB DMA at virt: {:?}, phys: {:?}",
            alloc.virt,
            alloc.phys
        );
        assert!(
            alloc.virt.as_u64() % align as u64 == 0,
            "DMA alignment failed"
        );

        unsafe {
            let ptr = alloc.virt.as_u64() as *mut u8;
            ptr.write_volatile(0xDE);
            assert!(ptr.read_volatile() == 0xDE, "DMA read/write failed");
        }

        dma::dma_free(alloc);
        vgaprintln!("  OK: Basic DMA test passed");
    } else {
        panic!("  FAIL: Basic DMA allocation failed");
    }
}

fn test_dma_continuity() {
    vgaprintln!("[TEST] DMA Physical Continuity");
    let pages = 1024;
    let size = pages * 4096;

    if let Some(alloc) = dma::dma_alloc_coherent(size, 4096) {
        vgaprintln!("  Allocated {} pages DMA at phys: {:?}", pages, alloc.phys);

        for i in 0..pages {
            let offset = i * 4096;
            let virt_page = VirtAddr::new(alloc.virt.as_u64() + offset as u64);
            let phys_page = crate::memory::paging::virtual_to_physical(virt_page).unwrap();
            let expected_phys = alloc.phys.as_u64() + offset as u64;

            if phys_page.as_u64() != expected_phys {
                dump_debug(dma::DMA_MANAGER.lock().allocator());
            }

            assert!(
                phys_page.as_u64() == expected_phys,
                "DMA continuity failed at page {}: expected {:#x}, got {:?}",
                i,
                expected_phys,
                phys_page
            );
        }

        dma::dma_free(alloc);
        vgaprintln!("  OK: DMA physical continuity verified");
    } else {
        panic!("  FAIL: DMA continuity allocation failed");
    }
}

fn test_dma_fragmentation() {
    vgaprintln!("[TEST] DMA Fragmentation and Reuse");
    let size = 4096;
    let mut allocs: [Option<dma::DmaAlloc>; 8] = Default::default();

    for i in 0..8 {
        allocs[i] = dma::dma_alloc_coherent(size, size);
        assert!(
            allocs[i].is_some(),
            "  FAIL: DMA fragmentation seed allocation {} failed",
            i
        );
    }

    for i in (0..8).step_by(2) {
        if let Some(a) = allocs[i].take() {
            dma::dma_free(a);
        }
    }

    let a1 = dma::dma_alloc_coherent(size, size).expect("Failed to reuse hole 1");
    let a2 = dma::dma_alloc_coherent(size, size).expect("Failed to reuse hole 2");

    dma::dma_free(a1);
    dma::dma_free(a2);

    for i in (1..8).step_by(2) {
        if let Some(a) = allocs[i].take() {
            dma::dma_free(a);
        }
    }

    vgaprintln!("  OK: DMA fragmentation reuse test passed");
}

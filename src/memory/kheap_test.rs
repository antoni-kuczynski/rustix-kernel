#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 27/04/2026
 */

use crate::memory::{dma, dma32, SizeUnit};
use crate::memory::ll_allocator::{LinkedListAllocator, ListNode};
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::memory::paging::{virtual_to_physical, vmm_unmap_page};
use crate::memory::pmm::{pmm_allocate_frame, pmm_free_frame};
use crate::{kprintln};
use core::alloc::Layout;
use x86_64::VirtAddr;

pub fn run_kheap_tests(allocator: &mut LinkedListAllocator) {
    kprintln!(Debug,"--- STARTING KHEAP TEST SUITE ---");

    // run_kheap_test_cases(allocator);

    test_true_heap_exhaustion(allocator);

    kprintln!(Debug,"--- KHEAP TEST SUITE COMPLETED ---\n");
}

pub fn dump_debug(allocator: &LinkedListAllocator) {
    kprintln!(Debug,"[DEBUG] Current Free List:");

    let head_addr = core::ptr::addr_of!(allocator.head) as usize;

    kprintln!(Debug,
        "  Head  : Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}",
        head_addr,
        allocator.head.size,
        head_addr + allocator.head.size
    );

    kprintln!(Debug,
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
                kprintln!(Debug,
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
                kprintln!(Debug,
                    "  Node {}: Addr: 0x{:X}, Size: {} bytes, End: 0x{:X}, Next: NULL",
                    i,
                    node_addr,
                    node.size,
                    node_end
                );
            } else {
                kprintln!(Debug,
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
        kprintln!(Debug,"  (List is empty - heap fully allocated)");
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
                kprintln!(Debug,
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
                kprintln!(Debug,
                    "  FAILED [{}]: free node end overflow: addr=0x{:X}, size={}",
                    label,
                    start,
                    node.size
                );
                return false;
            };

            if node.size < size_of::<ListNode>() {
                kprintln!(Debug,
                    "  FAILED [{}]: free node too small: addr=0x{:X}, size={}",
                    label,
                    start,
                    node.size
                );
                return false;
            }

            if start % align_of::<ListNode>() != 0 {
                kprintln!(Debug,
                    "  FAILED [{}]: free node not aligned: addr=0x{:X}",
                    label,
                    start
                );
                return false;
            }

            if end <= start {
                kprintln!(Debug,
                    "  FAILED [{}]: invalid free node range: start=0x{:X}, end=0x{:X}",
                    label,
                    start,
                    end
                );
                return false;
            }

            if previous_end != 0 && start < previous_end {
                kprintln!(Debug,
                    "  FAILED [{}]: free list is not sorted or overlaps: prev_end=0x{:X}, start=0x{:X}",
                    label,
                    previous_end,
                    start
                );
                return false;
            }

            if end == allocator.current_end {
                if allocator.top_size == 0 {
                    kprintln!(Debug,
                        "  FAILED [{}]: free node ends at heap_end, but top is empty. node=0x{:X}..0x{:X}",
                        label,
                        start,
                        end
                    );
                    return false;
                }

                if allocator.top_start != start || allocator.top_size != node.size {
                    kprintln!(Debug,
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
                    kprintln!(Debug,
                        "  FAILED [{}]: top_size does not match node.size. top_size={}, node.size={}",
                        label,
                        allocator.top_size,
                        node.size
                    );
                    return false;
                }

                if end != allocator.current_end {
                    kprintln!(Debug,
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
            kprintln!(Debug,
                "  FAILED [{}]: top_size is 0 but top_start is not 0: 0x{:X}",
                label,
                allocator.top_start
            );
            return false;
        }

        if allocator.top_size != 0 && !found_top {
            kprintln!(Debug,
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

fn test_true_heap_exhaustion(allocator: &mut LinkedListAllocator) -> bool {
    kprintln!(Debug,"[TEST] True Heap Exhaustion (OOM)");

    unsafe {
        const MAX_ALLOCS: usize = 512;
        let mut count = 0;

        let estimated_capacity = if allocator.top_size > 0 {
            allocator.top_size
        } else {
            allocator.current_end - allocator.global_start
        };

        let chunk_size = core::cmp::max(PageSize::SIZE_4KB as usize, estimated_capacity / 100);
        let chunk_size = (chunk_size + 0xFFF) & !0xFFF;

        let layout = Layout::from_size_align(chunk_size, PageSize::SIZE_4KB as usize).unwrap();

        kprintln!(Debug, "  Draining heap with chunks of {} bytes...", chunk_size);

        loop {
            let ptr = allocator.allocate(layout);

            if ptr.is_null() {
                kprintln!(Debug, "  Hit OOM perfectly after {} allocations.", count);
                break;
            }

            core::ptr::write_volatile(ptr, 0xAA);

            count += 1;
        }

        let tiny_layout = Layout::from_size_align(16, 16).unwrap();
        let tiny_ptr = allocator.allocate(tiny_layout);

        if !tiny_ptr.is_null() {
            kprintln!(Debug, "  FAILED: Hit OOM for chunks, but a tiny allocation still passed.");
            kprintln!(Debug, "  (This might mean heap fragmentation or bad chunk math, not necessarily a crash, but check it)");
            allocator.deallocate(tiny_ptr, tiny_layout);
            return false;
        }
        kprintln!(Debug, "  OK: Tiny allocation correctly rejected during OOM.");

        if !validate_allocator_state(allocator, "during OOM") {
            kprintln!(Debug, "  FAILED: Allocator state corrupted during OOM!");
            return false;
        }

        kprintln!(Debug, "  Restoring heap...");

        if !validate_allocator_state(allocator, "after OOM recovery") {
            kprintln!(Debug, "  FAILED: Allocator state corrupted after OOM recovery!");
            return false;
        }
    }

    kprintln!(Debug, "  OK: Survived true heap exhaustion and fully recovered.");
    true
}

fn validate_allocator_drained(allocator: &LinkedListAllocator, label: &str) -> bool {
    if !allocator.head.next.is_null() {
        kprintln!(Debug,
            "  FAILED [{}]: allocator still has free-list nodes after test",
            label
        );
        dump_debug(allocator);
        return false;
    }

    if allocator.top_start != 0 || allocator.top_size != 0 {
        kprintln!(Debug,
            "  FAILED [{}]: allocator still has top state after test. top_start=0x{:X}, top_size={}",
            label,
            allocator.top_start,
            allocator.top_size
        );
        dump_debug(allocator);
        return false;
    }

    if allocator.current_end != allocator.global_start {
        kprintln!(Debug,
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
    kprintln!(Debug,"\n[CASE] {}", name);

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
        kprintln!(Debug,"  FAILED: first byte read/write mismatch");
        return false;
    }

    let last = ptr.add(size - 1);
    last.write_volatile(0x5A);

    if last.read_volatile() != 0x5A {
        kprintln!(Debug,"  FAILED: last byte read/write mismatch");
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
                kprintln!(Debug,
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
                kprintln!(Debug,
                    "  FAILED: page start mismatch at offset 0x{:X}, addr=0x{:X}",
                    page_start,
                    start_ptr as usize
                );
                return false;
            }

            if end_ptr.read_volatile() != end_value {
                kprintln!(Debug,
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
    kprintln!(Debug,
        "  Allocating [{}]: size={}, align={}",
        name,
        layout.size(),
        layout.align()
    );

    let ptr = allocator.allocate(layout);

    if ptr.is_null() {
        kprintln!(Debug,"  FAILED [{}]: allocation returned NULL", name);
        return None;
    }

    let addr = ptr as usize;

    if addr % layout.align() != 0 {
        kprintln!(Debug,
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

    kprintln!(Debug,
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
    kprintln!(Debug,
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
    kprintln!(Debug,"[TEST] Basic allocation/deallocation cases");

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
    kprintln!(Debug,"[TEST] Fragmentation, reuse, and merge");

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

        kprintln!(Debug,"  Freeing even indices to create fragmentation...");

        for &i in [0usize, 2, 4, 6].iter() {
            if !dealloc_checked(allocator, "fragment even free", ptrs[i], layout) {
                return false;
            }

            ptrs[i] = core::ptr::null_mut();
        }

        dump_debug(allocator);

        kprintln!(Debug,"  Allocating smaller blocks into fragmented holes...");

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

        kprintln!(Debug,"  Reclaiming remaining allocations...");

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
    kprintln!(Debug,"[TEST] Multiple live page allocations");

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
                kprintln!(Debug,"i: {}", i);
                for j in 0..i {
                    if !ptrs[j].is_null() {
                        allocator.deallocate(ptrs[j], layouts[j]);
                    }
                }

                return false;
            };

            ptrs[i] = ptr;
        }

        kprintln!(Debug,"  Freeing multiple live allocations in reverse order...");

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
    kprintln!(Debug,"[TEST] Freeing middle allocation does not shrink heap");

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

        kprintln!(Debug,
            "  Middle candidate: B=0x{:X}..0x{:X}, heap_end=0x{:X}",
            b as usize,
            b_end,
            heap_end_before
        );

        allocator.deallocate(b, layout);

        if b_end != heap_end_before && allocator.current_end != heap_end_before {
            kprintln!(Debug,
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
    kprintln!(Debug,"[TEST] Top region shrink behavior");

    let page_size = PageSize::SIZE_4KB as usize;

    let layout = Layout::from_size_align(page_size * 8, page_size).unwrap();

    unsafe {
        let Some(ptr) = alloc_checked(allocator, "top shrink seed", layout) else {
            return false;
        };

        let alloc_start = ptr as usize;
        let alloc_end = alloc_start + layout.size();
        let heap_end_after_alloc = allocator.current_end;

        kprintln!(Debug,
            "  Seed allocation: 0x{:X}..0x{:X}, heap_end=0x{:X}",
            alloc_start,
            alloc_end,
            heap_end_after_alloc,
        );

        allocator.deallocate(ptr, layout);

        if allocator.current_end > heap_end_after_alloc {
            kprintln!(Debug,
                "  FAILED: heap_end grew during deallocation. before=0x{:X}, after=0x{:X}",
                heap_end_after_alloc,
                allocator.current_end
            );
            return false;
        }

        if !validate_allocator_state(allocator, "top shrink") {
            return false;
        }

        kprintln!(Debug,
            "  OK: after free heap_end=0x{:X}, top_start=0x{:X}, top_size={}",
            allocator.current_end,
            allocator.top_start,
            allocator.top_size
        );
    }

    true
}

fn test_allocating_heap_end_clears_top_if_possible(allocator: &mut LinkedListAllocator) -> bool {
    kprintln!(Debug,"[TEST] Allocating heap end clears top if possible");

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
            kprintln!(Debug,"  FAILED: expected a partial top after freeing suffix");
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let old_top_start = allocator.top_start;
        let old_top_end = allocator.top_start + allocator.top_size;
        let old_top_size = allocator.top_size;
        let old_heap_end = allocator.current_end;

        if old_top_end != old_heap_end {
            kprintln!(Debug,
                "  FAILED: invalid top before test. top_end=0x{:X}, heap_end=0x{:X}",
                old_top_end,
                old_heap_end
            );
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let layout = Layout::from_size_align(old_top_size, 16).unwrap();

        kprintln!(Debug,
            "  Trying end allocation: expected=0x{:X}..0x{:X}, size={}",
            old_top_start,
            old_top_end,
            old_top_size
        );

        let ptr = allocator.allocate(layout);

        if ptr.is_null() {
            kprintln!(Debug,"  FAILED: end allocation returned NULL");
            allocator.deallocate(prefix, prefix_layout);
            return false;
        }

        let got_start = ptr as usize;
        let got_end = got_start + old_top_size;

        if got_start != old_top_start || got_end != old_heap_end {
            kprintln!(Debug,
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
            kprintln!(Debug,
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

        kprintln!(Debug,"  OK: top cleared after allocating heap end");

        allocator.deallocate(ptr, layout);
        allocator.deallocate(prefix, prefix_layout);

        validate_allocator_state(allocator, "end allocation cleanup")
    }
}

fn run_kheap_test_cases(allocator: &mut LinkedListAllocator) {
    kprintln!(Debug,"\n==============================");
    kprintln!(Debug,"[TEST SUITE] LinkedListAllocator");
    kprintln!(Debug,"==============================");

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
        kprintln!(Debug,"\n[TEST SUITE] OK: all allocator tests passed");
    } else {
        kprintln!(Debug,"\n[TEST SUITE] FAILED: at least one allocator test failed");
    }
}

pub fn run_dma_tests() {
    kprintln!(Debug,"\n--- STARTING DMA TEST SUITE ---");
    test_dma_basic();
    test_dma_continuity();
    test_dma_fragmentation();
    test_dma_chunk_merge_contiguity();
    kprintln!(Debug,"--- DMA TEST SUITE COMPLETED ---\n");
}

/// K1: the contiguous grow path maps a new physically contiguous chunk and add_free_region merges
/// it with the free tail of the previous chunk. An allocation that starts in the old tail and
/// continues into the new chunk is then virtually contiguous but physically split.
///
/// Uses its own LinkedListAllocator configured like the DMA pools (bypassing the contiguity check
/// in dma.rs), placed in an unused part of the DMA32 virtual range.
/// Returns true if the allocation is physically contiguous (bug fixed), false if K1 reproduced.
fn test_dma_chunk_merge_contiguity() -> bool {
    kprintln!(Debug,"[TEST] DMA chunk merge physical contiguity (K1)");

    const TEST_VA_START: u64 = dma::DMA64_START + dma::DMA64_SIZE + 2 * 1_099_511_627_776;
    const TEST_VA_SIZE: u64 = 1_099_511_627_776;
    let page = PageSize::SIZE_4KB;

    let mut allocator = LinkedListAllocator::new();
    unsafe {
        allocator.init(TEST_VA_START as usize, TEST_VA_SIZE as usize);
    }
    allocator.set_contiguous(true);
    allocator.set_flags(
        PageTableEntry::PRESENT
            | PageTableEntry::WRITABLE
            | PageTableEntry::CACHE_DISABLE
            | PageTableEntry::WRITE_THROUGH,
    );

    let small = Layout::from_size_align(64, 64).unwrap();
    let big = Layout::from_size_align(2 * page as usize, 64).unwrap();

    unsafe {
        // 1. first chunk: one page, A takes its first 64 bytes, the rest stays as a free tail.
        //    A single frame comes from pmm_allocate_frame, so it is the lowest free frame.
        let a = allocator.allocate(small);
        assert!(!a.is_null(), "  FAIL: K1 setup allocation A failed");
        let chunk1_phys = virtual_to_physical(VirtAddr::new(TEST_VA_START)).unwrap();

        // 2. take the next lowest free frame, like the heap / page tables would in the meantime.
        //    Everything below it is used, so the next chunk lands above it - not right after chunk 1.
        let stolen = pmm_allocate_frame().expect("  FAIL: K1 could not take a frame from PMM");

        // 3. B does not fit into the 4032 byte tail -> the pool grows by a 2 page chunk, which gets
        //    merged with the tail, and B is carved out starting in the tail of chunk 1.
        let b = allocator.allocate(big);
        assert!(!b.is_null(), "  FAIL: K1 allocation B failed");

        let b_virt = VirtAddr::new(b as u64);
        let b_phys = virtual_to_physical(b_virt).unwrap();
        // with the fix, the second chunk starts after an unmapped guard page
        let guard_page_mapped = virtual_to_physical(VirtAddr::new(TEST_VA_START + page)).is_some();

        kprintln!(Debug,
            "  chunk1 phys={:#x}, stolen frame={:#x}, page after chunk1 mapped: {} (false = guard page)",
            chunk1_phys.as_u64(),
            stolen.as_u64(),
            guard_page_mapped
        );
        kprintln!(Debug,
            "  B virt={:#x}, phys={:#x}, size={}",
            b_virt.as_u64(),
            b_phys.as_u64(),
            big.size()
        );

        let mut contiguous = true;
        let b_end = b_virt.as_u64() + big.size() as u64;
        let mut page_virt = (b_virt.as_u64() & !(page - 1)) + page;
        while page_virt < b_end {
            let expected = b_phys.as_u64() + (page_virt - b_virt.as_u64());
            let actual = virtual_to_physical(VirtAddr::new(page_virt)).unwrap();
            if actual.as_u64() != expected {
                kprintln!(Debug,
                    "  page virt={:#x}: expected phys {:#x}, got {:#x}",
                    page_virt,
                    expected,
                    actual.as_u64()
                );
                contiguous = false;
            }
            page_virt += page;
        }

        // cleanup: unmap everything the test allocator mapped and give all frames back,
        // guard pages are not mapped, skip them
        let mapped_end = allocator.current_end as u64;
        let mut addr = TEST_VA_START;
        while addr < mapped_end {
            if virtual_to_physical(VirtAddr::new(addr)).is_some() {
                let frame = vmm_unmap_page(VirtAddr::new(addr));
                pmm_free_frame(frame).ok();
            }
            addr += page;
        }
        pmm_free_frame(stolen).ok();

        if contiguous {
            kprintln!(Debug,"  OK: allocation spanning two chunks is physically contiguous");
        } else {
            kprintln!(Debug,"  FAIL: K1 reproduced - DMA allocation spanning two chunks is not physically contiguous");
        }
        contiguous
    }
}

fn test_dma_basic() {
    kprintln!(Debug,"[TEST] DMA Basic Alloc/Free");
    let size = 1024 * 1024 * 128; //128mb lower this if out of memory
    let align = 64;

    if let Some(alloc) = dma::dma_alloc_coherent(size, align) {
        kprintln!(Debug,
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
        kprintln!(Debug,"  OK: Basic DMA test passed");
    } else {
        panic!("  FAIL: Basic DMA allocation failed");
    }
}

fn test_dma_continuity() {
    kprintln!(Debug,"[TEST] DMA Physical Continuity");
    let pages = 1024;
    let size = pages * 4096;

    if let Some(alloc) = dma::dma_alloc_coherent(size, 4096) {
        kprintln!(Debug,"  Allocated {} pages DMA at phys: {:?}", pages, alloc.phys);

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
        kprintln!(Debug,"  OK: DMA physical continuity verified");
    } else {
        panic!("  FAIL: DMA continuity allocation failed");
    }
}

fn test_dma_fragmentation() {
    kprintln!(Debug,"[TEST] DMA Fragmentation and Reuse");
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

    kprintln!(Debug,"  OK: DMA fragmentation reuse test passed");
}

//==================================================================================================
// DMA32
//==================================================================================================

pub fn run_dma32_tests() {
    kprintln!(Debug,"\n--- STARTING DMA32 TEST SUITE ---");
    test_dma32_basic();
    test_dma32_continuity();
    test_dma32_fragmentation();
    test_dma32_zone_routing();
    kprintln!(Debug,"--- DMA32 TEST SUITE COMPLETED ---\n");
}

fn dma32_va_range() -> (u64, u64) {
    let start = dma::DMA64_START + dma::DMA64_SIZE;
    (start, start + 4 * 1_099_511_627_776)
}

fn assert_dma32_alloc(alloc: &dma::DmaAlloc, size: usize, label: &str) {
    let (va_start, va_end) = dma32_va_range();
    assert!(
        alloc.zone == dma::DmaZone::Dma32,
        "{}: wrong zone {:?}",
        label,
        alloc.zone
    );
    assert!(
        alloc.virt.as_u64() >= va_start && alloc.virt.as_u64() + size as u64 <= va_end,
        "{}: virt {:#x} outside of the DMA32 pool",
        label,
        alloc.virt.as_u64()
    );
    assert!(
        alloc.phys.as_u64() + size as u64 <= dma32::DMA32_PHYS_LIMIT,
        "{}: phys {:#x} + {} is above 4 GiB",
        label,
        alloc.phys.as_u64(),
        size
    );
}

fn test_dma32_basic() {
    kprintln!(Debug,"[TEST] DMA32 Basic Alloc/Free");
    let size = 1024 * 1024 * 16; //16mb
    let align = 64;

    let alloc = dma32::dma32_alloc_coherent(size, align).expect("  FAIL: Basic DMA32 allocation failed");
    kprintln!(Debug,"  Allocated 16MB DMA32 at virt: {:?}, phys: {:?}", alloc.virt, alloc.phys);

    assert_dma32_alloc(&alloc, size, "DMA32 basic");
    assert!(alloc.virt.as_u64() % align as u64 == 0, "DMA32 alignment failed");

    unsafe {
        let first = alloc.virt.as_u64() as *mut u8;
        let last = (alloc.virt.as_u64() + size as u64 - 1) as *mut u8;
        first.write_volatile(0xDE);
        last.write_volatile(0xAD);
        assert!(first.read_volatile() == 0xDE, "DMA32 read/write failed (first byte)");
        assert!(last.read_volatile() == 0xAD, "DMA32 read/write failed (last byte)");
    }

    dma::dma_free(alloc);
    kprintln!(Debug,"  OK: Basic DMA32 test passed");
}

fn test_dma32_continuity() {
    kprintln!(Debug,"[TEST] DMA32 Physical Continuity");
    let pages = 1024;
    let size = pages * 4096;

    let alloc = dma32::dma32_alloc_coherent(size, 4096).expect("  FAIL: DMA32 continuity allocation failed");
    kprintln!(Debug,"  Allocated {} pages DMA32 at phys: {:?}", pages, alloc.phys);
    assert_dma32_alloc(&alloc, size, "DMA32 continuity");

    for i in 0..pages {
        let offset = i * 4096;
        let virt_page = VirtAddr::new(alloc.virt.as_u64() + offset as u64);
        let phys_page = crate::memory::paging::virtual_to_physical(virt_page).unwrap();
        let expected_phys = alloc.phys.as_u64() + offset as u64;

        if phys_page.as_u64() != expected_phys {
            dump_debug(dma32::DMA32_MANAGER.lock().allocator());
        }

        assert!(
            phys_page.as_u64() == expected_phys,
            "DMA32 continuity failed at page {}: expected {:#x}, got {:?}",
            i,
            expected_phys,
            phys_page
        );
    }

    dma::dma_free(alloc);
    kprintln!(Debug,"  OK: DMA32 physical continuity verified");
}

fn test_dma32_fragmentation() {
    kprintln!(Debug,"[TEST] DMA32 Fragmentation and Reuse");
    let size = 4096;
    let mut allocs: [Option<dma::DmaAlloc>; 8] = Default::default();

    for i in 0..8 {
        allocs[i] = dma32::dma32_alloc_coherent(size, size);
        assert!(allocs[i].is_some(), "  FAIL: DMA32 fragmentation seed allocation {} failed", i);
        assert_dma32_alloc(allocs[i].as_ref().unwrap(), size, "DMA32 fragmentation seed");
    }

    // free every second one and remember the holes
    let mut holes = [0u64; 4];
    for (hole, i) in (0..8).step_by(2).enumerate() {
        if let Some(a) = allocs[i].take() {
            holes[hole] = a.virt.as_u64();
            dma::dma_free(a);
        }
    }

    let a1 = dma32::dma32_alloc_coherent(size, size).expect("Failed to reuse DMA32 hole 1");
    let a2 = dma32::dma32_alloc_coherent(size, size).expect("Failed to reuse DMA32 hole 2");
    assert_dma32_alloc(&a1, size, "DMA32 hole 1");
    assert_dma32_alloc(&a2, size, "DMA32 hole 2");
    assert!(
        holes.contains(&a1.virt.as_u64()) && holes.contains(&a2.virt.as_u64()),
        "DMA32 holes were not reused: a1={:#x}, a2={:#x}",
        a1.virt.as_u64(),
        a2.virt.as_u64()
    );

    dma::dma_free(a1);
    dma::dma_free(a2);

    for i in (1..8).step_by(2) {
        if let Some(a) = allocs[i].take() {
            dma::dma_free(a);
        }
    }

    kprintln!(Debug,"  OK: DMA32 fragmentation reuse test passed");
}

/// Interleaves DMA64 and DMA32 allocations, checks that each lands in its own pool
/// and that freeing goes back to the right one (a freed block must be reusable by its own pool)
fn test_dma32_zone_routing() {
    kprintln!(Debug,"[TEST] DMA32 / DMA64 Zone Routing");
    let size = 4096;
    let dma64_end = dma::DMA64_START + dma::DMA64_SIZE;

    let a64 = dma::dma_alloc_coherent(size, size).expect("  FAIL: DMA64 allocation failed");
    let a32 = dma32::dma32_alloc_coherent(size, size).expect("  FAIL: DMA32 allocation failed");

    assert!(a64.zone == dma::DmaZone::Dma64, "DMA64 allocation has zone {:?}", a64.zone);
    assert!(
        a64.virt.as_u64() >= dma::DMA64_START && a64.virt.as_u64() + size as u64 <= dma64_end,
        "DMA64 virt {:#x} outside of the DMA64 pool",
        a64.virt.as_u64()
    );
    assert_dma32_alloc(&a32, size, "DMA32 routing");

    let a64_virt = a64.virt.as_u64();
    let a32_virt = a32.virt.as_u64();
    dma::dma_free(a32);
    dma::dma_free(a64);

    let b32 = dma32::dma32_alloc_coherent(size, size).expect("  FAIL: DMA32 re-allocation failed");
    let b64 = dma::dma_alloc_coherent(size, size).expect("  FAIL: DMA64 re-allocation failed");

    assert!(
        b32.virt.as_u64() == a32_virt,
        "DMA32 block was not returned to the DMA32 pool: expected {:#x}, got {:#x}",
        a32_virt,
        b32.virt.as_u64()
    );
    assert!(
        b64.virt.as_u64() == a64_virt,
        "DMA64 block was not returned to the DMA64 pool: expected {:#x}, got {:#x}",
        a64_virt,
        b64.virt.as_u64()
    );

    dma::dma_free(b32);
    dma::dma_free(b64);
    kprintln!(Debug,"  OK: DMA32 / DMA64 zone routing test passed");
}

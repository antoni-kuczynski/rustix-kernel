#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 18/05/2026
 */
use crate::memory::page_tables::PageSize;
use crate::memory::paging::vmm_map_page;
use crate::memory::pmm::{PMM_BITMAP, pmm_allocate_frame};
use crate::{kernel_main_post_stack, kprintln_ok};
use core::sync::atomic::Ordering;
use x86_64::VirtAddr;

const STACK_SIZE: u64 = 4096 * 4; // 4 pages = 16kb

pub fn switch_to_secure_stack() -> ! {
    let stack_start_addr = 0xffff_c100_0000_0000;
    let size_in_pages = STACK_SIZE / PageSize::SIZE_4KB;

    for i in 0..size_in_pages {
        let virt = VirtAddr::new(stack_start_addr + i * PageSize::SIZE_4KB);

        let frame = pmm_allocate_frame().expect("No physical frames available for secure stack!");

        unsafe {
            vmm_map_page(virt, frame, &PageSize::Size4Kb);
        }
    }

    let new_stack_top = stack_start_addr + size_in_pages * PageSize::SIZE_4KB;
    
    //switch to the new stack
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack_top}",
            "call {next_stage}",
            stack_top = in(reg) new_stack_top,
            next_stage = sym secure_stack_call,
            options(noreturn)
        )
    }
}

fn secure_stack_call() {
    kprintln_ok!("Switched to secure stack.");
    kernel_main_post_stack();
}

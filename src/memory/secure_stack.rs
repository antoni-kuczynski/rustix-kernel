use x86_64::VirtAddr;
use crate::memory::paging::vmm_map_page;
use crate::memory::pmm::pmm_allocate_frame;
use crate::memory::page_tables::PageSize;
use crate::{kernel_main_post_stack, vgaprintln};

/// Switches to secure stack, with overflow protection
pub unsafe fn switch_to_secure_stack(
    stack_base_addr: VirtAddr,
    size_in_pages: u64,
) -> ! {
    // first page is a guard page
    let stack_start_addr = stack_base_addr.as_u64() + PageSize::SIZE_4KB;

    for i in 0..size_in_pages {
        let virt = VirtAddr::new(stack_start_addr + i * PageSize::SIZE_4KB);

        let frame = pmm_allocate_frame()
            .expect("No physical frames available for secure stack!");

        unsafe {
            vmm_map_page(virt, frame, &PageSize::Size4Kb);
        }
    }

    let new_stack_top = stack_start_addr + size_in_pages * PageSize::SIZE_4KB;

    vgaprintln!("Switching to secure stack at {:#x}...", new_stack_top);

    //switch to the new stack
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack_top}",
            "call {next_stage}",
            stack_top = in(reg) new_stack_top,
            next_stage = sym kernel_main_post_stack,
            options(noreturn)
        )
    }
}

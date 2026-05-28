#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
use crate::ColorTextMode;
use crate::VGAWRITER;
use core::sync::atomic::Ordering::Acquire;
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
//==================================================================================================
// This is a tem heap region to store early page tables (Early bump allocator)
// It's been already mapped during early init, so dont care about that
//==================================================================================================
use crate::memory::page_tables::PagingSetupError;
use crate::memory::{_P2V_kernel, MemoryRange};
use crate::{earlyHeapEnd, earlyHeapStart, memory, print_ok_msg, vgaprint, vgaprintln};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PageTable;
//==================================================================================================
pub struct EarlyBumpAllocator {
    temp_range: MemoryRange,
    temp_ptr: AtomicPtr<u8>,
}
//==================================================================================================
impl EarlyBumpAllocator {
    fn empty() -> Self {
        Self {
            temp_range: MemoryRange { start: 0, end: 0 },
            temp_ptr: Default::default(),
        }
    }

    unsafe fn kmalloc_early<T>(&self, size: usize, align: usize) -> Option<*mut T> {
        let align_u64 = if align == 0 { 1 } else { align as u64 };
        let mut current_ptr = self.temp_ptr.load(Acquire);

        loop {
            let aligned_ptr =
                ((current_ptr.add((align_u64 - 1) as usize)) as u64 & !(align_u64 - 1)) as *mut u8;
            let next_ptr = aligned_ptr.add(size);

            if next_ptr as u64 > self.temp_range.end {
                return None;
            }

            match self.temp_ptr.compare_exchange_weak(
                current_ptr,
                next_ptr,
                Ordering::SeqCst,
                Acquire,
            ) {
                Ok(_) => {
                    return Some(aligned_ptr as *mut T);
                }
                Err(actual_ptr) => {
                    current_ptr = actual_ptr;
                }
            }
        }
    }
}
//==================================================================================================

//TODO: remove this garbage temp code :)
pub unsafe fn print_page_table_tree(phys_mem_offset: u64) {
    unsafe {
        let (level_4_table_frame, _) = Cr3::read();
        let phys_addr = level_4_table_frame.start_address();

        let virt_addr = phys_mem_offset + phys_addr.as_u64();
        let pml4_table = &*(virt_addr as *const PageTable);

        vgaprintln!("PML4 (L4) Table at: {:?}", phys_addr);

        for (i, entry) in pml4_table.iter().enumerate() {
            if !entry.is_unused() {
                vgaprintln!("  L4 Entry {}: {:?}", i, entry);

                let pdpt_phys = entry.addr();
                let pdpt_virt = phys_mem_offset + pdpt_phys.as_u64();
                let pdpt_table = &*(pdpt_virt as *const PageTable);

                for (j, entry_l3) in pdpt_table.iter().enumerate() {
                    if !entry_l3.is_unused() {
                        vgaprintln!("    L3 Entry {}: {:?}", j, entry_l3);

                        if entry_l3
                            .flags()
                            .contains(x86_64::structures::paging::PageTableFlags::HUGE_PAGE)
                        {
                            vgaprintln!("      [1GB Huge Page]");
                            continue;
                        }

                        let pd_phys = entry_l3.addr();
                        let pd_virt = phys_mem_offset + pd_phys.as_u64();
                        let pd_table = &*(pd_virt as *const PageTable);

                        for (k, entry_l2) in pd_table.iter().enumerate() {
                            if !entry_l2.is_unused() {
                                vgaprintln!("      L2 Entry {}: {:?}", k, entry_l2);

                                if entry_l2
                                    .flags()
                                    .contains(x86_64::structures::paging::PageTableFlags::HUGE_PAGE)
                                {
                                    vgaprintln!("        [2MB Huge Page]");
                                    continue;
                                }

                                let pt_phys = entry_l2.addr();
                                let pt_virt = phys_mem_offset + pt_phys.as_u64();
                                let pt_table = &*(pt_virt as *const PageTable);

                                for (l, entry_l1) in pt_table.iter().enumerate() {
                                    if !entry_l1.is_unused() {
                                        vgaprintln!("        L1 Entry {}: {:?}", l, entry_l1);
                                    } //WHYYYYYYYY
                                } //AREEEE
                            } //THEEERE
                        } //SOOOO
                    } //MANYYYYYYYYY?!?!?!?!?!??!
                } //I CANT SEE THE END!!!!! :(((((((
            } //SOMEONEEEEEEEEEE PLEEEEASE HEEEEEEEEEEEEEEEEEEELP!!!
        } //AREE WEE DOOOONE?!?!?!?!?!
    } //THEY ARE STILL GOING  AKLSHJDLKASDJLKASDUOHWEUIFDHXCV,NMHOUW;EF793EE :(((((((((((((((
} //I THINK THIS'S THE LAST ONE
//finally.

pub fn eba_init() {
    vgaprint!("Initializing early bumb allocator...");
    let start = _P2V_kernel(unsafe { earlyHeapStart });
    let end = _P2V_kernel(unsafe { earlyHeapEnd });

    let mut eba = EARLY_BUMP_ALLOCATOR.lock();
    eba.temp_range = MemoryRange::new(start, end);
    eba.temp_ptr = AtomicPtr::new(start as *mut u8);
    print_ok_msg!();
}

lazy_static! {
    pub static ref EARLY_BUMP_ALLOCATOR: Mutex<EarlyBumpAllocator> =
        Mutex::new(EarlyBumpAllocator::empty());
}

/// Early kmalloc for early bump allocator region
pub unsafe fn eba_kmalloc<T>(size: usize, align: usize) -> Option<*mut T> {
    unsafe { EARLY_BUMP_ALLOCATOR.lock().kmalloc_early(size, align) }
}

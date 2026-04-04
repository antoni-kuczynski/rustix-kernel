//==================================================================================================
// This is a tem heap region to store early page tables
// It's been already mapped during early init, so dont care about that
//==================================================================================================
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PageTable;
use crate::{vgaprintln};
use crate::memory::{MemoryRange};
use crate::memory::paging::PagingSetupError;
//==================================================================================================
pub struct EarlyHeap {
    temp_range: MemoryRange,
    temp_ptr: *mut u64
}
//==================================================================================================
impl EarlyHeap {
    pub fn kmalloc_early<T>(&mut self, size: usize, align: usize) -> Option<*mut T> {
        unsafe {
            let ptr = if align == 0 {
                self.temp_ptr as *mut T
            } else {
                (self.temp_ptr.byte_add(align) as u64 & !(align - 1) as u64) as *mut T
            };

            //region full
            if ptr.byte_add(size) as u64 > self.temp_range.end {
                return None;
            }

            self.temp_ptr = self.temp_ptr.byte_add(size);

            Some(ptr)
        }
    }
}
//==================================================================================================
pub fn init(memory_range: MemoryRange) -> Result<(EarlyHeap), PagingSetupError> {
    unsafe {
        let ptr_start = memory_range.start;

        let view = EarlyHeap {
            temp_range: memory_range,
            temp_ptr: ptr_start as *mut u64
        };

        Ok(view)
    }
}

//TODO: remove this garbage temp code :)
pub unsafe fn print_page_table_tree(phys_mem_offset: u64) {
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

                    if entry_l3.flags().contains(x86_64::structures::paging::PageTableFlags::HUGE_PAGE) {
                        vgaprintln!("      [1GB Huge Page]");
                        continue;
                    }

                    let pd_phys = entry_l3.addr();
                    let pd_virt = phys_mem_offset + pd_phys.as_u64();
                    let pd_table = &*(pd_virt as *const PageTable);

                    for (k, entry_l2) in pd_table.iter().enumerate() {
                        if !entry_l2.is_unused() {
                            vgaprintln!("      L2 Entry {}: {:?}", k, entry_l2);

                            if entry_l2.flags().contains(x86_64::structures::paging::PageTableFlags::HUGE_PAGE) {
                                vgaprintln!("        [2MB Huge Page]");
                                continue;
                            }

                            let pt_phys = entry_l2.addr();
                            let pt_virt = phys_mem_offset + pt_phys.as_u64();
                            let pt_table = &*(pt_virt as *const PageTable);

                            for (l, entry_l1) in pt_table.iter().enumerate() {
                                if !entry_l1.is_unused() {
                                    vgaprintln!("        L1 Entry {}: {:?}", l, entry_l1);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
/*
 * Created by Antoni Kuczyński
 * 13/02/2026
 */
use core::ptr;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::multiboot::MultibootInfoView;
use crate::memory::{flush_tlb_single_page, Cr3, SizeUnit, P2V, V2P};
use crate::memory::eba::eba_kmalloc;
use crate::memory::page_tables::{PageIndexes, PageSize, PageTable, PageTableEntry};
use crate::vgaprintln;

/// Maps page and uses early bump as allocator
/// Used for building early paging structure
pub unsafe fn eba_map_2mb_page(virt: VirtAddr, phys: PhysAddr) {
    unsafe {
        let indexes = PageIndexes::get_from_virt(virt);
        let pml4 = PageTable::from_cr3();
        let pdpt3 = (*pml4).get_ptr_from_index_or_eba_kmalloc(indexes.pml4_index());
        let pd2 = (*pdpt3).get_ptr_from_index_or_eba_kmalloc(indexes.pdpt_index());
    
        let mut entry = &mut (*pd2).get_entries()[indexes.pd_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_FFE0_0000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::HUGE, true);
         flush_tlb_single_page(virt);
    }
}


/// Allocates a continuous page range using early bump as an allocator.
pub unsafe fn eba_map_2mb_range(virt_start: VirtAddr, virt_end: VirtAddr, phys: PhysAddr) {
    unsafe {
        let mut temp_virt = virt_start.as_u64();
        let mut temp_phys = phys.as_u64();
        while temp_virt <= virt_end.as_u64() {
            eba_map_2mb_page(
                VirtAddr::new_truncate(temp_virt),
                PhysAddr::new_truncate(V2P(temp_phys))
            );
            temp_virt += 0x200000;
            temp_phys += 0x200000;
        }
    }
}


/// Unamps a 2mb page - it doesn't free the page table, as it does nothing on a bump allocator!
pub unsafe fn early_unmap_2mb_page(virt: VirtAddr) {
    unsafe {
        let indexes = PageIndexes::get_from_virt(virt);
        let pml4 = PageTable::from_cr3();
        let pdpt3 = (*pml4).get_ptr_from_index_or_eba_kmalloc(indexes.pml4_index());
        let pd2 = (*pdpt3).get_ptr_from_index_or_eba_kmalloc(indexes.pdpt_index());
    
        let mut entry = &mut (*pd2).get_entries()[indexes.pd_index()];
        entry.set_flag(PageTableEntry::PRESENT, false);
         flush_tlb_single_page(virt); 
    }
}


/// Frees a continuous page range using early bump as an allocator.
pub unsafe fn early_unmap_2mb_range(virt_start: VirtAddr, virt_end: VirtAddr) {
    unsafe {
        let mut temp = virt_start.as_u64();
        while temp <= virt_end.as_u64() {
            early_unmap_2mb_page(
                VirtAddr::new_truncate(virt_start.as_u64() + temp),
            );
            temp += 0x200000;
        }
    }
}

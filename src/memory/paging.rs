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
pub fn eba_map_2mb_range(virt_start: VirtAddr, phys_start: PhysAddr, length: u64) {
    unsafe {
        let mut mapped_bytes = 0;
        let mut current_virt = virt_start.as_u64();
        let mut current_phys = phys_start.as_u64();

        while mapped_bytes < length {
            eba_map_2mb_page(
                VirtAddr::new_truncate(current_virt),
                PhysAddr::new_truncate(current_phys)
            );

            current_virt += 0x200000;
            current_phys += 0x200000;
            mapped_bytes += 0x200000;
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
pub fn early_unmap_2mb_range(virt_start: VirtAddr, length: u64) {
    unsafe {
        let mut unmapped_bytes = 0;
        let mut current_virt = virt_start.as_u64();

        while unmapped_bytes < length {
            early_unmap_2mb_page(
                VirtAddr::new_truncate(current_virt)
            );

            current_virt += 0x200000;
            unmapped_bytes += 0x200000;
        }
    }
}


// Translates virtual address to a physical address
// pub fn virtual_to_physical(virt: VirtAddr) -> Option<PhysAddr> {
//     unsafe {
//         let indexes = PageIndexes::get_from_virt(virt);
//
//         let get_entry = |table_ptr: *mut PageTable, index| {
//             let entry: &PageTableEntry = &(*table_ptr).get_entries()[index];
//             if entry.is_present() { Some(entry) } else { None }
//         };
//
//         let calc_phys = |entry_addr: u64, mask: u64| {
//             let offset = virt.as_u64() & mask;
//             PhysAddr::new(entry_addr + offset)
//         };
//
//         let pml4 = PageTable::from_cr3();
//         let pml4_entry = get_entry(pml4, indexes.pml4_index())?;
//
//         let pdpt_entry = get_entry(pml4_entry.as_pt_address(), indexes.pdpt_index())?;
//         if pdpt_entry.is_huge() {
//             return Some(calc_phys(pdpt_entry.address(), 0x3FFF_FFFF));
//         }
//
//         let pd_entry = get_entry(pdpt_entry.as_pt_address(), indexes.pd_index())?;
//         if pd_entry.is_huge() {
//             return Some(calc_phys(pd_entry.address(), 0x1F_FFFF));
//         }
//
//         let pt_entry = get_entry(pd_entry.as_pt_address(), indexes.pt_index())?;
//
//         Some(calc_phys(pt_entry.address(), 0xFFF))
//     }
// }

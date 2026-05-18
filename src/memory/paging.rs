#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 13/02/2026
 */
use core::cmp::PartialEq;
use core::ops::AddAssign;
use core::ptr;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::multiboot::MultibootInfoView;
use crate::memory::{flush_tlb_single_page, Cr3, SizeUnit, _P2V_kernel, _V2P_kernel};
use crate::memory::dir_mapping::physical_to_virtual;
use crate::memory::eba::eba_kmalloc;
use crate::memory::page_tables::{PageIndexes, PageSize, PageTable, PageTableEntry};
use crate::memory::pmm::{pmm_free_range, pmm_is_enabled, pmm_reserve_range};
use crate::vgaprintln;

/// Maps page and uses early bump as allocator
/// Used for building early paging structure
pub unsafe fn vmm_eba_map_page(virt: VirtAddr, phys: PhysAddr, page_size: &PageSize, alloc_in_pmm: bool) {
    let indexes = PageIndexes::get_from_virt(virt);
    let pml4 = PageTable::from_cr3();
    let pdpt3 = (*pml4).get_ptr_from_index_or_eba_kmalloc(indexes.pml4_index());

    if let PageSize::Size1Gb = page_size {
        let mut entry = &mut (*pdpt3).get_entries()[indexes.pdpt_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_C000_0000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::HUGE, true);
        flush_tlb_single_page(virt);
        if alloc_in_pmm {
            pmm_reserve_range(phys, page_size.as_u64());
        }
        return;
    }

    let pd2 = (*pdpt3).get_ptr_from_index_or_eba_kmalloc(indexes.pdpt_index());

    if let PageSize::Size2Mb = page_size {
        let mut entry = &mut (*pd2).get_entries()[indexes.pd_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_FFE0_0000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::HUGE, true);
        flush_tlb_single_page(virt);
        if alloc_in_pmm {
            pmm_reserve_range(phys, page_size.as_u64());
        }
        return;
    }

    let pt1 = (*pd2).get_ptr_from_index_or_eba_kmalloc(indexes.pd_index());

    if let PageSize::Size4Kb = page_size {
        let mut entry = &mut (*pt1).get_entries()[indexes.pt_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_FFFF_F000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::HUGE, false);
        flush_tlb_single_page(virt);
        if alloc_in_pmm {
            pmm_reserve_range(phys, page_size.as_u64());
        }
    }
}

/// Allocates a continuous page range using early bump as an allocator.
pub unsafe fn vmm_eba_map_range(virt_start: VirtAddr, phys_start: PhysAddr, length: u64, page_size: &PageSize, alloc_in_pmm: bool) {
    let mut mapped_bytes = 0;
    let mut current_virt = virt_start.as_u64();
    let mut current_phys = phys_start.as_u64();

    let step = match page_size {
        PageSize::Size1Gb => 0x4000_0000,
        PageSize::Size2Mb => 0x20_0000,
        PageSize::Size4Kb => 0x1000,
    };

    while mapped_bytes < length {
        vmm_eba_map_page(
            VirtAddr::new_truncate(current_virt),
            PhysAddr::new_truncate(current_phys),
            page_size,
            alloc_in_pmm
        );

        current_virt += step;
        current_phys += step;
        mapped_bytes += step;
    }
}

/// Unamps a 2mb page - it doesn't free the page table, as it does nothing on a bump allocator!
pub unsafe fn vmm_early_unmap_page(virt: VirtAddr, phys: PhysAddr, page_size: &PageSize, free_in_pmm: bool) {
    let indexes = PageIndexes::get_from_virt(virt);
    let pml4 = PageTable::from_cr3();
    let pdpt3 = (*pml4).get_ptr_from_index_or_eba_kmalloc(indexes.pml4_index());

    if let PageSize::Size1Gb = page_size {
        let mut entry = &mut (*pdpt3).get_entries()[indexes.pdpt_index()];
        entry.set_flag(PageTableEntry::PRESENT, false);
        flush_tlb_single_page(virt);
        if free_in_pmm {
            pmm_free_range(phys, page_size.as_u64());
        }
        return;
    }

    let pd2 = (*pdpt3).get_ptr_from_index_or_eba_kmalloc(indexes.pdpt_index());

    if let PageSize::Size2Mb = page_size {
        let mut entry = &mut (*pd2).get_entries()[indexes.pd_index()];
        entry.set_flag(PageTableEntry::PRESENT, false);
        flush_tlb_single_page(virt);
        if free_in_pmm {
            pmm_free_range(phys, page_size.as_u64());
        }
        return;
    }

    let pt1 = (*pd2).get_ptr_from_index_or_eba_kmalloc(indexes.pd_index());

    if let PageSize::Size4Kb = page_size {
        let mut entry = &mut (*pt1).get_entries()[indexes.pt_index()];
        entry.set_flag(PageTableEntry::PRESENT, false);
        flush_tlb_single_page(virt);
        if free_in_pmm {
            pmm_free_range(phys, page_size.as_u64());
        }
    }
}

/// Frees a continuous page range using early bump as an allocator.
pub unsafe fn early_unmap_range(virt_start: VirtAddr, phys: PhysAddr, length: u64, page_size: &PageSize, free_in_pmm: bool) {
    let mut unmapped_bytes = 0;
    let mut current_virt = virt_start.as_u64();
    let mut current_phys = phys.as_u64();

    let step = match page_size {
        PageSize::Size1Gb => 0x4000_0000,
        PageSize::Size2Mb => 0x20_0000,
        PageSize::Size4Kb => 0x1000,
    };

    while unmapped_bytes < length {
        vmm_early_unmap_page(
            VirtAddr::new_truncate(current_virt),
            PhysAddr::new_truncate(current_phys),
            page_size,
            free_in_pmm
        );

        current_virt += step;
        current_phys += step;
        unmapped_bytes += step;
    }
}

/// Maps page and uses PMM for allocating page tables if needed
pub unsafe fn vmm_map_page(virt: VirtAddr, phys: PhysAddr, page_size: &PageSize) {
    let indexes = PageIndexes::get_from_virt(virt);
    let pml4 = PageTable::from_cr3();
    let pdpt3 = (*pml4).get_ptr_from_index_or_alloc(indexes.pml4_index());

    if let PageSize::Size1Gb = page_size {
        let mut entry = &mut (*pdpt3).get_entries()[indexes.pdpt_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_C000_0000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::WRITABLE, true);
        entry.set_flag(PageTableEntry::HUGE, true);
        flush_tlb_single_page(virt);
        return;
    }

    let pd2 = (*pdpt3).get_ptr_from_index_or_alloc(indexes.pdpt_index());

    if let PageSize::Size2Mb = page_size {
        let mut entry = &mut (*pd2).get_entries()[indexes.pd_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_FFE0_0000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::WRITABLE, true);
        entry.set_flag(PageTableEntry::HUGE, true);
        flush_tlb_single_page(virt);
        return;
    }

    let pt1 = (*pd2).get_ptr_from_index_or_alloc(indexes.pd_index());

    if let PageSize::Size4Kb = page_size {
        let mut entry = &mut (*pt1).get_entries()[indexes.pt_index()];
        entry.set_address(phys.as_u64() & 0x000F_FFFF_FFFF_F000);
        entry.set_flag(PageTableEntry::PRESENT, true);
        entry.set_flag(PageTableEntry::WRITABLE, true);
        entry.set_flag(PageTableEntry::HUGE, false);
        flush_tlb_single_page(virt);
    }
}

/// Allocates a continuous page range using PMM for page tables.
pub unsafe fn vmm_map_range(virt_start: VirtAddr, phys_start: PhysAddr, length: u64, page_size: &PageSize) {
    let mut mapped_bytes = 0;
    let mut current_virt = virt_start.as_u64();
    let mut current_phys = phys_start.as_u64();

    let step = page_size.as_u64();

    while mapped_bytes < length {
        vmm_map_page(
            VirtAddr::new_truncate(current_virt),
            PhysAddr::new_truncate(current_phys),
            page_size
        );

        current_virt += step;
        current_phys += step;
        mapped_bytes += step;
    }
}

/// Unmaps a page
/// Returns it's physical frame address
pub unsafe fn vmm_unmap_page(virt: VirtAddr) -> PhysAddr {
    let indexes = PageIndexes::get_from_virt(virt);
    let pml4 = PageTable::from_cr3();

    let get_entry = |table_ptr: *mut PageTable, index| {
        let entry: &mut PageTableEntry = &mut (*table_ptr).get_entries()[index];
        if entry.is_present() { Some(entry) } else { None }
    };

    let pml4_entry = get_entry(pml4, indexes.pml4_index()).expect("PML4 entry not present during unmap");

    let pdpt_ptr = physical_to_virtual(PhysAddr::new(pml4_entry.address())).as_u64() as *mut PageTable;
    let pdpt_entry = get_entry(pdpt_ptr, indexes.pdpt_index()).expect("PDPT entry not present during unmap");
    if pdpt_entry.is_huge() {
        let phys_addr = PhysAddr::new(pdpt_entry.address());
        pdpt_entry.set_flag(PageTableEntry::PRESENT, false);
        flush_tlb_single_page(virt);
        return phys_addr;
    }

    let pd_ptr = physical_to_virtual(PhysAddr::new(pdpt_entry.address())).as_u64() as *mut PageTable;
    let pd_entry = get_entry(pd_ptr, indexes.pd_index()).expect("PD entry not present during unmap");
    if pd_entry.is_huge() {
        let phys_addr = PhysAddr::new(pd_entry.address());
        pd_entry.set_flag(PageTableEntry::PRESENT, false);
        flush_tlb_single_page(virt);
        return phys_addr;
    }

    let pt_ptr = physical_to_virtual(PhysAddr::new(pd_entry.address())).as_u64() as *mut PageTable;
    let pt_entry = get_entry(pt_ptr, indexes.pt_index()).expect("PT entry not present during unmap");
    let phys_addr = PhysAddr::new(pt_entry.address());
    pt_entry.set_flag(PageTableEntry::PRESENT, false);
    flush_tlb_single_page(virt);
    phys_addr
}

/// Unmaps a continuous page range.
pub unsafe fn vmm_unmap_range(virt_start: VirtAddr, length: u64, page_size: &PageSize) {
    let mut unmapped_bytes = 0;
    let mut current_virt = virt_start.as_u64();

    let step = page_size.as_u64();

    while unmapped_bytes < length {
        vmm_unmap_page(VirtAddr::new_truncate(current_virt));
        current_virt += step;
        unmapped_bytes += step;
    }
}

/// Translates virtual address to a physical address
pub fn virtual_to_physical(virt: VirtAddr) -> Option<PhysAddr> {
    let indexes = PageIndexes::get_from_virt(virt);

    let get_entry = |table_ptr: *mut PageTable, index| {
        let entry: &PageTableEntry = unsafe { &(*table_ptr).get_entries()[index] };
        if entry.is_present() { Some(entry) } else { None }
    };

    let phys_to_table_ptr = |phys_addr: u64| {
        physical_to_virtual(PhysAddr::new(phys_addr)).as_u64() as *mut PageTable
    };

    let calc_phys = |entry_addr: u64, mask: u64| {
        let offset = virt.as_u64() & mask;
        PhysAddr::new(entry_addr + offset)
    };

    let pml4 = PageTable::from_cr3();
    let pml4_entry = get_entry(pml4, indexes.pml4_index())?;

    let pdpt_ptr = phys_to_table_ptr(pml4_entry.as_pt_address() as u64);
    let pdpt_entry = get_entry(pdpt_ptr, indexes.pdpt_index())?;
    if pdpt_entry.is_huge() {
        return Some(calc_phys(pdpt_entry.address(), 0x3FFF_FFFF));
    }

    let pd_ptr = phys_to_table_ptr(pdpt_entry.as_pt_address() as u64);
    let pd_entry = get_entry(pd_ptr, indexes.pd_index())?;
    if pd_entry.is_huge() {
        return Some(calc_phys(pd_entry.address(), 0x1F_FFFF));
    }

    let pt_ptr = phys_to_table_ptr(pd_entry.as_pt_address() as u64);
    let pt_entry = get_entry(pt_ptr, indexes.pt_index())?;

    Some(calc_phys(pt_entry.address(), 0xFFF))
}

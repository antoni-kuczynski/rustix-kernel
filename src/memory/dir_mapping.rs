#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 15/04/2026
 */
use crate::{print_fail_msg, vgaprintln, VGAWRITER};
use crate::ColorTextMode;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::cpuid::CpuId;
use crate::boot::multiboot::{multiboot2_memory_map_tag, MULTIBOOT_INFO};
use crate::memory::SizeUnit;
use crate::{print_ok_msg, vgaprint};
use crate::memory::page_tables::PageSize;
use crate::memory::paging::{eba_map_page};

const DIR_MAP_TOTAL_SIZE: u64 = 64 * 1_099_511_627_776; //64 terabytes
const DIR_MAP_START: VirtAddr = VirtAddr::new(0xffff_8080_0000_0000);
const DIR_MAP_END: VirtAddr = VirtAddr::new(0xffff_e080_0000_0000);
const PHYS_MEMORY_OFFSET: u64 = DIR_MAP_START.as_u64();

unsafe fn do_4kb_pages(total: u64, mut mapped: u64) -> u64 {
    while mapped  <= total - PageSize::SIZE_4KB {
        eba_map_page(
            VirtAddr::new(PHYS_MEMORY_OFFSET + mapped),
            PhysAddr::new(mapped),
            &PageSize::Size4Kb
        );
        mapped += PageSize::SIZE_4KB;
    }
    mapped
}

unsafe fn do_2mb_pages(total: u64, mut mapped: u64) -> u64 {
    while mapped  <= total - PageSize::SIZE_2MB {
        eba_map_page(
            VirtAddr::new(PHYS_MEMORY_OFFSET + mapped),
            PhysAddr::new(mapped),
            &PageSize::Size2Mb
        );
        mapped += PageSize::SIZE_2MB;
    }
    mapped
}

unsafe fn do_1gb_pages(total: u64, mut mapped: u64) -> u64 {
    while mapped  <= total - PageSize::SIZE_1GB {
        eba_map_page(
            VirtAddr::new(PHYS_MEMORY_OFFSET + mapped),
            PhysAddr::new(mapped),
            &PageSize::Size1Gb
        );
        mapped += PageSize::SIZE_1GB;
    }
    mapped
}


// If using 2mb pages, cap the memory to like 1tb and use +/- 5mb of page tables
fn init_2mb(high_addr: PhysAddr) -> u64 {
    vgaprint!("Initializing direct mapping using 2mb pages...");

    if high_addr.as_u64() > SizeUnit::Terabyte.as_u64() {
        print_fail_msg!();
        panic!("Memory size > 1tb - yeah thats a little too much memory for me :(((((");
    }

    let total = high_addr.as_u64();
    let mut mapped = 0u64;

    unsafe {
        mapped = do_2mb_pages(total, mapped);
        mapped = do_4kb_pages(total, mapped);
    }

    print_ok_msg!();
    mapped
}

// If using 1gb pages, we can easily map all 64tb usable memory in just 0,5mb of page tables
fn init_1gb(high_addr: PhysAddr) -> u64 {
    vgaprint!("Initializing direct mapping using 1gb pages...");

    if high_addr.as_u64() > DIR_MAP_TOTAL_SIZE {
        print_fail_msg!();
        panic!("Memory size > 64tb - yeah thats a little too much memory for me :(((((");
    }

    let total = high_addr.as_u64();
    let mut mapped = 0u64;

    unsafe {
        mapped = do_1gb_pages(total, mapped);
        mapped = do_2mb_pages(total, mapped);
        mapped = do_4kb_pages(total, mapped);
    }

    print_ok_msg!();
    mapped
}

pub fn dir_mapping_init() {
    unsafe {
        let high_addr = (*multiboot2_memory_map_tag().expect("no memory map tag provided!"))
            .get_high_usable_memory_address();

        let has_1gb_pages = CpuId::has_pdpe1gb();
        if has_1gb_pages {
            init_1gb(high_addr);
        } else {
            let mapped_bytes = init_2mb(high_addr); // old CPUs don't support 1gb pages, so use 2mb ones instead
            vgaprintln!("High addr: {}", high_addr.as_u64());
            vgaprintln!("Mapped bytes: {}", mapped_bytes);
        }
    }
}

pub fn physical_to_virtual(phys: PhysAddr) -> VirtAddr {
    VirtAddr::new(phys.as_u64() + PHYS_MEMORY_OFFSET)
}

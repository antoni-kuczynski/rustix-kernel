#![allow(dead_code)]
/*
 *  Created by Oskar Przybylski 
 *  28/09/2025
 *
 * Virtual adress for 4 level paging has following format:
 * Bits     Meaning
 * 0-11     Page Offset (offset in frame)
 * 12-20    Level 1 Page Table Index
 * 21-29    Level 2 Page Table Index
 * 30-38    Level 3 Page Table Index
 * 39-47    Level 4 Page Table Index
 * 48-64    Discarded (sign-extension)
 *
 * Level 4 Page Table (highest) adress is stored in CR3 register
 * Each page and page table is size of 4 KiB.
 * Each page table has 512 entries with size of 8 bytes with following format:
 * Bits     Name                Meaning
 * 0        present             this page is currently in memory
 * 1        writable            is it allowed to write on this page
 * 2        user-access         can user access this page
 * 3        cache-write         writes go directly to memory
 * 4        disable-cache       no cache is used for this page
 * 5        accessed            set when CPU uses page
 * 6        dirty               set when CPU writes to this page 
 * 7        huge page           must be 0 in P1 and P4, 1GiB page if in P3 and 2MiB page if in P2
 * 8        global              page isn’t flushed from caches on address space switch (PGE bit of CR4 register must be set) 
 * 9-11     available           free for OS to use 
 * 12-51    physical adress     pointer to next page table or frame in P1 
 * 52-62    available           free for OS to use 
 * 63       no execute          forbids executing code contained in this page
 *
 * Note: physical adress is not 64 bits because we always point to a 4096-byte aligned address, 
 * either to a page-aligned page table or to the start of a mapped frame. 
 * This means that bits 0–11 are always zero, so there is no reason to store these bits 
 * because the hardware can just set them to zero before using the address.  
 *
 * Page table 4 location is set in Cr3 register
 */

use bootloader::BootInfo;
use spin::Once;
use x86_64::{registers::control::Cr3, structures::paging::{OffsetPageTable, PageTable,  Translate}, PhysAddr, VirtAddr};


static PHYSICAL_MEMORY_OFFSET: Once<VirtAddr> = Once::new();

pub fn init(boot_info: &'static BootInfo) -> OffsetPageTable<'static>{
    init_offset(&boot_info);
    init_mapper()
}

// translates virtual adress to physical if mapped
pub fn v_to_p(v: VirtAddr, mapper: OffsetPageTable) -> Option<PhysAddr>{
    mapper.translate_addr(v)
}

// translates physical adress to virtual
pub fn p_to_v(p: PhysAddr) -> VirtAddr{
    get_physical_memory_offset() + p.as_u64()
}

fn init_offset(boot_info: &'static BootInfo){
    PHYSICAL_MEMORY_OFFSET.call_once(
        || VirtAddr::new(boot_info.physical_memory_offset));
}

fn init_mapper() -> OffsetPageTable<'static> {
    let page_table4            = get_active_page_table4();
    let physical_memory_offset = get_physical_memory_offset();
    unsafe{
        OffsetPageTable::new(page_table4,physical_memory_offset)
    }
}

fn get_physical_memory_offset() -> VirtAddr{
    match PHYSICAL_MEMORY_OFFSET.get(){
        Some(&offset) => offset,
        None          => panic!("Calling \"get_physical_memory_offset\" before initialization! "),
    }
}

fn get_active_page_table4() -> &'static mut PageTable{
    let (page_table4_frame,_) = Cr3::read();    // read physical adress of page table 4 
    let phys = page_table4_frame.start_address();

    let virt = p_to_v(phys); // convert to virtual adress

    let page_table4 : *mut PageTable = virt.as_mut_ptr(); // interpret as pointer

    return unsafe {&mut *page_table4};
}


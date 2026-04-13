/*
 * Created by Antoni Kuczyński
 * 13/02/2026
 */
use core::ptr;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::multiboot::MultibootInfoView;
use crate::memory::{Cr3, P2V, PAGE_SIZE, V2P};
use crate::memory::eba::eba_kmalloc;
use crate::memory::paging::PagingSetupError::NoMemoryMapProvided;
use crate::vgaprintln;

//==================================================================================================
//  PAGE TABLE
//==================================================================================================
const PAGE_TABLE_ENTRIES: usize = 512;

#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRIES]
}

impl PageTable {
    fn get_ptr_from_index_or_eba_kmalloc(&mut self, index: usize) -> *mut PageTable {
        let mut entry = &mut self.entries[index];

        vgaprintln!("{:#011x}", entry as *const PageTableEntry as *const u64 as u64);
        // no page table - need to allocate
        if !entry.is_present() {
            let new_table = early_page_alloc();
            entry.set_address(V2P(new_table as u64));
            entry.set_flag(PageTableEntry::PRESENT, true);
            entry.set_flag(PageTableEntry::WRITABLE, true);
            return new_table;
        }
        P2V(entry.address()) as *mut PageTable
    }

    fn from_cr3() -> *mut PageTable {
        P2V(Cr3::cr3_page_table_base().as_u64()) as *mut PageTable
    }
}
//==================================================================================================


pub enum PageSize {
    Size512Gb = 1,
    Size1Gb = 2,
    Size2Mb = 3,
    Size4Kb = 4
}
//==================================================================================================
struct PageIndexes([usize; 4]);
//==================================================================================================
impl PageIndexes {
    fn get_from_virt(virt: VirtAddr) -> Self {
        let pml4 = ((virt.as_u64() >> 39) & 0x1FF) as usize; // 4
        let pdpt = ((virt.as_u64() >> 30) & 0x1FF) as usize; // 3
        let pd = ((virt.as_u64() >> 21) & 0x1FF) as usize; // 2
        let pt = ((virt.as_u64() >> 12) & 0x1FF) as usize; // 1

        PageIndexes([pml4, pdpt, pd, pt])
    }

    fn pml4_index(&self) -> usize {
        self.0[0]
    }

    fn pdpt_index(&self) -> usize {
        self.0[1]
    }

    fn pd_index(&self) -> usize {
        self.0[2]
    }

    fn pt_index(&self) -> usize {
        self.0[3]
    }
}
//==================================================================================================
impl PageSize {

}
//==================================================================================================
#[repr(C)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const PRESENT: u64         = 1 << 0;
    pub const WRITABLE: u64        = 1 << 1;
    pub const USER_ACCESSIBLE: u64 = 1 << 2;
    pub const ACCESSED: u64        = 1 << 5;
    pub const DIRTY: u64           = 1 << 6;
    pub const HUGE: u64            = 1 << 7;
    pub const GLOBAL: u64          = 1 << 8;
    pub const NO_EXECUTE: u64      = 1 << 63;
    const PHYS_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

    pub fn read(entry: u64) -> Self {
        Self(entry)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    //-------------------------
    //  GETTERS
    //-------------------------
    pub fn as_pt_address(&self) -> *mut PageTable {
        self.address() as *mut PageTable
    }
    pub fn address(&self) -> u64 {
        self.0 & Self::PHYS_ADDR_MASK
    }

    pub fn is_present(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.0 & Self::WRITABLE) != 0
    }

    pub fn is_user_accessible(&self) -> bool {
        (self.0 & Self::USER_ACCESSIBLE) != 0
    }

    pub fn is_huge(&self) -> bool {
        (self.0 & Self::HUGE) != 0
    }

    pub fn is_global(&self) -> bool {
        (self.0 & Self::GLOBAL) != 0
    }

    pub fn is_no_execute(&self) -> bool {
        (self.0 & Self::NO_EXECUTE) != 0
    }

    pub fn is_accessed(&self) -> bool {
        (self.0 & Self::ACCESSED) != 0
    }

    pub fn is_dirty(&self) -> bool {
        (self.0 & Self::DIRTY) != 0
    }

    //-------------------------
    //  SETTERS
    //-------------------------
    pub fn set_address(&mut self, addr: u64) {
        self.0 = (self.0 & !Self::PHYS_ADDR_MASK) | (addr & Self::PHYS_ADDR_MASK);
    }

    fn set_flag(&mut self, flag: u64, value: bool) {
        if value {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}
//==================================================================================================
#[derive(Debug)]
pub enum PagingSetupError {
    NoMemoryMapProvided = 1

}
//==================================================================================================
fn early_page_alloc() -> *mut PageTable {
    unsafe {
        match eba_kmalloc::<PageTable>(PAGE_SIZE as usize, PAGE_SIZE as usize - 1) {
            None => panic!("No space for page table! Early bump region full"),
            Some(x) => {
                ptr::write_bytes(x as *mut u8, 0x00u8, PAGE_SIZE as usize);
                x
            }
        }
    }
}

//==================================================================================================
pub unsafe fn map_2mb_page(virt: VirtAddr, phys: PhysAddr) {
    let indexes = PageIndexes::get_from_virt(virt);

    let pml4 = PageTable::from_cr3();
    // vgaprintln!("pml4: {:#011x}", pml4 as *mut u64 as u64);
    let pdpt3 = (*pml4).get_ptr_from_index_or_eba_kmalloc(indexes.pml4_index());
    // vgaprintln!("pdpt3: {:#011x}", pdpt3 as *mut u64 as u64);
    let pd2 = (*pdpt3).get_ptr_from_index_or_eba_kmalloc(indexes.pdpt_index());

    let mut entry = &mut (*pd2).entries[indexes.pd_index()];
    entry.set_address(phys.as_u64() & 0x000F_FFFF_FFE0_0000);
    entry.set_flag(PageTableEntry::PRESENT, true);
    entry.set_flag(PageTableEntry::HUGE, true);
}







pub fn init(multiboot_info: &MultibootInfoView) -> Result<(), PagingSetupError> {
    multiboot_info.get_memory_map_tag().unwrap();

    let memory_map = match multiboot_info.get_memory_map_tag() {
        None => {
            return Err(NoMemoryMapProvided);
        },
        Some(x) => {
            x
        }
    };

    Ok(())


}

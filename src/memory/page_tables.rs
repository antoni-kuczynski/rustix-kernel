/*
 * Created by Antoni Kuczyński
 * 14/04/2026
 */
use core::ptr;
use x86_64::VirtAddr;
use crate::memory::{Cr3, SizeUnit, P2V, V2P};
use crate::memory::eba::eba_kmalloc;
use crate::vgaprintln;


fn early_page_alloc() -> *mut PageTable {
    unsafe {
        match eba_kmalloc::<PageTable>(PageSize::PAGE_TABLE_SIZE, PageSize::PAGE_TABLE_SIZE- 1) {
            None => panic!("No space for page table! Early bump region full"),
            Some(x) => {
                ptr::write_bytes(x as *mut u8, 0x00u8, PageSize::PAGE_TABLE_SIZE);
                x
            }
        }
    }
}

#[derive(Debug)]
pub enum PagingSetupError {
    NoMemoryMapProvided = 1
}

//==================================================================================================
//  PAGE TABLE
//==================================================================================================
const PAGE_TABLE_ENTRIES: usize = 512;
pub const PHYS_ADDR_MASK: u64 = 0x000F_FFFF_FFE0_0000;
#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; PAGE_TABLE_ENTRIES]
}

impl PageTable {
    pub fn get_ptr_from_index_or_eba_kmalloc(&mut self, index: usize) -> *mut PageTable {
        let mut entry = &mut self.entries[index];

        // vgaprintln!("{:#011x}", entry as *const PageTableEntry as *const u64 as u64);
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

    pub fn from_cr3() -> *mut PageTable {
        P2V(Cr3::cr3_page_table_base().as_u64()) as *mut PageTable
    }

    pub fn get_entries(&mut self) -> &mut [PageTableEntry; 512] {
        &mut self.entries
    }
}
//==================================================================================================
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

    pub fn set_flag(&mut self, flag: u64, value: bool) {
        if value {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}
//==================================================================================================
//==================================================================================================
pub struct PageIndexes([usize; 4]);

impl PageIndexes {
    pub fn get_from_virt(virt: VirtAddr) -> Self {
        let pml4 = ((virt.as_u64() >> 39) & 0x1FF) as usize; // 4
        let pdpt = ((virt.as_u64() >> 30) & 0x1FF) as usize; // 3
        let pd = ((virt.as_u64() >> 21) & 0x1FF) as usize; // 2
        let pt = ((virt.as_u64() >> 12) & 0x1FF) as usize; // 1

        PageIndexes([pml4, pdpt, pd, pt])
    }

    pub fn pml4_index(&self) -> usize {
        self.0[0]
    }

    pub fn pdpt_index(&self) -> usize {
        self.0[1]
    }

    pub fn pd_index(&self) -> usize {
        self.0[2]
    }

    pub fn pt_index(&self) -> usize {
        self.0[3]
    }
}
//==================================================================================================
//==================================================================================================
pub struct PageSize();

impl PageSize {
    pub const PAGE_TABLE_SIZE: usize = 0x1000;
    pub const SIZE_4KB: u64 = 0x1000;
    pub const SIZE_2MB: u64 = 0x200000;
    pub const SIZE_1GB: u64 = 1_073_741_824;
}
//==================================================================================================
//==================================================================================================

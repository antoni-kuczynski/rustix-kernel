/*
 * Created by Antoni Kuczyński
 * 13/02/2026
 */
use crate::boot::multiboot::MultibootInfoView;

//==================================================================================================
//  PAGE TABLE
//==================================================================================================
const PAGE_TABLE_ENTRIES: usize = 512;

#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRIES]
}

#[repr(C)]
#[derive(Copy)]
#[derive(Clone)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    const PRESENT: u64 = 1;
    const WRITABLE: u64 = 1 << 1;
    const USER_ACCESIBLE: u64 = 1 << 2;

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

//==================================================================================================
#[derive(Debug)]
pub enum PagingSetupError {

}



pub fn init(multiboot_info: &MultibootInfoView) -> Result<(), PagingSetupError> {
    //TODO
    Ok(())
}

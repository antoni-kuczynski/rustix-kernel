#![allow(unused)]
use core::{fmt, ptr};
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::multiboot::{multiboot2_logical_end, multiboot2_memory_map_tag, MemoryRegionType, MultibootInfoView, MultibootMemoryMapEntry, MultibootMemoryMapTag, MULTIBOOT_INFO};
use crate::{vgaprintln};
use crate::memory::{Cr3, SizeUnit, FRAME_SIZE, P2V, V2P};
use crate::memory::page_tables::{PageSize, PageTable};
use crate::memory::paging::eba_map_2mb_range;
use crate::memory::pmm::PmmInitError::NoMemorySizeProvided;
//==================================================================================================
const USED: u8 = 1;
const FREE: u8 = 0;
//==================================================================================================
#[derive(Debug)]
pub enum PmmInitError {
    NoMemorySizeProvided = 1
}
//==================================================================================================
#[derive(Debug)]
pub enum PmmAllocErrorType {
    FrameAddressNotAligned = 1,
    FrameAlreadyUsed = 2,
    BitmapWriteFailed = 3,
    FrameAlreadyFreed = 4
}
pub struct PmmAllocError {
    frame: PhysAddr,
    error_type: PmmAllocErrorType
}


impl fmt::Display for PmmAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at phys frame address {:#x}", self.error_type, self.frame)
    }
}

impl fmt::Debug for PmmAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at phys frame address {:#x}", self.error_type, self.frame)
    }
}

impl PmmAllocError {
    fn new(frame: PhysAddr, error_type: PmmAllocErrorType) -> Self {
        Self {
            frame, error_type
        }
    }
}
//==================================================================================================
pub struct PmmBitmap {
    ptr: AtomicPtr<u8>,
    length: u64
}
//==================================================================================================
impl PmmBitmap {
    fn alloc_used_memory_regions(&self) {
        /*
        Marks memory frames as used:
        - kernel code, multiboot info, modules
        - bios region
        - VGA region
        - bitmap, page tables
         */

        //==========================================================================================
        // 1. MARK USABLE MEMORY REGIONS AS FREE
        //==========================================================================================
        unsafe {
            //we already know that memory map tag exists, checked earlier
            let memory_map = multiboot2_memory_map_tag().unwrap();


            let size_entries = (*memory_map).header().size() - size_of::<MultibootMemoryMapTag>() as u32;
            let mut entry1 = (self as *const Self as *const u32).add(4) as *const MultibootMemoryMapEntry;
            let last = entry1.byte_add(size_entries as usize);

            vgaprintln!("entry 1: {:#011x}, last: {:#011x}", entry1 as *const u64 as u64, last as *const u64 as u64);

            while entry1 < last {
                let region_type = match MemoryRegionType::from_u32((*entry1).addr_range_type()) {
                    None => {
                        entry1 = entry1.add(1);
                        continue
                    },   //invalid memory region so skip it
                    Some(x) => { x }
                };

                vgaprintln!("base_addr: {:#011x}, length: {}", (*entry1).base_addr(), (*entry1).length());

                if region_type != MemoryRegionType::AvailableRAM {
                    entry1 = entry1.add(1);
                    continue
                }

                let mut base_frame_addr = ((*entry1).base_addr() / 4096) & !(FRAME_SIZE - 1);
                let length_of_frames = ((*entry1).length() / 4096) & !(FRAME_SIZE - 1);
                let last_frame = base_frame_addr + length_of_frames;
                let mut bitmap_ptr = &self.ptr;

                vgaprintln!("base: {:#011x} size: {}", base_frame_addr, length_of_frames);

                while base_frame_addr <= (last_frame - 8 * FRAME_SIZE) {
                    // let byte = base_frame_addr / 8;
                    // let bit = base_frame_addr & 0x07;

                    ptr::write_volatile(bitmap_ptr.load(Ordering::Acquire), FREE);

                    bitmap_ptr.fetch_ptr_add(1, Ordering::AcqRel);
                    base_frame_addr = base_frame_addr + FRAME_SIZE*8;
                }

                entry1 = entry1.add(1);
            }
        }
        //==========================================================================================
        // 2. MARK PAGED REGIONS AS USED
        //==========================================================================================
        self.sync_allocator_with_page_tables();

        vgaprintln!("Length total: {}", (V2P(self.ptr.load(Ordering::Acquire) as u64) + self.length) / FRAME_SIZE / 8);
        //also reserve the unmapped guard holes
        //basically, everything til the end of this bitmap is marked as used
        self.reserve_range(
            PhysAddr::new(0x00000),
            V2P(self.ptr.load(Ordering::Acquire) as u64) + self.length //we know that the pointer is the beginning of the bitmap right now
        );

    }


    fn sync_allocator_with_page_tables(&self) {
        unsafe {
            let pml4 = &*PageTable::from_cr3();
            do_pml4(&self, Cr3::cr3_page_table_base().as_u64(), &pml4);
        }

        unsafe fn do_pml4(self1: &PmmBitmap, pml4_phys: u64, pml4: &&PageTable) {
            for pml4_idx in 0..512 {
                let pml4_entry = &pml4.entries[pml4_idx];
                if !pml4_entry.is_present() {
                    continue;
                }

                let pdpt_phys = pml4_entry.address();
                if pdpt_phys == pml4_phys {
                    continue;
                }

                self1.reserve_range(PhysAddr::new(pdpt_phys), PageSize::SIZE_4KB);

                let pdpt = unsafe { &*(P2V(pdpt_phys) as *const PageTable) };
                unsafe { do_pdpt3(self1, &pdpt); }
            }
        }

        unsafe fn do_pdpt3(self1: &PmmBitmap, pdpt: &&PageTable) {
            for pdpt_idx in 0..512 {
                let pdpt_entry = &pdpt.entries[pdpt_idx];
                if !pdpt_entry.is_present() {
                    continue;
                }

                if pdpt_entry.is_huge() {
                    self1.reserve_range(PhysAddr::new(pdpt_entry.address()), PageSize::SIZE_1GB);
                    continue;
                }

                let pd_phys = pdpt_entry.address();

                let pd = unsafe { &*(P2V(pd_phys) as *const PageTable) };
                do_pd2(self1, &pd);
            }
        }

        fn do_pd2(self1: &PmmBitmap, pd: &&PageTable) {
            for pd_idx in 0..512 {
                let pd_entry = &pd.entries[pd_idx];
                if !pd_entry.is_present() {
                    continue;
                }

                if pd_entry.is_huge() {
                    self1.reserve_range(PhysAddr::new(pd_entry.address()), PageSize::SIZE_2MB);
                    continue;
                }

                let pt_phys = pd_entry.address();

                let pt = unsafe { &*(P2V(pt_phys) as *const PageTable) };
                do_pt1(&pt);
            }
        }

        fn do_pt1(pt: &&PageTable) {
            for pt_idx in 0..512 {
                let pt_entry = &pt.entries[pt_idx];
                if !pt_entry.is_present() {
                    continue;
                }
            }
        }
    }

    //==================================================================================================
    fn reserve_frame(&self, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
        //mark as used = true
        self.modify_frame(frame_addr, true)
    }


    fn reserve_range(&self, start_addr: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
        if length_bytes == 0 {
            return Ok(());
        }

        let start_aligned = start_addr.as_u64() & !(FRAME_SIZE - 1); //round down

        let end_addr = start_addr.as_u64() + length_bytes;
        let end_aligned = (end_addr + FRAME_SIZE - 1) & !(FRAME_SIZE - 1); //round up

        let mut current = start_aligned;
        while current < end_aligned {
            self.reserve_frame(PhysAddr::new(current)).expect("alloc error");
            current += FRAME_SIZE;
        }

        Ok(())
    }



    fn free_frame(&self, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
        //mark as used = false
        self.modify_frame(frame_addr, false)
    }

    fn free_range(&self, start_addr: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
        if length_bytes == 0 {
            return Ok(());
        }

        let start_aligned = start_addr.as_u64() & !(FRAME_SIZE - 1);
        let end_addr = start_addr.as_u64() + length_bytes;
        let end_aligned = (end_addr + FRAME_SIZE - 1) & !(FRAME_SIZE - 1);

        let mut current = start_aligned;
        while current < end_aligned {
            self.free_frame(PhysAddr::new(current))?;
            current += FRAME_SIZE;
        }

        Ok(())
    }
//==================================================================================================
    fn modify_frame(&self, frame_addr: PhysAddr, alloc_mode: bool) -> Result<(), PmmAllocError> {
        let frame_idx = frame_addr.as_u64() / 4096;
        let byte_idx = (frame_idx / 8) as usize;
        let bit_idx = (frame_idx % 8) as u8;
        let mask: u8 = 1 << bit_idx;

        let base_ptr = self.ptr.load(Ordering::Acquire);

        unsafe {
            let target_byte_ptr = base_ptr.add(byte_idx) as *const AtomicU8;
            let atomic_byte = &*target_byte_ptr;

            return if alloc_mode {
                alloc_fetch(atomic_byte, mask, frame_addr)
            } else {
                free_fetch(atomic_byte, mask, frame_addr)
            };
        }

        fn alloc_fetch(atomic_byte: &AtomicU8, mask: u8, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
            let prev = atomic_byte.fetch_or(mask, Ordering::AcqRel);

            Ok(())
        }

        fn free_fetch(atomic_byte: &AtomicU8, mask: u8, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
            let prev = atomic_byte.fetch_and(!mask, Ordering::AcqRel);

            Ok(())
        }
    }
//==================================================================================================
    pub fn print(&self, range: usize) {
        unsafe {
            let mut arr = &self.ptr;
            for i in 0..range {
                vgaprintln!("{}:    {:#08b}", i, *(arr.load(Ordering::Acquire)));
                arr.fetch_ptr_add(1, Ordering::AcqRel);
            }
        }
    }

    pub fn length(&self) -> u64 {
        self.length
    }
}
//==================================================================================================
pub fn init() -> Result<(), PmmInitError> {
    let mem_map = match multiboot2_memory_map_tag() {
        None => {
            return Err(NoMemorySizeProvided);
        },
        Some(x) => {
            x
        }
    };
    unsafe {
        let mem_size = (*mem_map).get_high_usable_memory_address();

        let bitmap_size_bytes = mem_size.as_u64() / FRAME_SIZE / 8; //one bit per frame
        let bitmap_start_ptr = AtomicPtr::new(((multiboot2_logical_end().as_u64() + PageSize::SIZE_2MB) & !(PageSize::SIZE_2MB - 1)) as *mut u8);

        eba_map_2mb_range(
            VirtAddr::new_truncate(bitmap_start_ptr.load(Ordering::Acquire) as u64),
            PhysAddr::new_truncate(V2P(bitmap_start_ptr.load(Ordering::Acquire) as u64)),
            bitmap_size_bytes
        );

        let mut p_bitmap = &bitmap_start_ptr;
        for _i in 0..=bitmap_size_bytes {
            ptr::write_volatile(p_bitmap.load(Ordering::Acquire), 0); //later we mark regions as free
            p_bitmap.fetch_ptr_add(1, Ordering::Acquire);
        }

        PMM_BITMAP.lock().ptr = bitmap_start_ptr;
        PMM_BITMAP.lock().length = bitmap_size_bytes;
        PMM_BITMAP.lock().alloc_used_memory_regions();

        Ok(())
    }
}
//==================================================================================================
lazy_static! {
    pub static ref PMM_BITMAP: Mutex<PmmBitmap> = Mutex::new(PmmBitmap {ptr: Default::default(),length: 0,});
}

/// Reserves a frame in pmm bitmap
pub fn pmm_reserve_frame(frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
    PMM_BITMAP.lock().reserve_frame(frame_addr)
}

/// Reserves a frame range in pmm bitmap
pub fn pmm_reserve_range(start_addr: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
    PMM_BITMAP.lock().reserve_range(start_addr, length_bytes)
}

/// Frees a frame in pmm bitmap
pub fn pmm_free_frame(frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
    PMM_BITMAP.lock().free_frame(frame_addr)
}


/// Reserves a frame range in pmm bitmap
pub fn pmm_free_range(start_addr: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
    PMM_BITMAP.lock().free_range(start_addr, length_bytes)
}


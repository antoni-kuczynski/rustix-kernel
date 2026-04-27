#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
use crate::VGAWRITER;
use crate::ColorTextMode;
use core::{fmt, ptr};
use core::sync::atomic::{AtomicPtr, AtomicU64, AtomicU8, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};
use crate::boot::multiboot::{multiboot2_logical_end, multiboot2_memory_map_tag, MemoryRegionType, MultibootInfoView, MultibootMemoryMapEntry, MultibootMemoryMapTag, MULTIBOOT_INFO};
use crate::{print_ok_msg, vgaprint, vgaprintln};
use crate::memory::{Cr3, SizeUnit, FRAME_SIZE, _P2V_kernel, _V2P_kernel, KERNEL_VIRT_BASE};
use crate::memory::page_tables::{PageSize, PageTable};
use crate::memory::paging::vmm_eba_map_range;
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
    unsafe fn alloc_used_memory_regions(&self, memory_map: &MultibootMemoryMapTag) {
        //==========================================================================================
        // 1. MARK USABLE MEMORY REGIONS AS FREE
        //==========================================================================================

        let size_entries = (*memory_map).header().size - size_of::<MultibootMemoryMapTag>() as u32;
        let mut entry1 = &memory_map.entries as *const MultibootMemoryMapEntry;
        let last = entry1.byte_add(size_entries as usize);

        while entry1 < last {
            let region_type = match MemoryRegionType::from_u32((*entry1).addr_range_type()) {
                None => {
                    entry1 = entry1.add(1);
                    continue
                },
                Some(x) => { x }
            };

            if region_type != MemoryRegionType::AvailableRAM {
                entry1 = entry1.add(1);
                continue
            }

            let mut base_frame_addr = (*entry1).base_addr() / 4096;
            let length_of_frames = ((*entry1).length() / 4096) & !(FRAME_SIZE - 1);
            let last_frame = base_frame_addr + length_of_frames;

            let base_ptr = self.ptr.load(Ordering::Acquire);

            while base_frame_addr <= (last_frame - 8 * FRAME_SIZE) {
                let byte_idx = (base_frame_addr / 8) as usize;
                if (byte_idx as u64) < self.length {
                    ptr::write_volatile(base_ptr.add(byte_idx), FREE);
                } else {
                    // check if we're really on high addr and nothings wrong
                    if base_frame_addr * 4096 != memory_map.get_high_usable_memory_address().as_u64() {
                        panic!("Memory high address not inside bitmap: high addr {:#011x} not equal to bitmaps: {:#011x}",
                               base_frame_addr * 4096, memory_map.get_high_usable_memory_address().as_u64());
                    }

                    break; // we already allocated memory up to high addr
                }
                base_frame_addr = base_frame_addr + 8;
            }

            entry1 = entry1.add(1);
        }
        //==========================================================================================
        // 2. MARK PAGED REGIONS AS USED
        //==========================================================================================
        self.reserve_range(
            PhysAddr::new(0x00000),
            (self.ptr.load(Ordering::Acquire) as u64 + self.length) - KERNEL_VIRT_BASE
        );
    }

    //==================================================================================================
    fn reserve_frame(&self, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
        let start_frame = frame_addr.as_u64() / 4096;
        self.modify_bit_range(start_frame, 1, true);
        Ok(())
    }

    fn reserve_range(&self, start_addr_inside_page: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
        if length_bytes == 0 {
            return Ok(());
        }

        let start_aligned = start_addr_inside_page.as_u64() & !0xFFF;
        let end_aligned = (start_addr_inside_page.as_u64() + length_bytes + 0xFFF) & !0xFFF;
        let frame_count = (end_aligned - start_aligned) / 4096;

        self.modify_bit_range(start_aligned / 4096, frame_count, true);
        Ok(())
    }

    fn free_frame(&self, frame_addr: PhysAddr) -> Result<(), PmmAllocError> {
        let start_frame = frame_addr.as_u64() / 4096;
        self.modify_bit_range(start_frame, 1, false);
        Ok(())
    }

    fn free_range(&self, start_addr_inside_page: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
        if length_bytes == 0 {
            return Ok(());
        }

        let start_aligned = start_addr_inside_page.as_u64() & !0xFFF;
        let end_aligned = (start_addr_inside_page.as_u64() + length_bytes + 0xFFF) & !0xFFF;
        let frame_count = (end_aligned - start_aligned) / 4096;

        self.modify_bit_range(start_aligned / 4096, frame_count, false);
        Ok(())
    }
//==================================================================================================
    fn modify_bit_range(&self, start_frame: u64, frame_count: u64, alloc_mode: bool) {
        if frame_count == 0 {
            return;
        }

        let base_ptr = self.ptr.load(Ordering::Acquire) as *const AtomicU64;
        let mut current_frame = start_frame;
        let mut frames_left = frame_count;

        while frames_left > 0 {
            let u64_idx = (current_frame / 64) as usize;
            let bit_offset = current_frame % 64;

            let frames_in_this_u64 = core::cmp::min(64 - bit_offset, frames_left);

            let mask = if frames_in_this_u64 == 64 {
                u64::MAX
            } else {
                ((1u64 << frames_in_this_u64) - 1) << bit_offset
            };

            unsafe {
                let atomic_val = &*base_ptr.add(u64_idx);
                if alloc_mode { //alloc
                    atomic_val.fetch_or(mask, Ordering::AcqRel);
                } else { //free
                    atomic_val.fetch_and(!mask, Ordering::AcqRel);
                }
            }

            current_frame += frames_in_this_u64;
            frames_left -= frames_in_this_u64;
        }
    }
//==================================================================================================
    pub unsafe fn print(&self, range: usize) {
        let base_ptr = self.ptr.load(Ordering::Acquire);
        for i in 0..range {
            vgaprintln!("{}:    {:#08b}", i, *base_ptr.add(i));
        }
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn allocate_frame(&self) -> Option<PhysAddr> {
        let base_ptr = self.ptr.load(Ordering::Acquire) as *const AtomicU64;
        let u64_count = self.length / size_of::<u64>() as u64;

        for i in 0..u64_count {
            unsafe {
                let atomic_val = &*base_ptr.add(i as usize);
                let val = atomic_val.load(Ordering::Acquire);
                if val != u64::MAX {
                    let bit_idx = (!val).trailing_zeros() as u64;
                    if bit_idx < 64 {
                        let frame_idx = i * 64 + bit_idx;
                        self.modify_bit_range(frame_idx, 1, true);
                        return Some(PhysAddr::new(frame_idx * 4096));
                    }
                }
            }
        }
        None
    }
}
//==================================================================================================
/// Initializes the PMM
pub fn pmm_init() {
    vgaprint!("Initializing physical frame allocator...");
    let mem_map = match multiboot2_memory_map_tag() {
        None => {
            panic!("PMM init: Mb2 memory map tag doesn't exist!")
        },
        Some(x) => {
            x
        }
    };
    unsafe {
        let mem_size = mem_map.get_high_usable_memory_address();

        let bitmap_size_bytes = mem_size.as_u64() / FRAME_SIZE / 8; //one bit per frame
        let bitmap_start_addr = (multiboot2_logical_end().as_u64() + PageSize::SIZE_2MB) & !(PageSize::SIZE_2MB - 1);
        
        vmm_eba_map_range(
            VirtAddr::new_truncate(bitmap_start_addr),
            PhysAddr::new_truncate(_V2P_kernel(bitmap_start_addr)),
            bitmap_size_bytes,
            &PageSize::Size2Mb,
            false
        );

        let p_bitmap = bitmap_start_addr as *mut u8;
        for i in 0..bitmap_size_bytes {
            ptr::write_volatile(p_bitmap.add(i as usize), 0xFF); // Mark all as USED initially
        }

        let mut lock = PMM_BITMAP.lock();
        lock.ptr.store(p_bitmap, Ordering::Release);
        lock.length = bitmap_size_bytes;
        lock.alloc_used_memory_regions(&mem_map);
    }
    print_ok_msg!();
}
//==================================================================================================
lazy_static! {
    pub static ref PMM_BITMAP: Mutex<PmmBitmap> = Mutex::new(PmmBitmap {ptr: AtomicPtr::new(ptr::null_mut()),length: 0,});
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

/// Allocates a first fit frame in PMM
pub fn pmm_allocate_frame() -> Option<PhysAddr> {
    PMM_BITMAP.lock().allocate_frame()
}


/// Reserves a frame range in pmm bitmap
pub fn pmm_free_range(start_addr: PhysAddr, length_bytes: u64) -> Result<(), PmmAllocError> {
    PMM_BITMAP.lock().free_range(start_addr, length_bytes)
}

pub fn pmm_is_enabled() -> bool {
    !PMM_BITMAP.lock().ptr.load(Ordering::Acquire).is_null()
}

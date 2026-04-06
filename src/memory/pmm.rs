#![allow(unused)]
use core::fmt;
use core::sync::atomic::{AtomicU8, Ordering};
use lazy_static::lazy_static;
use crate::boot::multiboot::{MemoryRegionType, MultibootInfoView, MultibootMemoryMapEntry, MultibootMemoryMapTag};
use crate::{vgaprintln};
use crate::memory::{ FRAME_SIZE };
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
    frame: u64,
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
    fn new(frame: u64, error_type: PmmAllocErrorType) -> Self {
        Self {
            frame, error_type
        }
    }
}
//==================================================================================================
pub struct PmmBitmap {
    ptr: *mut AtomicU8,
    length: u64
}
//==================================================================================================
impl PmmBitmap {
    fn alloc_used_memory_regions(&self, multiboot_info: &MultibootInfoView) {
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
            let memory_map = multiboot_info.get_memory_map_tag().unwrap();


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
                let mut bitmap_ptr = self.ptr;

                vgaprintln!("base: {:#011x} size: {}", base_frame_addr, length_of_frames);

                while base_frame_addr <= (last_frame - 8 * FRAME_SIZE) {
                    // let byte = base_frame_addr / 8;
                    // let bit = base_frame_addr & 0x07;

                    *bitmap_ptr = AtomicU8::from(0x00u8); //free

                    bitmap_ptr = bitmap_ptr.add(1);
                    base_frame_addr = base_frame_addr + FRAME_SIZE*8;
                }

                
                entry1 = entry1.add(1);
            }
        }

    }
//==================================================================================================
    pub fn allocate_frame(&self, frame_addr: u64) -> Result<(), PmmAllocError> {
        //mark as used = true
        self.modify_frame(frame_addr, true)
    }

    pub fn free_frame(&self, frame_addr: u64) -> Result<(), PmmAllocError> {
        //mark as used = false
        self.modify_frame(frame_addr, false)
    }
//==================================================================================================
    pub fn modify_frame(&self, frame_addr: u64, alloc_mode: bool) -> Result<(), PmmAllocError> {
        if frame_addr & 0xFFF != 0 {
            return Err(PmmAllocError::new(frame_addr, PmmAllocErrorType::FrameAddressNotAligned));
        }

        let target_bit = frame_addr / 4096;
        let target_byte = if target_bit == 0 {
            0 //frame 0
        } else {
            ((target_bit + 8) / 8) - 1
        };

        unsafe {
            let ptr = self.ptr.add(target_byte as usize);

            return if alloc_mode {
                alloc(ptr, target_bit, frame_addr)
            } else {
                free(ptr, target_bit, frame_addr)
            };

            unsafe fn alloc(ptr: *mut AtomicU8, target_bit: u64, frame_addr: u64) -> Result<(), PmmAllocError> {
                unsafe {
                    let mask = 1 << target_bit;
                    if (*ptr).load(Ordering::Acquire) & (mask) != FREE {
                        return Err(PmmAllocError::new(frame_addr, PmmAllocErrorType::FrameAlreadyUsed));
                    }

                    (*ptr).fetch_or(mask, Ordering::Release);
                    // *ptr |= 1 << (target_bit);

                    if (*ptr).load(Ordering::Acquire) | (mask) == 0 {
                        return Err(PmmAllocError::new(frame_addr, PmmAllocErrorType::BitmapWriteFailed));
                    }
                    Ok(())
                }
            }


            unsafe fn free(ptr: *mut AtomicU8, target_bit: u64, frame_addr: u64) -> Result<(), PmmAllocError> {
                unsafe {
                    let mask = 1 << target_bit;
                    if (*ptr).load(Ordering::Acquire) & (mask) == FREE {
                        return Err(PmmAllocError::new(frame_addr, PmmAllocErrorType::FrameAlreadyFreed));
                    }

                    (*ptr).fetch_and(!mask, Ordering::Release);
                    // *ptr &= !(1 << (target_bit));

                    if (*ptr).load(Ordering::Acquire) | (mask) == USED {
                        return Err(PmmAllocError::new(frame_addr, PmmAllocErrorType::BitmapWriteFailed));
                    }
                    Ok(())
                }
            }
        }
    }
//==================================================================================================
    pub fn print(&self, range: usize) {
        unsafe {
            let mut arr = self.ptr;
            for i in 0..range {
                vgaprintln!("{}:    {:#08b}", i, (*arr).load(Ordering::Acquire));
                arr = arr.add(1);
            }
        }
    }

    pub fn ptr(&self) -> *mut AtomicU8 {
        self.ptr
    }

    pub fn length(&self) -> u64 {
        self.length
    }
}
//==================================================================================================
pub fn init(multiboot_info: &MultibootInfoView) -> Result<PmmBitmap, PmmInitError> {
    let mem_map = match multiboot_info.get_memory_map_tag() {
        None => {
            return Err(NoMemorySizeProvided);
        },
        Some(x) => {
            x
        }
    };
    unsafe {
        let mem_size = (*mem_map).get_high_usable_memory_address();

        let bitmap_size_bytes = mem_size / FRAME_SIZE / 8; //one bit per frame
        let bitmap = multiboot_info.multiboot_end_logical() as *mut AtomicU8;

        let mut p_bitmap = bitmap;
        for _i in 0..=bitmap_size_bytes {
            (*p_bitmap).fetch_or(0xFF, Ordering::Release); //later we mark regions as free
            // *p_bitmap = 0xFFu8;
            p_bitmap = p_bitmap.add(1);
        }

        let bitmap_data = PmmBitmap {
            ptr: bitmap,
            length: bitmap_size_bytes
        };
        bitmap_data.alloc_used_memory_regions(multiboot_info);

        Ok(bitmap_data)
    }
}
//==================================================================================================

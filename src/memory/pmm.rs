use core::error::Error;
use core::ops::Add;
use core::fmt;
use crate::boot::multiboot::{MemoryRegionType, MultibootInfoView, MultibootMemoryMapTag};
use crate::{endKernel, vgaprintln};
use crate::memory::{kernel_end, SizeUnit, FRAME_SIZE};
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
    BitmapWriteFailed = 3
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
    ptr: *mut u8,
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
            let mut entry1 = (self as *const Self as *const u32).add(4) as *const crate::boot::multiboot::MultibootMemoryMapEntry;
            let last = entry1.byte_add(size_entries as usize);

            while entry1 < last {
                let region_type = match MemoryRegionType::from_u32((*entry1).addr_range_type()) {
                    None => {
                        entry1 = entry1.add(1);
                        continue
                    },   //invalid memory region so skip it
                    Some(x) => { x }
                };

                if region_type != MemoryRegionType::AvailableRAM {
                    entry1 = entry1.add(1);
                    continue
                }

                let base_addr = (*entry1).base_addr();
                let length = (*entry1).length();
                //todo
                
                
                entry1 = entry1.add(1);
            }
        }

    }

    pub fn free_frame(&self) {

    }

    pub fn allocate_frame_range(&self, frame_start_addr: u64, frame_length: u64) -> Result<(), PmmAllocError> {
        if frame_start_addr & 0xFFF != 0 {
            return Err(PmmAllocError::new(frame_start_addr, PmmAllocErrorType::FrameAddressNotAligned));
        }
        let frame_end = frame_start_addr + frame_length*FRAME_SIZE;

        let mut i = frame_start_addr;
        while i < frame_end {
            let alloc = self.allocate_frame(i);

            //alloc went okay
            if matches!(alloc, Ok(())) {
                vgaprintln!("{:#011x}", i);
                i = i + FRAME_SIZE;
                continue;
            }

            //error occured
            return alloc;
        }
        Ok(())
    }
    
    pub fn allocate_frame(&self, frame_addr: u64) -> Result<(), PmmAllocError> {
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
            let ptr = (self.ptr.add(target_byte as usize));
            if *ptr & (1 << (target_bit)) != 0 {
                return Err(PmmAllocError::new(ptr as u64, PmmAllocErrorType::FrameAlreadyUsed));
            }

            *ptr |= 1 << (target_bit);

            if *ptr | (1 << (target_bit)) == 0 {
                return Err(PmmAllocError::new(ptr as u64, PmmAllocErrorType::BitmapWriteFailed));
            }
        }
        Ok(())
    }

    pub fn print(&self, range: usize) {
        unsafe {
            let mut arr = self.ptr;
            for i in 0..range {
                vgaprintln!("{}:    {:#08b}", i, *arr);
                arr = arr.add(1);
            }
        }
    }
}
//==================================================================================================
pub fn init(multiboot_info: &MultibootInfoView) -> Result<(PmmBitmap), PmmInitError> {
    let mem_map = match multiboot_info.get_memory_map_tag() {
        None => {
            return Err(NoMemorySizeProvided);
        },
        Some(x) => {
            x
        }
    };
    unsafe {
        let mem_size = (*mem_map).get_available_memory(SizeUnit::Byte);

        let bitmap_size_bytes = mem_size / FRAME_SIZE / 8; //one bit per frame
        let bitmap = multiboot_info.multiboot_end_logical() as *mut u8;

        //todo:fix and remove temp
        let mut p_bitmap = bitmap;
        for i in 0..bitmap_size_bytes {
            *p_bitmap = 0x00u8; //later we mark regions as free
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
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
    
    
    fn allocate_frame(&self) {
        //todo
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

        let mut p_bitmap = bitmap;
        for i in 0..bitmap_size_bytes {
            *p_bitmap = USED; //later we mark regions as free
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
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 19/05/2026
 */
use crate::ColorTextMode;
use crate::memory::SizeUnit;
use crate::memory::ll_allocator::LinkedListAllocator;
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::memory::paging::{virtual_to_physical, vmm_map_page_ext};
use crate::memory::pmm::pmm_allocate_contiguous;
use crate::{VGAWRITER, print_ok_msg, vgaprint, vgaprintln};
use core::alloc::Layout;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

const DMA_START: u64 = 0xffff_d300_0000_0000;
const DMA_SIZE: u64 = 16 * 1_099_511_627_776; // 16tb

pub struct DmaManager {
    allocator: LinkedListAllocator,
}

impl DmaManager {
    pub const fn new() -> Self {
        Self {
            allocator: LinkedListAllocator::new(),
        }
    }

    pub unsafe fn init(&mut self) {
        let flags = PageTableEntry::PRESENT
            | PageTableEntry::WRITABLE
            | PageTableEntry::CACHE_DISABLE
            | PageTableEntry::WRITE_THROUGH;

        self.allocator.init(DMA_START as usize, DMA_SIZE as usize);
        self.allocator.set_contiguous(true);
        self.allocator.set_flags(flags);
    }

    pub fn alloc_coherent(&mut self, size: usize, align: usize) -> Option<DmaAlloc> {
        let layout = Layout::from_size_align(size, align).ok()?;
        unsafe {
            let ptr = self.allocator.allocate(layout);
            if ptr.is_null() {
                return None;
            }

            let virt = VirtAddr::new(ptr as u64);
            let phys =
                virtual_to_physical(virt).expect("DMA virtual address not mapped to physical");

            Some(DmaAlloc::new(virt, phys, layout))
        }
    }

    pub unsafe fn free(&mut self, virt: VirtAddr, layout: Layout) {
        self.allocator.deallocate(virt.as_u64() as *mut u8, layout);
    }

    pub fn allocator(&mut self) -> &mut LinkedListAllocator {
        &mut self.allocator
    }
}

lazy_static! {
    pub static ref DMA_MANAGER: Mutex<DmaManager> = Mutex::new(DmaManager::new());
}

#[derive(Debug)]
pub struct DmaAlloc {
    pub virt: VirtAddr,
    pub phys: PhysAddr,
    pub layout: Layout,
}

impl DmaAlloc {
    pub fn new(virt: VirtAddr, phys: PhysAddr, layout: Layout) -> Self {
        Self { virt, phys, layout }
    }
}

impl Drop for DmaAlloc {
    fn drop(&mut self) {
        unsafe {
            DMA_MANAGER.lock().free(self.virt, self.layout);
        }
    }
}

//==================================================================================================
// PUBLIC WRAPPERS
//==================================================================================================

pub fn dma_init() {
    vgaprint!("Initializing DMA allocator...");
    unsafe {
        DMA_MANAGER.lock().init();
    }
    print_ok_msg!();
}

pub fn dma_alloc_coherent(size: usize, align: usize) -> Option<DmaAlloc> {
    DMA_MANAGER.lock().alloc_coherent(size, align)
}

pub fn dma_free(alloc: DmaAlloc) {
    drop(alloc);
}

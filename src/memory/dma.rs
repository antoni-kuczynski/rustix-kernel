#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 19/05/2026
 */
use crate::memory::SizeUnit;
use crate::memory::ll_allocator::LinkedListAllocator;
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::memory::paging::{virtual_to_physical, vmm_map_page_ext};
use crate::memory::pmm::pmm_allocate_contiguous;
use crate::memory::dma32::DMA32_MANAGER;
use crate::{kprintln, kprintln_ok};
use core::alloc::Layout;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

pub(super) const DMA64_START: u64 = 0xffff_d300_0000_0000;
pub(super) const DMA64_SIZE: u64 = 12 * 1_099_511_627_776; // 12tb, DMA32 pool follows right after

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaZone {
    Dma64,
    Dma32,
}

pub(super) fn is_physically_contiguous(virt: VirtAddr, phys: PhysAddr, size: usize) -> bool {
    let page_size = PageSize::SIZE_4KB;
    let start = virt.as_u64();
    let end = start + size as u64;
    let mut page = (start & !(page_size - 1)) + page_size;

    while page < end {
        match virtual_to_physical(VirtAddr::new(page)) {
            Some(p) if p.as_u64() == phys.as_u64() + (page - start) => {}
            _ => return false,
        }
        page += page_size;
    }
    true
}

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

        self.allocator.init(DMA64_START as usize, DMA64_SIZE as usize);
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

            if !is_physically_contiguous(virt, phys, size) {
                kprintln!(Error, "[DMA] Allocation at {:#x} ({} bytes) is not physically contiguous, rejecting it.", virt.as_u64(), size);
                self.allocator.deallocate(ptr, layout);
                return None;
            }

            Some(DmaAlloc::new(virt, phys, layout, DmaZone::Dma64))
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
    pub zone: DmaZone,
}

impl DmaAlloc {
    pub fn new(virt: VirtAddr, phys: PhysAddr, layout: Layout, zone: DmaZone) -> Self {
        Self { virt, phys, layout, zone }
    }
}

impl Drop for DmaAlloc {
    fn drop(&mut self) {
        unsafe {
            match self.zone {
                DmaZone::Dma64 => DMA_MANAGER.lock().free(self.virt, self.layout),
                DmaZone::Dma32 => DMA32_MANAGER.lock().free(self.virt, self.layout),
            }
        }
    }
}

//==================================================================================================
// PUBLIC WRAPPERS
//==================================================================================================

pub fn dma_init() {
    unsafe {
        DMA_MANAGER.lock().init();
    }
    kprintln_ok!("Initialized DMA pool.");
}

pub fn dma_alloc_coherent(size: usize, align: usize) -> Option<DmaAlloc> {
    DMA_MANAGER.lock().alloc_coherent(size, align)
}

pub fn dma_free(alloc: DmaAlloc) {
    drop(alloc);
}

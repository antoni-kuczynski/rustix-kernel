#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
use crate::memory::dma::{is_physically_contiguous, DmaAlloc, DmaZone, DMA64_START, DMA64_SIZE};
use crate::memory::ll_allocator::LinkedListAllocator;
use crate::memory::page_tables::PageTableEntry;
use crate::memory::paging::virtual_to_physical;
use crate::{kprintln, kprintln_ok};
use core::alloc::Layout;
use core::ptr;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;

const DMA32_START: u64 = DMA64_START + DMA64_SIZE;
const DMA32_SIZE: u64 = 4 * 1_099_511_627_776; // 4tb

/// Exclusive upper bound for physical addresses of the DMA32 pool
pub const DMA32_PHYS_LIMIT: u64 = 1 << 32;

pub struct Dma32Manager {
    allocator: LinkedListAllocator,
}

impl Dma32Manager {
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

        self.allocator.init(DMA32_START as usize, DMA32_SIZE as usize);
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
                virtual_to_physical(virt).expect("DMA32 virtual address not mapped to physical");

            if phys.as_u64() + size as u64 > DMA32_PHYS_LIMIT {
                kprintln!(Error, "[DMA32] Allocation at phys {:#x} ({} bytes) is above 4 GiB, rejecting it.", phys.as_u64(), size);
                self.allocator.deallocate(ptr, layout);
                return None;
            }

            if !is_physically_contiguous(virt, phys, size) {
                kprintln!(Error, "[DMA32] Allocation at {:#x} ({} bytes) is not physically contiguous, rejecting it.", virt.as_u64(), size);
                self.allocator.deallocate(ptr, layout);
                return None;
            }

            Some(DmaAlloc::new(virt, phys, layout, DmaZone::Dma32))
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
    pub static ref DMA32_MANAGER: Mutex<Dma32Manager> = Mutex::new(Dma32Manager::new());
}

//==================================================================================================
// PUBLIC WRAPPERS
//==================================================================================================

pub fn dma32_init() {
    unsafe {
        DMA32_MANAGER.lock().init();
    }
    kprintln_ok!("Initialized DMA32 pool.");
}

pub fn dma32_alloc_coherent(size: usize, align: usize) -> Option<DmaAlloc> {
    DMA32_MANAGER.lock().alloc_coherent(size, align)
}

pub fn dma32_alloc_zeroed(size: usize, align: usize) -> Option<DmaAlloc> {
    if let Some(alloc) = dma32_alloc_coherent(size, align) {
        unsafe {
            ptr::write_bytes(alloc.virt.as_mut_ptr::<u8>(), 0, size);
        }
        Some(alloc)
    } else {
        None
    }}

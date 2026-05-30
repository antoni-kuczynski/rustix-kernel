#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 29/05/2026
 */
use core::ops::Add;
use crate::{print_fail_msg, vgaprintln, VGAWRITER};
use crate::ColorTextMode;
use core::ptr;
use spin::Mutex;
use core::sync::atomic::AtomicPtr;
use x86_64::{PhysAddr, VirtAddr};
use crate::memory::page_tables::{PageSize, PageTableEntry};
use crate::{print_ok_msg, vgaprint};
use crate::boot::cpuid::{CpuId, CPU_ID};
use crate::memory::align_up;
use crate::memory::paging::{vmm_map_range, vmm_map_range_ext};

const IOREMAP_START: u64 = 0xffff_e400_0000_0000;
const IOREMAP_LENGTH: u64 = 16 * 1_099_511_627_776; // 16tb
const IOREMAP_END: u64 = IOREMAP_START + IOREMAP_LENGTH;


//For now making this a bump allocator, i think its gonna be enough, may change it later
pub struct IoRemapManager {
    alloc_ptr: *mut u8,
    flags: u64
}

pub struct IoAlloc {
    phys: PhysAddr,
    virt_addr: VirtAddr,
    size: usize
}

impl IoRemapManager {
    const fn new() -> IoRemapManager {
        IoRemapManager {
            alloc_ptr: ptr::null_mut(),
            flags: PageTableEntry::WRITABLE | PageTableEntry::PRESENT | PageTableEntry::CACHE_DISABLE
        }
    }

    unsafe fn ioremap(&mut self, phys_addr: PhysAddr, size: u64, align: usize, flags: u64) {
        assert!(size > 0, "ioremap size must be > 0");
        assert!(align.is_power_of_two(), "ioremap align must be power of two");

        let start_phys = phys_addr.align_down(PageSize::SIZE_4KB);
        let start_ptr = VirtAddr::new(align_up(self.alloc_ptr as usize, PageSize::SIZE_4KB as usize) as u64);
        let page_size = (size + PageSize::SIZE_4KB) & !(PageSize::SIZE_4KB - 1);
        let end_ptr = start_ptr.add(page_size);

        if !vmm_map_range_ext(
            start_ptr,
            start_phys,
            page_size,
            &PageSize::Size4Kb,
            self.flags | flags //always use cache disabled and no execute if supported
        ) {
            //mapping the pages failed
            panic!(" [IOREMAP] Mapping at phys {:#011x}, size {}, align {} at virt {:#011x} failed.",
            start_phys, size, align, start_ptr)
        }
        self.alloc_ptr = end_ptr.as_mut_ptr::<u8>();
    }
}

unsafe impl Send for IoRemapManager {}

pub static IOREMAP_MANAGER: Mutex<IoRemapManager> = Mutex::new(IoRemapManager::new());

pub fn ioremap_init() {
    vgaprint!("Initializing ioremap...");

    if CpuId::has_xd() {
        IOREMAP_MANAGER.lock().flags |= PageTableEntry::NO_EXECUTE;
    }

    print_ok_msg!();
}

pub fn ioremap_permanent(phys_addr: PhysAddr, size: u64, align: usize) {
    unsafe {
        IOREMAP_MANAGER.lock().ioremap(phys_addr, size, align, 0);
    }
}

pub fn ioremap_ext_permanent(phys_addr: PhysAddr, size: u64, align: usize, flags: u64) {
    unsafe {
        IOREMAP_MANAGER.lock().ioremap(phys_addr, size, align, flags);
    }
}
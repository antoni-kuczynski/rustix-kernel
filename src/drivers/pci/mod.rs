use crate::drivers::pci::pci_device::PciDeviceInitError;
use crate::drivers::pci::pci_device::PciDeviceInitError::DmaAllocationFailure;
use crate::memory::dma::{DmaAlloc, dma_alloc_coherent};
use core::ops::Add;
use core::ptr;
use x86_64::VirtAddr;

pub mod pci;
pub mod pci_bar;
pub mod pci_device;
pub mod pci_io;

#[inline(always)]
pub unsafe fn mmio_read<T: Copy>(base: VirtAddr, offset: u64) -> T {
    unsafe { ptr::read_volatile(base.add(offset).as_ptr::<T>()) }
}

#[inline(always)]
pub unsafe fn mmio_write<T>(base: VirtAddr, offset: u64, value: T) {
    unsafe { ptr::write_volatile(base.add(offset).as_mut_ptr::<T>(), value) }
}

pub fn dma_alloc_zeroed(size: usize, align: usize) -> Result<DmaAlloc, PciDeviceInitError> {
    let alloc = dma_alloc_coherent(size, align).ok_or(DmaAllocationFailure)?;
    unsafe {
        ptr::write_bytes(alloc.virt.as_mut_ptr::<u8>(), 0, size);
    }
    Ok(alloc)
}

pub unsafe fn dma_as_mut<T>(alloc: &DmaAlloc) -> &'static mut T {
    unsafe { &mut *alloc.virt.as_mut_ptr::<T>() }
}

pub unsafe fn dma_as_slice_mut<T>(alloc: &DmaAlloc, len: usize) -> &'static mut [T] {
    unsafe { core::slice::from_raw_parts_mut(alloc.virt.as_mut_ptr::<T>(), len) }
}

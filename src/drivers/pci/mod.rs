use core::ops::Add;
use core::ptr;
use x86_64::VirtAddr;

pub mod pci;
pub mod pci_bar;
pub mod pci_device;
pub mod pci_io;
pub mod pci_quirks;
pub mod pci_msi;

#[inline(always)]
pub unsafe fn mmio_read<T: Copy>(base: VirtAddr, offset: u64) -> T {
    unsafe { ptr::read_volatile(base.add(offset).as_ptr::<T>()) }
}

#[inline(always)]
pub unsafe fn mmio_write<T>(base: VirtAddr, offset: u64, value: T) {
    unsafe { ptr::write_volatile(base.add(offset).as_mut_ptr::<T>(), value) }
}

use bootloader::{bootinfo::{FrameRange, MemoryMap, MemoryRegionType}, BootInfo};

use crate::vgaprintln;

#[repr(usize)]
enum MemoryMapIndex{
    KERNEL = 10,
}

#[derive(Debug)]
pub struct MemInfo {
    pub total : u64, // in Bytes
    pub usable: u64,
    pub reserved: u64,
    pub kernel: u64,
}

impl MemInfo{
    pub fn from(mem_map: &'static MemoryMap) -> Self{
        let mut total: u64 = 0;
        let mut usable: u64 = 0;
        let mut reserved: u64 = 0;
        let mut kernel: u64 = 0;

        for region in mem_map.iter() {
            let size = region.range.end_addr() - region.range.start_addr();
            total += size;
            match region.region_type {
                MemoryRegionType::Kernel        => kernel += size,
                MemoryRegionType::KernelStack   => kernel += size,
                MemoryRegionType::Usable        => usable += size,
                _                               => reserved += size,
            }
        }

        MemInfo {
            total,
            usable,
            reserved,
            kernel,
        }
    }
}

fn get_frame_range(boot_info: &'static BootInfo, index: MemoryMapIndex) -> FrameRange{
    boot_info.memory_map[index as usize].range
}

pub fn show_vitals(boot_info: &'static BootInfo){
    vgaprintln!("Kernel loaded at: {:?}",get_frame_range(boot_info, MemoryMapIndex::KERNEL));
    vgaprintln!("Physical mem offset: {:?}",boot_info.physical_memory_offset);
    vgaprintln!("Memory Info: ");
    print_meminfo(&MemInfo::from(&boot_info.memory_map));
}

fn print_size(label: &str, bytes: u64) {
    let gb = bytes / (1024 * 1024 * 1024);
    let mb = (bytes % (1024 * 1024 * 1024)) / (1024 * 1024);
    let kb = (bytes % (1024 * 1024)) / 1024;
    let b  = bytes % 1024;

    vgaprintln!("| {:<8} | {:>3} GB {:>3} MB {:>3} KB {:>3} B     |", label, gb, mb, kb, b);
}

pub fn print_meminfo(mem: &MemInfo) {
    vgaprintln!("+----------+--------------------------------+");
    vgaprintln!("| Field    | Value                          |");
    vgaprintln!("+----------+--------------------------------+");
    print_size("Total", mem.total);
    print_size("Usable", mem.usable);
    print_size("Reserved", mem.reserved);
    print_size("Kernel", mem.kernel);
    vgaprintln!("+----------+--------------------------------+");
}

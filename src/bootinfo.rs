use bootloader::{bootinfo::{FrameRange}, BootInfo};

use crate::vgaprintln;

#[repr(usize)]
enum MemoryMapIndex{
    KERNEL = 10,
}

fn get_frame_range(boot_info: &'static BootInfo, index: MemoryMapIndex) -> FrameRange{
    boot_info.memory_map[index as usize].range
}

pub fn show_vitals(boot_info: &'static BootInfo){
    vgaprintln!("Kernel loaded at: {:?}",get_frame_range(boot_info, MemoryMapIndex::KERNEL));
}

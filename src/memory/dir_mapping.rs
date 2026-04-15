/*
 * Created by Antoni Kuczyński
 * 15/04/2026
 */
use x86_64::VirtAddr;
use crate::boot::multiboot::{multiboot2_memory_map_tag, MULTIBOOT_INFO};
use crate::memory::SizeUnit;

const DIR_MAP_TOTAL_SIZE: u64 = 64 * 1_099_511_627_776; //64 terabytes
const DIR_MAP_START: VirtAddr = VirtAddr::new(0xffff_8080_0000_0000);
const DIR_MAP_END: VirtAddr = VirtAddr::new(0xffff_e080_0000_0000);


pub fn init() {
    unsafe {
        let high_addr = (*multiboot2_memory_map_tag().expect("no memory map tag provided!"))
            .get_high_usable_memory_address().as_u64();

        if high_addr > DIR_MAP_TOTAL_SIZE {
            panic!("Memory size > 64tb - yeah thats a little too much memory for me :(((((");
        }

        //TODO: allocate memory using huge pages (1gb)
        //TODO: check if huge pages are supported (CPUID) if not use 2mb ones
        //TODO: physical to virtual function

    }
}

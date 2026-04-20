#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 20/04/2026
 */
use x86_64::{PhysAddr, VirtAddr};
use crate::{vgaprint, VGAWRITER};
use crate::ColorTextMode;
use crate::{print_ok_msg, vgaprintln};
use crate::memory::paging::vmm_eba_map_page;

const KHEAP_START: u64 = 0xffff_c200_0000_0000;
const KHEAP_LENGTH: u64 = 16 * 1_099_511_627_776; // 16tb
pub const KHEAP_END: u64 = KHEAP_START + KHEAP_LENGTH;


pub fn kheap_init() {
    vgaprint!("Initializing kernel heap...");


    print_ok_msg!();
}
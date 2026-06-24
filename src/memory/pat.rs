/*
 * Created by Antoni Kuczyński
 * 24/06/2026
 */
use crate::asm::{rdmsr, wrmsr};
use crate::kprintln_ok;
use crate::memory::page_tables::PageSize;
/*
Number 	Name 	Description
0 	UC — Uncacheable 	All accesses are uncacheable. Write combining is not allowed. Speculative accesses are not allowed.
1 	WC — Write-Combining 	All accesses are uncacheable. Write combining is allowed. Speculative reads are allowed.
4 	WT — Writethrough 	Reads allocate cache lines on a cache miss. Cache lines are not allocated on a write miss.
    Write hits update the cache and main memory.
5 	WP — Write-Protect 	Reads allocate cache lines on a cache miss. All writes update main memory.
    Cache lines are not allocated on a write miss. Write hits invalidate the cache line and update main memory.
6 	WB — Writeback 	Reads allocate cache lines on a cache miss, and can allocate to either the shared,
    exclusive, or modified state. Writes allocate to the modified state on a cache miss.
7 	UC- — Uncached 	Same as uncacheable, except that this can be overriden by Write-Combining MTRRs.
 */

pub struct PatIndex;

impl PatIndex {
    ///Reads allocate cache lines on a cache miss, and can allocate to either the shared, exclusive, or modified state. Writes allocate to the modified state on a cache miss.
    pub const WRITE_BACK: u8 = 0;

    ///All accesses are uncacheable. Write combining is allowed. Speculative reads are allowed.
    pub const WRITE_COMBINING: u8 = 1;

    ///Reads allocate cache lines on a cache miss. Cache lines are not allocated on a write miss.
    pub const WRITE_THROUGH: u8 = 2;

    ///All accesses are uncacheable. Write combining is not allowed. Speculative accesses are not allowed.
    pub const UNCACHEABLE: u8 = 3;

    ///Reads allocate cache lines on a cache miss. All writes update main memory. Cache lines are not allocated on a write miss. Write hits invalidate the cache line and update main memory.
    pub const WRITE_PROTECT: u8 = 4;

    ///Same as uncacheable, except that this can be overriden by Write-Combining MTRRs.
    pub const UNCACHED: u8 = 5;
    pub const _WRITE_BACK1: u8 = 6;
    pub const _UNCACHED_1: u8 = 7;


    pub fn get_u64_page_flags(index: u8, page_table_size: PageSize) -> u64 {
        let mut flags = 0u64;

        if (index & 0x01) != 0 {
            flags |= 1 << 3;
        }

        if (index & 0x02) != 0 {
            flags |= 1 << 4;
        }

        if (index & 0x04) != 0 && page_table_size == PageSize::SIZE_4KB{
            flags |= 1 << 7;
        } else if (index & 0x04) != 0 && page_table_size != PageSize::SIZE_4KB{
            flags |= 1 << 12;
        }

        flags
    }
}

pub fn pat_init() {
    // WB - PA0 => 6
    // WC - PA1 => 1
    // WT - PA2 => 4
    // UC - PA3 => 0
    // WP - PA4 => 5
    //UC- - PA5 => 7
    // WB - PA6 => 6 (backup of PA0)
    // UC - PA7 => 0 (backup of PA3)

    let magic = (6u64 | (1u64 << 8) | (4u64 << 16) | (0u64 << 24) | (5u64 << 32) | (7u64 << 40) | (6u64 << 48) | (0u64 << 56));
    unsafe { wrmsr(0x277, magic) };
    // kprintln_ok!("Configured the Page Attribute Table.");
}
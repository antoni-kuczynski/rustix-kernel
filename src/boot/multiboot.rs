#![allow(dead_code)]
use crate::VGAWRITER;
use crate::ColorTextMode;
use core::arch::asm;
use core::cmp::PartialEq;
use core::ops::Add;
use core::ptr;
use core::ptr::read_volatile;
use crate::{print_ok_msg, vgaprint, vgaprintln};
use crate::memory::{kernel_end, SizeUnit, V2P};
use crate::memory::P2V;

/*
==============================================
SOURCES:
https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html
==============================================
 */

//==================================================================================================
//Multiboot information structures
//==================================================================================================
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootTagBase {
    tag_type: u32,
    size: u32
}
//==================================================================================================
//  MEMORY MAP
//==================================================================================================
/*
‘entry_size’ contains the size of one entry so that in future new fields may be added to it.
It’s guaranteed to be a multiple of 8. ‘entry_version’ is currently set at ‘0’.
Future versions will increment this field. Future version are guranteed to be backward compatible with older format.
 */
#[repr(C, packed)]
pub struct MultibootMemoryMapTag { //type = 6
    header: MultibootTagBase,
    entry_size: u32,
    entry_version: u32,
    entries: MultibootMemoryMapEntry
}

/*
‘size’ contains the size of current entry including this field itself.
It may be bigger than 24 bytes in future versions but is guaranteed to be ‘base_addr’ is the starting physical address.

‘length’ is the size of the memory region in bytes.

‘type’ is the variety of address range represented, where a
    value of 1 indicates available RAM,
    value of 3 indicates usable memory holding ACPI information,
    value of 4 indicates reserved memory which needs to be preserved on hibernation,
    value of 5 indicates a memory which is occupied by defective RAM modules and all other values currently indicated a reserved area.

‘reserved’ is set to ‘0’ by bootloader and must be ignored by the OS image.

The map provided is guaranteed to list all standard RAM that should be available for normal use.
This type however includes the regions occupied by kernel, mbi, segments and modules.
Kernel must take care not to overwrite these regions.

This tag may not be provided by some boot loaders on EFI platforms if EFI boot services are enabled and available
for the loaded image (EFI boot services not terminated tag exists in Multiboot2 information structure).
 */
#[repr(C, packed)]
pub(crate) struct MultibootMemoryMapEntry { //type = 6
    base_addr: u64,
    length: u64,
    addr_range_type: u32,
    _reserved: u32
}

#[derive(PartialEq)]
pub enum MemoryRegionType {
    AvailableRAM = 1,
    UsableAcpi = 3,
    HibernationPreserved = 4,
    DefectiveRAM = 5
}
//==================================================================================================
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootModulesTag {
    header: MultibootTagBase, //type = 3
    mod_start: u32,
    mod_end: u32,
    string: *const u8
}

//==================================================================================================
#[repr(C, packed)]
struct MultibootInfo {
    total_size: u32,
    _reserved: u32
}
//==================================================================================================
pub struct MultibootInfoView {
    base: &'static MultibootInfo,
    tags_size_bytes: usize,
    pub tags: *const u32,
    multiboot_end_logical: u64
}
//==================================================================================================

impl MultibootInfoView {
    pub fn init_multiboot_info_struct(original_address: u64, address_to_copy_to: u64) -> MultibootInfoView {
        unsafe {
            //---------------------------------------------------------
            //copy multiboot info struct to right after kernel code
            //---------------------------------------------------------
            let copied_addr = address_to_copy_to as *const u32; // address already guarenteed to be 8byte aligned
            vgaprintln!("Copying multiboot2 info struct to address {:#011x}...", copied_addr as u64);
            {
                let base = MultibootInfo::new(original_address);

                if base._reserved != 0x00 {
                    panic!("Multiboot info reserved value is not zero!");
                }

                let mut src_addr = base as *const MultibootInfo as *mut u8;
                let size = base.total_size;
                let mut target = copied_addr as *mut u8;
                for _i in 0..size {
                    *target = *src_addr;
                    *src_addr = 0x00u8;
                    src_addr = src_addr.add(1);
                    target = target.add(1);
                }
            }
            //---------------------------------------------------------
            let copied_base = MultibootInfo::new(copied_addr as u64);
            let tags_size_bytes = copied_base.total_size as usize - (2 * size_of::<u32>());
            let tags = copied_addr.add(2);
            print_ok_msg!(); // finished copying

            //---------------------------------------------------------
            // copy kernel modules right after multiboot info end
            //---------------------------------------------------------
            let mut modules_start_address = (
                (copied_base as *const MultibootInfo as u64 + copied_base.total_size as u64 + 0xFFF) & !0xFFF
            ) as *mut u8; // page aligned start address

            let mut view = Self {
                base: copied_base,
                tags_size_bytes,
                tags,
                multiboot_end_logical: 0 //temp value
            };

            vgaprintln!("Copying kernel modules to address {:#011x}...", modules_start_address as u64);

            let mut modules = view.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MODULES, view.tags);

            let mut copied_end_addr = (copied_base as *const MultibootInfo as *const u8).add((*copied_base).total_size as usize);
            while modules != None {
                let module: *mut MultibootModulesTag = modules.unwrap() as *mut MultibootModulesTag;
                let mut original_address = P2V((*module).mod_start() as u64) as *mut u8;
                let len = (*module).mod_end - (*module).mod_start;

                let mut copied_start_address = modules_start_address;
                copied_end_addr = copied_start_address.add(len as usize);


                //set the addresses in multiboot info struct
                (*module).mod_start = V2P(copied_start_address as u64) as u32;
                (*module).mod_end = V2P(copied_end_addr as u64) as u32;

                for i in (*module).mod_start()..(*module).mod_end() {
                    *copied_start_address = *original_address; // copy
                    *original_address = 0x00u8; // clear

                    original_address = original_address.add(1);
                    copied_start_address = copied_start_address.add(1);
                }

                modules_start_address = ((modules_start_address as u64 + (*module).header().size() as u64 + 0xFFF) & !0xFFF) as *mut u8;


                // (*module).print();
                let start_ptr = module.byte_add((((*module).header().size() + 7) & !0x7) as usize);
                modules = view.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MODULES, start_ptr as *const u32);
            }
            print_ok_msg!(); //copying kernel modules ok


            view.multiboot_end_logical = copied_end_addr as u64;

            view
        }
    }
//==================================================================================================
    pub fn get_tag_addr_by_type(&self, tag_type: u32, start_tag_addr: *const u32) -> Option<*const u32> {
        unsafe {
            let mut tags = start_tag_addr as *const MultibootTagBase;
            let tags_end = self.tags.byte_add(self.tags_size_bytes) as *const MultibootTagBase;

            while tags < tags_end {
                let tag_base = read_volatile(tags);
                let current_tag_type = tag_base.tag_type;
                let length = (tag_base.size as usize + 7) & !7;

                // vgaprintln!("{:#06x}", current_tag_type);

                if current_tag_type == 0x00 {
                    break;
                }

                if current_tag_type == tag_type {
                    return Some(tags as *const u32);
                }

                tags = tags.byte_add(length);
            }
            None
        }
    }
//==================================================================================================
    pub fn get_boot_loader_name(&self) -> Option<&str> {
        unsafe {
            let addr = match self.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_BOOTLOADER_NAME, self.tags) {
                Some(x) => x,
                None => return None
            };

            let strlen = *addr.add(1) as usize - (size_of::<u32>() * 2) - 1;
            let val = addr.add(2) as *mut u8;

            let slice = ptr::slice_from_raw_parts(val, strlen);
            match str::from_utf8(&*slice) {
                Ok(x) => Some(x),
                Err(_) => None
            }
        }
    }
//==================================================================================================
    pub fn get_memory_map_tag(&self) -> Option<*const MultibootMemoryMapTag> {
        match self.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MEMORY_MAP, self.tags) {
            Some(x) => Some(x as *const MultibootMemoryMapTag),
            None => None
        }
    }

    pub fn get_modules_tag(&self, search_start_addr: *const u32) -> Option<*const MultibootModulesTag> {
        let tag = self.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MODULES, search_start_addr);
        match tag {
            None => {None}
            Some(x) => {Some(x as *const MultibootModulesTag)}
        }
    }
//==================================================================================================
    pub fn print(&self) {
        unsafe {
            let total_size = self.base.total_size;
            let mut tags = self.tags as *const MultibootTagBase;
            let tags_end = tags.byte_add(self.tags_size_bytes);

            vgaprintln!("Multiboot info structure:");
            vgaprintln!("===================================");
            vgaprintln!("Total size: {}", total_size);
            vgaprintln!("Tags:");
            while tags < tags_end {
                let tag_base = read_volatile(tags);
                let tag_type = tag_base.tag_type;
                let length = (tag_base.size as usize + 7) & !7;

                vgaprintln!("{:#06x}", tag_type);

                if tag_type == 0x00 {
                    break
                }

                tags = tags.byte_add(length);
            }
            vgaprintln!("end");
        }
    }
//==================================================================================================
    pub fn get_multiboot_address_from_ebx() -> u32 {
        unsafe {
            let addr: u32;
            asm!(
            "mov {0:e}, ebx",
            out(reg) addr,
            );
            addr
        }
    }
//==================================================================================================
    pub fn base(&self) -> &'static MultibootInfo {
        self.base
    }

    pub fn tags_size_bytes(&self) -> usize {
        self.tags_size_bytes
    }

    pub fn tags(&self) -> *const u32 {
        self.tags
    }

    pub fn multiboot_end_logical(&self) -> u64 {
        self.multiboot_end_logical
    }
}
//==================================================================================================
impl MultibootInfo {
    fn new(addr: u64) -> &'static Self {
        unsafe {
            let ptr = addr as usize as *const MultibootInfo;

            match ptr.as_ref() {
                Some(x) => x,
                None => panic!("Could not reference MultibootInfo struct!")
            }
        }
    }
}
//==================================================================================================
impl MultibootTagBase {
    pub const MULTIBOOT_TAG_TYPE_BOOT_COMMAND_LINE: u32 = 1;
    pub const MULTIBOOT_TAG_TYPE_BOOTLOADER_NAME: u32 = 2;
    pub const MULTIBOOT_TAG_TYPE_MODULES: u32 = 3;
    pub const MULTIBOOT_TAG_TYPE_FLAGS: u32 = 4;
    pub const MULTIBOOT_TAG_TYPE_FRAMEBUFFER: u32 = 5;
    pub const MULTIBOOT_TAG_TYPE_MEMORY_MAP: u32 = 6;
    pub const MULTIBOOT_TAG_TYPE_VBE_INFO: u32 = 7;
    pub const MULTIBOOT_TAG_TYPE_FRAMEBUFFER_INFO: u32 = 8;
    pub const MULTIBOOT_TAG_TYPE_ELF_SYMBOLS: u32 = 9;
    pub const MULTIBOOT_TAG_TYPE_APM_TABLE: u32 = 10;
    pub const MULTIBOOT_TAG_TYPE_EFI_32_BIT_SYSTEM_TABLE_POINTER: u32 = 11;
    pub const MULTIBOOT_TAG_TYPE_EFI_64_BIT_SYSTEM_TABLE_POINTER: u32 = 12;
    pub const MULTIBOOT_TAG_TYPE_SMBIOS_TABLES: u32 = 13;
    pub const MULTIBOOT_TAG_TYPE_ACPI_OLD_RSDP: u32 = 14;
    pub const MULTIBOOT_TAG_TYPE_ACPI_NEW_RSDP: u32 = 15;
    pub const MULTIBOOT_TAG_TYPE_NETWORKING_INFORMATION: u32 = 16;
    pub const MULTIBOOT_TAG_TYPE_EFI_MEMORY_MAP: u32 = 17;
    pub const MULTIBOOT_TAG_TYPE_EFI_BOOT_SERVICES_NOT_TERMINATED: u32 = 18;
    pub const MULTIBOOT_TAG_TYPE_EFI_32_BIT_IMAGE_HANDLE_POINTER: u32 = 19;
    pub const MULTIBOOT_TAG_TYPE_EFI_64_BIT_IMAGE_HANDLE_POINTER: u32 = 20;
    pub const MULTIBOOT_TAG_TYPE_IMAGE_LOAD_BASE_PHYSICAL_ADDRESS: u32 = 21;

    pub fn tag_type(&self) -> u32 {
        self.tag_type
    }

    pub fn size(&self) -> u32 {
        self.size
    }
}

//==================================================================================================
//TODO: I absolutely despise of the code repetition here, it's so extremely disgusting it makes me wanna reconsider my life choices.
// For now it works so all is good, but for the love of god and all human beings looking at this abomination, change this!!!
// It doesnt even apply to just this code - this whole multiboot code is a pile of shit held together by tiny strings
// All these dumb pointer casts, code repetitions and braindead logic....
// So please, fix this
impl MultibootMemoryMapTag {
    /*
    ‘type’ is the variety of address range represented, where a
    value of 1 indicates available RAM,
    value of 3 indicates usable memory holding ACPI information,
    value of 4 indicates reserved memory which needs to be preserved on hibernation,
    value of 5 indicates a memory which is occupied by defective RAM modules and all other values currently indicated a reserved area.
     */
//==================================================================================================
    pub fn get_available_memory(&self, size_unit: SizeUnit) -> u64 {
        let mut mem_size: u64 = 0;
        unsafe {
            let size_entries = self.header.size - size_of::<MultibootMemoryMapTag>() as u32;
            let entry_length = self.entry_size;
            let entry_version = self.entry_version;

            assert_eq!(size_of::<MultibootMemoryMapEntry>(), entry_length as usize);    //should be 24 bytes
            assert_eq!(0, entry_version);   //should be 0

            //this sucks so badly
            let mut entry1 = (self as *const Self as *const u32).add(4) as *const MultibootMemoryMapEntry;
            let last = entry1.byte_add(size_entries as usize);

            while entry1 < last {
                let base_addr = (*entry1).base_addr;
                let length = (*entry1).length;

                let region_type = match MemoryRegionType::from_u32((*entry1).addr_range_type) {
                    None => {
                        entry1 = entry1.add(1);
                        continue
                    },   //invalid memory region so skip it
                    Some(x) => {x}
                };

                if region_type == MemoryRegionType::AvailableRAM {
                    mem_size += length;
                }

                entry1 = entry1.add(1);
            }
            mem_size / size_unit.as_usize() as u64
        }
    }
    //==================================================================================================
    pub fn get_high_usable_memory_address(&self) -> u64 {
        unsafe {
            let size_entries = self.header.size - size_of::<MultibootMemoryMapTag>() as u32;
            let entry_length = self.entry_size;
            let entry_version = self.entry_version;

            assert_eq!(size_of::<MultibootMemoryMapEntry>(), entry_length as usize);    //should be 24 bytes
            assert_eq!(0, entry_version);   //should be 0

            let mut entry1 = (self as *const Self as *const u32).add(4) as *const MultibootMemoryMapEntry;
            let last = entry1.byte_add(size_entries as usize);
            let mut max = 0x00u64;

            while entry1 < last {
                let addr = (*entry1).base_addr + (*entry1).length;

                if addr > max {
                    max  = addr;
                }

                entry1 = entry1.add(1);
            }
            max
        }
    }
//==================================================================================================
pub fn print_memory_map(&self) {
    unsafe {
        let size_entries = self.header.size - size_of::<MultibootMemoryMapTag>() as u32;
        let entry_length = self.entry_size;
        let entry_version = self.entry_version;

        assert_eq!(size_of::<MultibootMemoryMapEntry>(), entry_length as usize);    //should be 24 bytes
        assert_eq!(0, entry_version);   //should be 0

        let mut entry1 = (self as *const Self as *const u32).add(4) as *const MultibootMemoryMapEntry;
        let last = entry1.byte_add(size_entries as usize);

        while entry1 < last {
            let base_addr = (*entry1).base_addr;
            let length = (*entry1).length;
            let region_type = match MemoryRegionType::from_u32((*entry1).addr_range_type) {
                None => {
                    entry1 = entry1.add(1);
                    continue
                },   //invalid memory region so skip it
                Some(x) => { x }
            };

            if region_type != MemoryRegionType::AvailableRAM {
                entry1 = entry1.add(1);
                continue
            }

            // vgaprintln!("Memory map entry:");
            // vgaprintln!("========================");
            vgaprintln!("Base addr: {:#011x}, Length: {:#011x}", base_addr, length);
            // vgaprintln!("Length: {:#011x}", length);
            // vgaprintln!("Region type: {:#06x}", region_type);
            // vgaprintln!("========================");

            entry1 = entry1.add(1);
        }
    }
}
    //==================================================================================================
    pub fn print(&self) {
        let tag_type = self.header.tag_type;
        let tag_size = self.header.size;
        let entry_size = self.entry_size;
        let entry_version = self.entry_version;

        vgaprintln!("Multiboot memory map tag:");
        vgaprintln!("===================================");
        vgaprintln!("Type: {:#02x}", tag_type);
        vgaprintln!("Size: {}", tag_size);
        vgaprintln!("Entry size: {}", entry_size);
        vgaprintln!("Entry version: {}", entry_version);
        vgaprintln!("===================================");
        self.print_memory_map();
    }

    pub fn header(&self) -> MultibootTagBase {
        self.header
    }

    pub fn entry_size(&self) -> u32 {
        self.entry_size
    }

    pub fn entry_version(&self) -> u32 {
        self.entry_version
    }

    pub fn entries(&self) -> &MultibootMemoryMapEntry {
        &self.entries
    }
}
//==================================================================================================
impl MultibootMemoryMapEntry {
    pub fn base_addr(&self) -> u64 {
        self.base_addr
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn addr_range_type(&self) -> u32 {
        self.addr_range_type
    }

    pub fn _reserved(&self) -> u32 {
        self._reserved
    }
}
//==================================================================================================
impl MultibootModulesTag {
    pub fn print(&self) {
        unsafe {
            let tag_type = self.header.tag_type;
            let tag_size = self.header.size;
            let mod_start = self.mod_start;
            let mod_end = self.mod_end;
            let mut str = self.string;

            vgaprintln!("===================================");
            vgaprintln!("Multiboot modules tag:");
            vgaprintln!("===================================");
            vgaprintln!("Type: {:#02x}", tag_type);
            vgaprintln!("Size: {}", tag_size);
            vgaprintln!("Mod start: {:#011x}", mod_start);
            vgaprintln!("Mod end: {:#011x}", mod_end);
            vgaprintln!("===================================");
        }
    }

    pub fn header(&self) -> MultibootTagBase {
        self.header
    }

    pub fn mod_start(&self) -> u32 {
        self.mod_start
    }

    pub fn mod_end(&self) -> u32 {
        self.mod_end
    }

    pub fn string(&self) -> *const u8 {
        self.string
    }
}


#[allow(dead_code)]
impl MemoryRegionType {
    const ADDR_RANGE_TYPE_AVAILABLE_RAM: u32 = 1;
    const ADDR_RANGE_TYPE_USABLE_ACPI: u32 = 3;
    const ADDR_RANGE_TYPE_RESERVED_HIBERNATION: u32 = 4;
    const ADDR_RANGE_TYPE_DEFECTIVE_RAM: u32 = 5;

    pub(crate) fn from_u32(val: u32) -> Option<Self> {
        match val {
            Self::ADDR_RANGE_TYPE_AVAILABLE_RAM => {
                Some(Self::AvailableRAM)
            },
            Self::ADDR_RANGE_TYPE_USABLE_ACPI => {
                Some(Self::UsableAcpi)
            },
            Self::ADDR_RANGE_TYPE_RESERVED_HIBERNATION => {
                Some(Self::HibernationPreserved)
            },
            Self::ADDR_RANGE_TYPE_DEFECTIVE_RAM => {
                Some(Self::DefectiveRAM)
            },
            _ => {
                None
            }
        }
    }

    fn to_u32(&self) -> u32 {
        match self {
            Self::AvailableRAM => Self::ADDR_RANGE_TYPE_AVAILABLE_RAM,
            Self::UsableAcpi => Self::ADDR_RANGE_TYPE_USABLE_ACPI,
            Self::HibernationPreserved => Self::ADDR_RANGE_TYPE_RESERVED_HIBERNATION,
            Self::DefectiveRAM => Self::ADDR_RANGE_TYPE_DEFECTIVE_RAM
        }
    }
}


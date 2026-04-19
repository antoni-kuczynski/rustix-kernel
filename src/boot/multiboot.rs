#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
use crate::{__oldMultibootPhysAddr, earlyHeapEnd, vgaprint, VGAWRITER};
use crate::ColorTextMode;
use core::cmp::PartialEq;
use core::ptr;
use core::ptr::read_volatile;
use spin::{Once};
use x86_64::{PhysAddr, VirtAddr};
use crate::{print_ok_msg, vgaprintln};
use crate::memory::{SizeUnit, _V2P_kernel, KERNEL_VIRT_BASE};
use crate::memory::_P2V_kernel;
use crate::memory::page_tables::{PageSize};
use crate::memory::paging::{early_unmap_page, eba_map_page, eba_map_range};
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
pub struct MultibootMemoryMapEntry { //type = 6
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
pub struct MultibootInfo {
    total_size: u32,
    _reserved: u32
}
//==================================================================================================
pub struct MultibootInfoView {
    base: &'static MultibootInfo,
    tags_size_bytes: usize,
    tags: *const u32,
    multiboot_end_logical: u64
}
//==================================================================================================

impl MultibootInfoView {
    fn empty() -> Self {
        Self {
            base: &MultibootInfo { total_size: 0, _reserved: 0 },
            tags_size_bytes: 0,
            tags: Default::default(),
            multiboot_end_logical: 0,
        }
    }

    pub unsafe fn init_multiboot_info_struct() -> MultibootInfoView {
        let original_virt_address = _P2V_kernel(__oldMultibootPhysAddr as u64);
        let original_aligned = original_virt_address & !(PageSize::SIZE_2MB - 1);
        let virt_address_to_copy_to = _P2V_kernel((earlyHeapEnd + PageSize::SIZE_2MB - 1) & !(PageSize::SIZE_2MB - 1));

        //map original struct
        eba_map_page(
            VirtAddr::new_truncate(original_aligned),
            PhysAddr::new_truncate(_V2P_kernel(original_aligned)),
            &PageSize::Size2Mb
        );

        let length_bytes = read_volatile(original_virt_address as *const u32) as u64;

        eba_map_range(
            VirtAddr::new_truncate(original_aligned + PageSize::SIZE_2MB),
            PhysAddr::new_truncate(_V2P_kernel(original_aligned + PageSize::SIZE_2MB)),
            length_bytes,
            &PageSize::Size2Mb
        );
        eba_map_range(
            VirtAddr::new_truncate(virt_address_to_copy_to),
            PhysAddr::new_truncate(_V2P_kernel(virt_address_to_copy_to)),
            length_bytes,
            &PageSize::Size2Mb
        );

        //copy mb struct
        vgaprint!("Initializing multiboot2 and modules...");
        Self::copy_mb_struct(original_virt_address, virt_address_to_copy_to);

        let copied_base = &*(virt_address_to_copy_to as *const MultibootInfo);

        //copy modules
        let modules_start_address = ((virt_address_to_copy_to + copied_base.total_size as u64 + PageSize::SIZE_2MB) & !(PageSize::SIZE_2MB - 1)) as *mut u8;
        let tags = (virt_address_to_copy_to as *const u32).add(2);

        let mut view = Self {
            base: &*copied_base,
            tags_size_bytes: copied_base.total_size as usize - (2 * size_of::<u32>()),
            tags,
            multiboot_end_logical: 0,
        };

        let multiboot_end = Self::copy_modules(copied_base, modules_start_address, &mut view);
        view.multiboot_end_logical = multiboot_end as u64;

        //unmap original
        Self::unmap_mb_region(original_aligned, virt_address_to_copy_to, length_bytes);

        print_ok_msg!();
        view
    }

    unsafe fn copy_modules(
        copied_base: &MultibootInfo,
        start_dst: *mut u8,
        view: &mut MultibootInfoView
    ) -> *const u8 {
        let mut modules = view.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MODULES, view.tags);

        let mut current_dst = start_dst;
        let mut final_end_addr = (copied_base as *const _ as *const u8).add(copied_base.total_size as usize);

        while let Some(module_ptr) = modules {
            let module = &mut *(module_ptr as *mut MultibootModulesTag);

            let original_src = _P2V_kernel(module.mod_start() as u64) as *mut u8;
            let module_len = (module.mod_end - module.mod_start) as u64;

            //map src and destination regions
            eba_map_range(
                VirtAddr::new_truncate(original_src as u64),
                PhysAddr::new_truncate(_V2P_kernel(original_src as u64)),
                module_len,
                &PageSize::Size2Mb
            );
            eba_map_range(
                VirtAddr::new_truncate(current_dst as u64),
                PhysAddr::new_truncate(_V2P_kernel(current_dst as u64)),
                module_len,
                &PageSize::Size2Mb
            );

            let copied_end = current_dst.add(module_len as usize);

            //update mod addresses in mb struct
            module.mod_start = _V2P_kernel(current_dst as u64) as u32;
            module.mod_end = _V2P_kernel(copied_end as u64) as u32;

            //copy and clear original
            for i in 0..module_len as usize {
                ptr::write_volatile(current_dst.add(i), read_volatile(original_src.add(i)));
                ptr::write_volatile(original_src.add(i), 0);
            }

            //unmap the module
            Self::unmap_mb_region(original_src as u64, current_dst as u64, module_len);

            final_end_addr = copied_end;
            current_dst = ((copied_end as u64 + 0xFFF) & !0xFFF) as *mut u8; // Wyrównanie do 4KB

            //next tag
            let next_tag_ptr = (module as *const _ as *const u8).add(((module.header().size() + 7) & !0x7) as usize);
            modules = view.get_tag_addr_by_type(MultibootTagBase::MULTIBOOT_TAG_TYPE_MODULES, next_tag_ptr as *const u32);
        }

        final_end_addr
    }

    unsafe fn copy_mb_struct(original_virt: u64, copied_virt: u64) {
        let base = &*(original_virt as *const MultibootInfo);
        if base._reserved != 0x00 {
            panic!("Multiboot info reserved value is not zero!");
        }

        let src = original_virt as *mut u8;
        let dst = copied_virt as *mut u8;

        for i in 0..base.total_size as usize {
            ptr::write_volatile(dst.add(i), read_volatile(src.add(i)));
            ptr::write_volatile(src.add(i), 0);
        }
    }


    unsafe fn unmap_mb_region(original_virt: u64, copied_virt: u64, length: u64) {
        let mut offset = 0;
        while offset <= length {
            let original = original_virt + offset;
            let copied = copied_virt + offset;

            //unmap only if the addresses are not the same and they do not cover kernel / eba region
            if original != copied && !Self::is_page_inside_kernel_or_eba_regions(original) {
                early_unmap_page(VirtAddr::new_truncate(original), &PageSize::Size2Mb);
            }
            offset += PageSize::SIZE_2MB;
        }
    }


    fn is_page_inside_kernel_or_eba_regions(virt_addr: u64) -> bool {
        if virt_addr >= KERNEL_VIRT_BASE && virt_addr <= unsafe { _P2V_kernel(earlyHeapEnd) } {
            return true;
        }
        false
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

                if tag_type == 0x00 {
                    break
                }

                tags = tags.byte_add(length);
            }
            vgaprintln!("end");
        }
    }
//==================================================================================================
    pub fn base(&self) -> &'static MultibootInfo {
        self.base
    }

    pub fn length(&self) -> u32 {
        self.base().total_size
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
    pub fn get_high_usable_memory_address(&self) -> PhysAddr {
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
            PhysAddr::new_truncate(max)
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
        let tag_type = self.header.tag_type;
        let tag_size = self.header.size;
        let mod_start = self.mod_start;
        let mod_end = self.mod_end;

        vgaprintln!("===================================");
        vgaprintln!("Multiboot modules tag:");
        vgaprintln!("===================================");
        vgaprintln!("Type: {:#02x}", tag_type);
        vgaprintln!("Size: {}", tag_size);
        vgaprintln!("Mod start: {:#011x}", mod_start);
        vgaprintln!("Mod end: {:#011x}", mod_end);
        vgaprintln!("===================================");
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

    pub fn from_u32(val: u32) -> Option<Self> {
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
//==================================================================================================
// the struct is read only (well, except the init part at least)
// so this is already thread safe so this should be fine i guess
unsafe impl Send for MultibootInfoView {}

unsafe impl Sync for MultibootInfoView {}

pub static MULTIBOOT_INFO: Once<MultibootInfoView> = Once::new();

pub fn multiboot2_init() {
    unsafe {
        let view = MultibootInfoView::init_multiboot_info_struct();

        MULTIBOOT_INFO.call_once(|| view);
    }
}

pub fn multiboot2_memory_map_tag() -> Option<*const MultibootMemoryMapTag> {
    let info = MULTIBOOT_INFO.get().expect("Multiboot was not initialized yet!");
    info.get_memory_map_tag()
}

pub fn multiboot2_modules_tag(search_start_addr: *const u32) -> Option<*const MultibootModulesTag> {
    let info = MULTIBOOT_INFO.get().expect("Multiboot was not initialized yet!");
    info.get_modules_tag(search_start_addr)
}

pub fn multiboot2_bootloader_name() -> Option<&'static str> {
    let info = MULTIBOOT_INFO.get().expect("Multiboot was not initialized yet!");
    info.get_boot_loader_name()
}

pub fn multiboot2_logical_end() -> VirtAddr {
    let info = MULTIBOOT_INFO.get().expect("Multiboot was not initialized yet!");
    VirtAddr::new(info.multiboot_end_logical)
}

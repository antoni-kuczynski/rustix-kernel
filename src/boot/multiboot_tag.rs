#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::ptr::null_mut;
use core::slice;
use crate::drivers::acpi::tables::rsdp::{RSDP, XSDP};

//==================================================================================================
//Multiboot information structures
//==================================================================================================
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootTagBase {
    pub(crate) tag_type: u32,
    pub(crate) size: u32,
}
//==================================================================================================
pub struct MultibootBootloaderName(pub(crate) u8); // type = 2

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootModulesTag {
    pub(crate) header: MultibootTagBase, //type = 3
    pub(crate) mod_start: u32,
    pub(crate) mod_end: u32,
    pub(crate) string: *const u8,
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
pub struct MultibootMemoryMapTag {
    //type = 6
    pub(crate) header: MultibootTagBase,
    pub(crate) entry_size: u32,
    pub(crate) entry_version: u32,
    pub(crate) entries: MultibootMemoryMapEntry,
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
pub struct MultibootMemoryMapEntry {
    //type = 6
    pub(crate) base_addr: u64,
    pub(crate) length: u64,
    pub(crate) addr_range_type: u32,
    _reserved: u32,
}

#[derive(PartialEq)]
pub enum MemoryRegionType {
    AvailableRAM = 1,
    UsableAcpi = 3,
    HibernationPreserved = 4,
    DefectiveRAM = 5,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MultibootTagType {
    BootCommandLine = 1,
    BootloaderName = 2,
    Modules = 3,
    Flags = 4,
    Framebuffer = 5,
    MemoryMap = 6,
    VbeInfo = 7,
    FramebufferInfo = 8,
    ElfSymbols = 9,
    ApmTable = 10,
    Efi32BitSystemTablePointer = 11,
    Efi64BitSystemTablePointer = 12,
    SmbiosTables = 13,
    AcpiOldRsdp = 14,
    AcpiNewRsdp = 15,
    NetworkingInformation = 16,
    EfiMemoryMap = 17,
    EfiBootServicesNotTerminated = 18,
    Efi32BitImageHandlePointer = 19,
    Efi64BitImageHandlePointer = 20,
    ImageLoadBasePhysicalAddress = 21,
}

impl TryFrom<u32> for MultibootTagType {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::BootCommandLine),
            2 => Ok(Self::BootloaderName),
            3 => Ok(Self::Modules),
            4 => Ok(Self::Flags),
            5 => Ok(Self::Framebuffer),
            6 => Ok(Self::MemoryMap),
            7 => Ok(Self::VbeInfo),
            8 => Ok(Self::FramebufferInfo),
            9 => Ok(Self::ElfSymbols),
            10 => Ok(Self::ApmTable),
            11 => Ok(Self::Efi32BitSystemTablePointer),
            12 => Ok(Self::Efi64BitSystemTablePointer),
            13 => Ok(Self::SmbiosTables),
            14 => Ok(Self::AcpiOldRsdp),
            15 => Ok(Self::AcpiNewRsdp),
            16 => Ok(Self::NetworkingInformation),
            17 => Ok(Self::EfiMemoryMap),
            18 => Ok(Self::EfiBootServicesNotTerminated),
            19 => Ok(Self::Efi32BitImageHandlePointer),
            20 => Ok(Self::Efi64BitImageHandlePointer),
            21 => Ok(Self::ImageLoadBasePhysicalAddress),
            _ => Err(value),
        }
    }
}
//==================================================================================================
/*
3.6.12 Framebuffer info

        +--------------------+
u32     | type = 8           |
u32     | size               |
u64     | framebuffer_addr   |
u32     | framebuffer_pitch  |
u32     | framebuffer_width  |
u32     | framebuffer_height |
u8      | framebuffer_bpp    |
u8      | framebuffer_type   |
u8      | reserved           |
varies  | color_info         |
        +--------------------+

The field ‘framebuffer_addr’ contains framebuffer physical address.
This field is 64-bit wide but bootloader should set it under 4GiB if possible for compatibility with payloads which aren’t aware of PAE or amd64.
The field ‘framebuffer_pitch’ contains pitch in bytes. The fields ‘framebuffer_width’, ‘framebuffer_height’ contain framebuffer dimensions in pixels.
The field ‘framebuffer_bpp’ contains number of bits per pixel. ‘reserved’ always contains 0 in
current version of specification and must be ignored by OS image. If ‘framebuffer_type’ is set to 0 it means indexed color.
In this case color_info is defined as follows:

        +----------------------------------+
u32     | framebuffer_palette_num_colors   |
varies  | framebuffer_palette              |
        +----------------------------------+

‘framebuffer_palette’ is an array of colour descriptors. Each colour descriptor has following structure:

        +-------------+
u8      | red_value   |
u8      | green_value |
u8      | blue_value  |
        +-------------+

If ‘framebuffer_type’ is set to ‘1’ it means direct RGB color. Then color_type is defined as follows:

       +----------------------------------+
u8     | framebuffer_red_field_position   |
u8     | framebuffer_red_mask_size        |
u8     | framebuffer_green_field_position |
u8     | framebuffer_green_mask_size      |
u8     | framebuffer_blue_field_position  |
u8     | framebuffer_blue_mask_size       |
       +----------------------------------+

If ‘framebuffer_type’ is set to ‘2’ it means EGA text. In this case ‘framebuffer_width’ and ‘framebuffer_height’ are expressed in characters and not in pixels. ‘framebuffer_bpp’ is equal 16 (16 bits per character) and ‘framebuffer_pitch’ is expressed in bytes per text line. All further values of ‘framebuffer_type’ are reserved for future expansion
 */
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MultibootFramebufferInfoTag {
    //type = 8
    pub(crate) header: MultibootTagBase,
    pub(crate) framebuffer_addr: u64,
    pub(crate) framebuffer_pitch: u32,
    pub(crate) framebuffer_width: u32,
    pub(crate) framebuffer_height: u32,
    pub(crate) framebuffer_bpp: u8,
    pub(crate) framebuffer_type: u8,
    _reserved: u8,
}

impl MultibootFramebufferInfoTag {
    #[inline(always)]
    pub unsafe fn color_info_ptr(&self) -> *const u8 {
        (self as *const Self as *const u8)
            .add(size_of::<MultibootFramebufferInfoTag>() + 1)
    }
    pub unsafe fn color_info(&self) -> MultibootFramebufferColorInfo<'_> {
        match self.framebuffer_type {
            FRAMEBUFFER_TYPE_INDEXED => {
                let info_ptr =
                    self.color_info_ptr() as *const MultibootFbColorInfo;

                let num_colors =
                    core::ptr::addr_of!((*info_ptr).framebuffer_palette_num_colors)
                        .read_unaligned();

                let palette_ptr = (info_ptr as *const u8)
                    .add(core::mem::size_of::<u32>())
                    as *const MultibootFbColorDescriptor;

                let palette = slice::from_raw_parts(
                    palette_ptr,
                    num_colors as usize,
                );

                MultibootFramebufferColorInfo::Indexed {
                    num_colors,
                    palette,
                }
            }

            FRAMEBUFFER_TYPE_RGB => {
                let info_ptr =
                    self.color_info_ptr() as *const MultibootFramebufferRgbInfo;

                let info = core::ptr::read_unaligned(info_ptr);

                MultibootFramebufferColorInfo::Rgb {
                    info,
                }
            }

            FRAMEBUFFER_TYPE_EGA_TEXT => {
                MultibootFramebufferColorInfo::EgaText
            }

            other => {
                MultibootFramebufferColorInfo::Unknown(other)
            }
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct MultibootFbColorInfo {
    framebuffer_palette_num_colors: u32,
    pallete_start: [u8; 0]
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct MultibootFbColorDescriptor {
    red_value: u8,
    green_value: u8,
    blue_value: u8
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MultibootFramebufferRgbInfo {
    pub red_pos: u8,
    pub red_mask_size: u8,
    pub green_pos: u8,
    pub green_mask_size: u8,
    pub blue_pos: u8,
    pub blue_mask_size: u8,
}

pub enum MultibootFramebufferColorInfo<'a> {
    Indexed {
        num_colors: u32,
        palette: &'a [MultibootFbColorDescriptor],
    },
    Rgb {
        info: MultibootFramebufferRgbInfo,
    },
    EgaText,
    Unknown(u8),
}

const FRAMEBUFFER_TYPE_INDEXED: u8 = 0;
const FRAMEBUFFER_TYPE_RGB: u8 = 1;
const FRAMEBUFFER_TYPE_EGA_TEXT: u8 = 2;

//==================================================================================================
//  ACPI STUFF
//==================================================================================================
/*
3.6.16 ACPI old RSDP

        +-------------------+
u32     | type = 14         |
u32     | size              |
        | copy of RSDPv1    |
        +-------------------+

This tag contains a copy of RSDP as defined per ACPI 1.0 specification.
3.6.17 ACPI new RSDP

        +-------------------+
u32     | type = 15         |
u32     | size              |
        | copy of RSDPv2    |
        +-------------------+

This tag contains a copy of RSDP as defined per ACPI 2.0 or later specification.
 */
#[repr(C, packed)]
pub struct MultibootOldRsdpTag {
    //type = 14
    pub(crate) header: MultibootTagBase,
    pub(crate) copy_of_rsdp: RSDP,
}

#[repr(C, packed)]
pub struct MultibootNewRsdpTag {
    //type = 15
    pub(crate) header: MultibootTagBase,
    pub(crate) copy_of_xsdp: XSDP,
}

pub trait MultibootTagStruct {
    const TAG_TYPE: u32;
}

pub fn mb_tag_as_u32<T: MultibootTagStruct>() -> u32 {
    T::TAG_TYPE
}

impl MultibootTagStruct for MultibootBootloaderName {
    const TAG_TYPE: u32 = 2;
}

impl MultibootTagStruct for MultibootModulesTag {
    const TAG_TYPE: u32 = 3;
}

impl MultibootTagStruct for MultibootMemoryMapTag {
    const TAG_TYPE: u32 = 6;
}

impl MultibootTagStruct for MultibootFramebufferInfoTag {
    const TAG_TYPE: u32 = 8;
}

// ===== ACPI =====
impl MultibootTagStruct for MultibootOldRsdpTag {
    const TAG_TYPE: u32 = 14;
}

impl MultibootTagStruct for MultibootNewRsdpTag {
    const TAG_TYPE: u32 = 15;
}

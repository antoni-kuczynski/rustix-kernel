#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]


//==================================================================================================
//Multiboot information structures
//==================================================================================================
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootTagBase {
    pub(crate) tag_type: u32,
    pub(crate) size: u32
}
//==================================================================================================
pub struct MultibootBootloaderName(pub(crate) u8); // type = 2


#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootModulesTag {
    pub(crate) header: MultibootTagBase, //type = 3
    pub(crate) mod_start: u32,
    pub(crate) mod_end: u32,
    pub(crate) string: *const u8
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
    pub(crate) header: MultibootTagBase,
    pub(crate) entry_size: u32,
    pub(crate) entry_version: u32,
    pub(crate) entries: MultibootMemoryMapEntry
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
    pub(crate) base_addr: u64,
    pub(crate) length: u64,
    pub(crate) addr_range_type: u32,
    _reserved: u32
}

#[derive(PartialEq)]
pub enum MemoryRegionType {
    AvailableRAM = 1,
    UsableAcpi = 3,
    HibernationPreserved = 4,
    DefectiveRAM = 5
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


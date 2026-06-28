#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 30/05/2026
 */
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;
use alloc::vec::Vec;
use x86_64::VirtAddr;
use crate::kprintln;

#[repr(C, packed)]
pub struct Madt {
    pub header: ACPISDTHeader,
    pub local_apic_address: u32,
    pub flags: u32,
    //after flags entries start
}

impl AcpiSdtTable for Madt {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::MADT
    }

    fn validate(&self) -> bool {
        self.header.validate_checksum()
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

// MADT entry types

pub const MADT_ENTRY_TYPE_LOCAL_APIC: u8 = 0;
pub const MADT_ENTRY_TYPE_IO_APIC: u8 = 1;
pub const MADT_ENTRY_TYPE_INTERRUPT_SOURCE_OVERRIDE: u8 = 2;
pub const MADT_ENTRY_TYPE_NMI_SOURCE: u8 = 3;
pub const MADT_ENTRY_TYPE_LOCAL_APIC_NMI: u8 = 4;
pub const MADT_ENTRY_TYPE_LOCAL_APIC_ADDRESS_OVERRIDE: u8 = 5;
pub const MADT_ENTRY_TYPE_LOCAL_X2APIC: u8 = 9;

// MADT Local APIC / x2APIC flags

pub const MADT_APIC_FLAG_PROCESSOR_ENABLED: u32 = 1 << 0;
pub const MADT_APIC_FLAG_ONLINE_CAPABLE: u32 = 1 << 1;

#[inline]
pub fn madt_cpu_can_be_enabled(flags: u32) -> bool {
    flags & (MADT_APIC_FLAG_PROCESSOR_ENABLED | MADT_APIC_FLAG_ONLINE_CAPABLE) != 0
}

#[inline]
pub fn madt_cpu_is_enabled(flags: u32) -> bool {
    flags & MADT_APIC_FLAG_PROCESSOR_ENABLED != 0
}

#[inline]
pub fn madt_cpu_is_online_capable(flags: u32) -> bool {
    flags & MADT_APIC_FLAG_ONLINE_CAPABLE != 0
}

// MADT interrupt flags layout:
// bits 0..1 = polarity
// bits 2..3 = trigger mode

pub const MADT_INTERRUPT_POLARITY_MASK: u16 = 0b0000_0011;
pub const MADT_INTERRUPT_TRIGGER_MASK: u16 = 0b0000_1100;

// Polarity values

pub const MADT_INTERRUPT_POLARITY_CONFORMS: u16 = 0b00;
pub const MADT_INTERRUPT_POLARITY_ACTIVE_HIGH: u16 = 0b01;
pub const MADT_INTERRUPT_POLARITY_RESERVED: u16 = 0b10;
pub const MADT_INTERRUPT_POLARITY_ACTIVE_LOW: u16 = 0b11;

// Trigger mode values

pub const MADT_INTERRUPT_TRIGGER_CONFORMS: u16 = 0b00 << 2;
pub const MADT_INTERRUPT_TRIGGER_EDGE: u16 = 0b01 << 2;
pub const MADT_INTERRUPT_TRIGGER_RESERVED: u16 = 0b10 << 2;
pub const MADT_INTERRUPT_TRIGGER_LEVEL: u16 = 0b11 << 2;

#[inline]
pub fn madt_interrupt_polarity(flags: u16) -> u16 {
    flags & MADT_INTERRUPT_POLARITY_MASK
}

#[inline]
pub fn madt_interrupt_trigger_mode(flags: u16) -> u16 {
    flags & MADT_INTERRUPT_TRIGGER_MASK
}

#[inline]
pub fn madt_interrupt_is_active_low(flags: u16) -> bool {
    madt_interrupt_polarity(flags) == MADT_INTERRUPT_POLARITY_ACTIVE_LOW
}

#[inline]
pub fn madt_interrupt_is_active_high(flags: u16) -> bool {
    madt_interrupt_polarity(flags) == MADT_INTERRUPT_POLARITY_ACTIVE_HIGH
}

#[inline]
pub fn madt_interrupt_is_level_triggered(flags: u16) -> bool {
    madt_interrupt_trigger_mode(flags) == MADT_INTERRUPT_TRIGGER_LEVEL
}

#[inline]
pub fn madt_interrupt_is_edge_triggered(flags: u16) -> bool {
    madt_interrupt_trigger_mode(flags) == MADT_INTERRUPT_TRIGGER_EDGE
}

pub const MADT_ENTRY_HEADER_SIZE: usize = 2;

pub const MADT_LOCAL_APIC_ENTRY_SIZE: usize = 8;
pub const MADT_IO_APIC_ENTRY_SIZE: usize = 12;
pub const MADT_INTERRUPT_SOURCE_OVERRIDE_ENTRY_SIZE: usize = 10;
pub const MADT_NMI_SOURCE_ENTRY_SIZE: usize = 10;
pub const MADT_LOCAL_APIC_NMI_ENTRY_SIZE: usize = 6;
pub const MADT_LOCAL_APIC_ADDRESS_OVERRIDE_ENTRY_SIZE: usize = 12;
pub const MADT_LOCAL_X2APIC_ENTRY_SIZE: usize = 16;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadtEntryType {
    LocalApic = 0,
    IoApic = 1,
    InterruptSourceOverride = 2,
    NmiSource = 3,
    LocalApicNmi = 4,
    LocalApicAddressOverride = 5,
    LocalX2Apic = 9,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

//==================================================================================================
// TYPE 0 - LOCAL APIC
//==================================================================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtLocalApic {
    pub header: MadtEntryHeader,
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

impl MadtLocalApic {
    pub const MADT_LOCAL_APIC_ENABLED: u32 = 1 << 0;
    pub const MADT_LOCAL_APIC_ONLINE_CAPABLE: u32 = 1 << 1;
}
//==================================================================================================
// TYPE 1 - I/O APIC
//==================================================================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtIoApic {
    pub header: MadtEntryHeader,
    pub io_apic_id: u8,
    pub reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}
//==================================================================================================
// TYPE 2 - INTERRUPT SOURCE OVERRIDE
//==================================================================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtInterruptSourceOverride {
    pub header: MadtEntryHeader,
    pub bus_source: u8,
    pub irq_source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}
//==================================================================================================
// TYPE 5 - LOCAL APIC ADDRESS OVERRIDE
//==================================================================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtLocalApicAddressOverride {
    pub header: MadtEntryHeader,
    pub reserved: u16,
    pub local_apic_address: u64,
}
//==================================================================================================
// TYPE 9 - LOCAL X2 APIC
//==================================================================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MadtLocalX2Apic {
    pub header: MadtEntryHeader,
    pub reserved: u16,
    pub x2apic_id: u32,
    pub flags: u32,
    pub acpi_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MadtCpuApic {
    pub acpi_processor_id: u32,
    pub apic_id: u32,
    pub flags: u32,
    pub is_x2apic: bool,
}

impl MadtCpuApic {
    pub fn enabled(&self) -> bool {
        madt_cpu_is_enabled(self.flags)
    }

    pub fn online_capable(&self) -> bool {
        madt_cpu_is_online_capable(self.flags)
    }

    pub fn can_be_enabled(&self) -> bool {
        madt_cpu_can_be_enabled(self.flags)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MadtIoApicInfo {
    pub id: u8,
    pub physical_address: u32,
    pub global_system_interrupt_base: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MadtInterruptSourceOverrideInfo {
    pub bus_source: u8,
    pub irq_source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

#[derive(Debug)]
pub struct MadtParseResult {
    pub local_apic_physical_address: u64,
    pub cpu_apics: Vec<MadtCpuApic>,
    pub io_apics: Vec<MadtIoApicInfo>,
    pub interrupt_source_overrides: Vec<MadtInterruptSourceOverrideInfo>,
}

impl MadtParseResult {
    pub fn cpu_apic_count(&self) -> usize {
        self.cpu_apics.len()
    }

    pub fn enabled_cpu_apic_count(&self) -> usize {
        self.cpu_apics.iter().filter(|apic| apic.enabled()).count()
    }

    pub fn io_apic_count(&self) -> usize {
        self.io_apics.len()
    }

    pub fn print(&self) {
        kprintln!(Debug, "MADT parse result:");
        kprintln!(Debug,
            "  Local APIC physical address: {:#018x}",
            self.local_apic_physical_address
        );

        kprintln!(Debug, "  CPU Local APICs:");
        kprintln!(Debug, "    total:   {}", self.cpu_apic_count());
        kprintln!(Debug, "    enabled: {}", self.enabled_cpu_apic_count());

        for (index, apic) in self.cpu_apics.iter().enumerate() {
            kprintln!(Debug, "    CPU APIC #{}:", index);
            kprintln!(Debug, "      {:?}", apic);
        }

        kprintln!(Debug, "  I/O APICs:");
        kprintln!(Debug, "    total: {}", self.io_apic_count());

        for (index, io_apic) in self.io_apics.iter().enumerate() {
            kprintln!(Debug, "    I/O APIC #{}:", index);
            kprintln!(Debug, "      {:?}", io_apic);
        }
    }
}
/*
Entry Type 0: Processor Local APIC

This type represents a single logical processor and its local interrupt controller.
Offset (hex) 	Length 	Description
2 	1 	ACPI Processor ID
3 	1 	APIC ID
4 	4 	Flags (bit 0 = Processor Enabled) (bit 1 = Online Capable)

If flags bit 0 is set the CPU is able to be enabled, if it is not set you need to check bit 1. If that one is set you can still enable it, if it is not the CPU can not be enabled and the OS should not try.
Entry Type 1: I/O APIC

This type represents a I/O APIC. The global system interrupt base is the first interrupt number that this I/O APIC handles. You can see how many interrupts it handles using the register by getting the number of redirection entries from register 0x01, as described in IO APIC Registers.
Offset (hex) 	Length 	Description
2 	1 	I/O APIC's ID
3 	1 	Reserved (0)
4 	4 	I/O APIC Address
8 	4 	Global System Interrupt Base
Entry Type 2: I/O APIC Interrupt Source Override

This entry type contains the data for an Interrupt Source Override. This explains how IRQ sources are mapped to global system interrupts. For example, IRQ source for the timer is 0, and the global system interrupt will usually be 2. So you could look for the I/O APIC with the base below 2 and within its redirection entries, then make the redirection entry for (2 - base) to be the timer interrupt.
Offset (hex) 	Length 	Description
2 	1 	Bus Source
3 	1 	IRQ Source
4 	4 	Global System Interrupt
8 	2 	Flags (see below)
Entry type 3: I/O APIC Non-maskable interrupt source

Specifies which I/O APIC interrupt inputs should be enabled as non-maskable.
Offset (hex) 	Length 	Description
2 	1 	NMI Source
3 	1 	Reserved
4 	2 	Flags (see below)
6 	4 	Global System Interrupt
Entry Type 4: Local APIC Non-maskable interrupts

Configure these with the LINT0 and LINT1 entries in the Local vector table of the relevant processor(')s(') local APIC.
Offset (hex) 	Length 	Description
2 	1 	ACPI Processor ID (0xFF means all processors)
3 	2 	Flags (see below)
5 	1 	LINT# (0 or 1)
Entry Type 5: Local APIC Address Override

Provides 64 bit systems with an override of the physical address of the Local APIC. There can only be one of these defined in the MADT. If this structure is defined, the 64-bit Local APIC address stored within it should be used instead of the 32-bit Local APIC address stored in the MADT header.
Offset (hex) 	Length 	Description
2 	2 	Reserved
4 	8 	64-bit physical address of Local APIC
Entry Type 9: Processor Local x2APIC

Represents a physical processor and its Local x2APIC. Identical to Local APIC; used only when that struct would not be able to hold the required values.
Offset (hex) 	Length 	Description
2 	2 	Reserved
4 	4 	Processor's local x2APIC ID
8 	4 	Flags (same as the Local APIC flags)
C 	4 	ACPI ID
 */
impl Madt {
    pub fn new_from_virt_addr<'a>(ptr: VirtAddr) -> &'a Madt {
        unsafe { &*(ptr.as_ptr::<Madt>()) }
    }

    pub fn entries_start(&self) -> *const u8 {
        let base = self as *const Self as *const u8;

        unsafe { base.add(core::mem::size_of::<Madt>()) }
    }

    pub fn entries_end(&self) -> *const u8 {
        let base = self as *const Self as *const u8;

        unsafe { base.add(self.header.length as usize) }
    }

    fn validate_entry_size(header: &MadtEntryHeader, expected_size: usize) {
        if header.length < expected_size as u8 {
            panic!("Invalid MADT entry length");
        }
    }

    pub unsafe fn parse(&self) -> MadtParseResult {
        let mut ptr = self.entries_start();
        let end = self.entries_end();
        let mut result = MadtParseResult {
            local_apic_physical_address: self.local_apic_address as u64,
            cpu_apics: Vec::new(),
            io_apics: Vec::new(),
            interrupt_source_overrides: Vec::new(),
        };

        while ptr < end {
            let header = &*(ptr as *const MadtEntryHeader);

            if header.length < core::mem::size_of::<MadtEntryHeader>() as u8 {
                panic!("Invalid MADT entry length");
            }

            let next = ptr.add(header.length as usize);
            if next > end {
                panic!("MADT entry extends past table end");
            }

            match header.entry_type {
                MADT_ENTRY_TYPE_LOCAL_APIC => {
                    Self::validate_entry_size(header, MADT_LOCAL_APIC_ENTRY_SIZE);
                    let entry = &*(ptr as *const MadtLocalApic);

                    result.cpu_apics.push(MadtCpuApic {
                        acpi_processor_id: entry.acpi_processor_id as u32,
                        apic_id: entry.apic_id as u32,
                        flags: entry.flags,
                        is_x2apic: false,
                    });
                }

                MADT_ENTRY_TYPE_IO_APIC => {
                    Self::validate_entry_size(header, MADT_IO_APIC_ENTRY_SIZE);
                    let entry = &*(ptr as *const MadtIoApic);

                    result.io_apics.push(MadtIoApicInfo {
                        id: entry.io_apic_id,
                        physical_address: entry.io_apic_address,
                        global_system_interrupt_base: entry.global_system_interrupt_base,
                    });
                }

                MADT_ENTRY_TYPE_INTERRUPT_SOURCE_OVERRIDE => {
                    Self::validate_entry_size(header, MADT_INTERRUPT_SOURCE_OVERRIDE_ENTRY_SIZE);
                    let entry = &*(ptr as *const MadtInterruptSourceOverride);

                    result
                        .interrupt_source_overrides
                        .push(MadtInterruptSourceOverrideInfo {
                            bus_source: entry.bus_source,
                            irq_source: entry.irq_source,
                            global_system_interrupt: entry.global_system_interrupt,
                            flags: entry.flags,
                        });
                }

                MADT_ENTRY_TYPE_LOCAL_APIC_ADDRESS_OVERRIDE => {
                    Self::validate_entry_size(header, MADT_LOCAL_APIC_ADDRESS_OVERRIDE_ENTRY_SIZE);
                    let entry = &*(ptr as *const MadtLocalApicAddressOverride);

                    result.local_apic_physical_address = entry.local_apic_address;
                }

                MADT_ENTRY_TYPE_LOCAL_X2APIC => {
                    Self::validate_entry_size(header, MADT_LOCAL_X2APIC_ENTRY_SIZE);
                    let entry = &*(ptr as *const MadtLocalX2Apic);

                    result.cpu_apics.push(MadtCpuApic {
                        acpi_processor_id: entry.acpi_id,
                        apic_id: entry.x2apic_id,
                        flags: entry.flags,
                        is_x2apic: true,
                    });
                }

                _ => {
                    // unknown type - skip
                }
            }

            ptr = next;
        }

        result
    }

    pub unsafe fn parse_madt(madt: &'static Madt) -> MadtParseResult {
        madt.parse()
    }
}

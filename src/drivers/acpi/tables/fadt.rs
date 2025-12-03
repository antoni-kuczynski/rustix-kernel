/*
 * Created by Antoni Kuczyński
 * 03/11/2025
 */
use core::ptr::slice_from_raw_parts;
use crate::asm::{inw};
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::{vgaprintln};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;

// ============================================================
//               **FADT STRUCT**
// ============================================================
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct FADT {
    pub h: ACPISDTHeader,
    pub firmware_ctrl: u32,
    pub dsdt: u32,

    //fields used in ACPI 1.0, for compatibility only
    pub reserved: u8,

    pub preferred_power_management_profile: u8,
    pub sci_interrupt: u16,
    pub smi_command_port: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4bios_req: u8,
    pub pstate_control: u8,
    pub pm1a_event_block: u32,
    pub pm1b_event_block: u32,
    pub pm1a_control_block: u32,
    pub pm1b_control_block: u32,
    pub pm2_control_block: u32,
    pub pm_timer_block: u32,
    pub gpe0_block: u32,
    pub gpe1_block: u32,
    pub pm1_event_length: u8,
    pub pm1_control_length: u8,
    pub pm2_control_length: u8,
    pub pm_timer_length: u8,
    pub gpe0_length: u8,
    pub gpe1_length: u8,
    pub gpe1_base: u8,
    pub cstate_control: u8,
    pub worst_c2_latency: u16,
    pub worst_c3_latency: u16,
    pub flush_size: u16,
    pub flush_stride: u16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alarm: u8,
    pub month_alarm: u8,
    pub century: u8,

    // reserved in ACPI 1.0; used since ACPI 2.0+
    pub boot_architecture_flags: u16,

    pub reserved2: u8,
    pub flags: u32,

    //12byte structure
    pub reset_reg: GenericAddressStructure,

    pub reset_value: u8,
    pub reserved3: [u8; 3],

    // 64bit pointers - Available on ACPI 2.0+
    pub x_firmware_control: u64,
    pub x_dsdt: u64,

    pub x_pm1a_event_block: GenericAddressStructure,
    pub x_pm1b_event_block: GenericAddressStructure,
    pub x_pm1a_control_block: GenericAddressStructure,
    pub x_pm1b_control_block: GenericAddressStructure,
    pub x_pm2_control_block: GenericAddressStructure,
    pub x_pm_timer_block: GenericAddressStructure,
    pub x_gpe0_block: GenericAddressStructure,
    pub x_gpe1_block: GenericAddressStructure,
}

pub enum PrefferedPowerManagementProfile {
    Unspecified,
    Desktop,
    Mobile,
    Workstation,
    EnterpriseServer,
    SOHOServer,
    AppliancePC,
    PerformanceServer,
    Reserved
}

impl AcpiSdtTable for FADT {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::FADT
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.h
    }
}

impl FADT {
    pub fn new_from_ptr(ptr: u64) -> &'static FADT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let rsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                (length - size_of_val(&header)) >> 2,
            );

            &*(rsdt_ptr as *const FADT)
        }
    }

    pub fn get_dsdt_pointer(&self, mem_logical_offset: u64) -> u64 {
        self.dsdt as u64 + mem_logical_offset
    }

    pub fn get_preffered_power_management_profile(&self) -> PrefferedPowerManagementProfile {
        match self.preferred_power_management_profile {
            0 => PrefferedPowerManagementProfile::Unspecified,
            1 => PrefferedPowerManagementProfile::Desktop,
            2 => PrefferedPowerManagementProfile::Mobile,
            3 => PrefferedPowerManagementProfile::Workstation,
            4 => PrefferedPowerManagementProfile::EnterpriseServer,
            5 => PrefferedPowerManagementProfile::SOHOServer,
            6 => PrefferedPowerManagementProfile::AppliancePC,
            _ => PrefferedPowerManagementProfile::Reserved
        }
    }
}
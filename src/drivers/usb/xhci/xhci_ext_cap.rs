#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 03/06/2026
 */
use crate::drivers::pci::mmio_read;
use crate::vgaprintln;
use alloc::vec::Vec;
use core::fmt;
use x86_64::VirtAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhciPortProtocol {
    Unknown,
    Usb2,
    Usb3,
    Usb4,
}

impl fmt::Display for XhciPortProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XhciPortProtocol::Unknown => write!(f, "unknown"),
            XhciPortProtocol::Usb2 => write!(f, "USB 2"),
            XhciPortProtocol::Usb3 => write!(f, "USB 3"),
            XhciPortProtocol::Usb4 => write!(f, "USB 4"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhciPortSpeed {
    Unknown,
    LowSpeed,
    FullSpeed,
    HighSpeed,
    SuperSpeed,
    SuperSpeedPlus,
    Usb4,
    VendorDefined,
}

#[derive(Debug, Clone, Copy)]
pub struct XhciPortInfo {
    pub port_id: u8,
    pub protocol: XhciPortProtocol,
    pub major: u8,
    pub minor: u8,
    pub slot_type: u8,
    pub protocol_defined: u16,
    pub speed: XhciPortSpeed,
    pub psiv: u8,
    pub raw_bps: Option<u64>,
    pub psi: Option<XhciProtocolSpeedId>,
}

impl XhciPortInfo {
    pub const fn unknown() -> Self {
        Self {
            port_id: 0,
            protocol: XhciPortProtocol::Unknown,
            major: 0,
            minor: 0,
            slot_type: 0,
            protocol_defined: 0,
            speed: XhciPortSpeed::Unknown,
            psiv: 0,
            raw_bps: None,
            psi: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct XhciSupportedProtocol {
    pub name: u32,
    pub major: u8,
    pub minor: u8,
    pub port_offset: u8,
    pub port_count: u8,
    pub psic: u8,
    pub protocol_defined: u16,
    pub slot_type: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct XhciProtocolSpeedId {
    pub psiv: u8,
    pub psie: u8,
    pub plt: u8,
    pub pfd: bool,
    pub lp: u8,
    pub psim: u16,
}

// ============================================================
// Generic Extended Capability header, offset 00h
// ============================================================

const XHCI_EXT_CAP_ID_MASK: u32 = 0xFF;

const XHCI_EXT_CAP_NEXT_SHIFT: u32 = 8;
const XHCI_EXT_CAP_NEXT_MASK: u32 = 0xFF << XHCI_EXT_CAP_NEXT_SHIFT;

const XHCI_EXT_CAP_MINOR_SHIFT: u32 = 16;
const XHCI_EXT_CAP_MINOR_MASK: u32 = 0xFF << XHCI_EXT_CAP_MINOR_SHIFT;

const XHCI_EXT_CAP_MAJOR_SHIFT: u32 = 24;
const XHCI_EXT_CAP_MAJOR_MASK: u32 = 0xFF << XHCI_EXT_CAP_MAJOR_SHIFT;

// ============================================================
const XHCI_EXT_CAP_ID_SUPPORTED_PROTOCOL: u8 = 0x02;

// ============================================================
// Supported Protocol Capability offsets
// ============================================================

const XHCI_SUPPORTED_PROTOCOL_NAME_OFFSET: usize = 0x04;
const XHCI_SUPPORTED_PROTOCOL_PORTS_OFFSET: usize = 0x08;
const XHCI_SUPPORTED_PROTOCOL_SLOT_OFFSET: usize = 0x0C;
const XHCI_SUPPORTED_PROTOCOL_PSI_OFFSET: usize = 0x10;

// Offset 08h - Supported Protocol Ports
const XHCI_PROTO_PORT_OFFSET_SHIFT: u32 = 0;
const XHCI_PROTO_PORT_OFFSET_MASK: u32 = 0xFF << XHCI_PROTO_PORT_OFFSET_SHIFT;

const XHCI_PROTO_PORT_COUNT_SHIFT: u32 = 8;
const XHCI_PROTO_PORT_COUNT_MASK: u32 = 0xFF << XHCI_PROTO_PORT_COUNT_SHIFT;

const XHCI_PROTO_DEFINED_SHIFT: u32 = 16;
const XHCI_PROTO_DEFINED_MASK: u32 = 0x0FFF << XHCI_PROTO_DEFINED_SHIFT;

const XHCI_PROTO_PSIC_SHIFT: u32 = 28;
const XHCI_PROTO_PSIC_MASK: u32 = 0xF << XHCI_PROTO_PSIC_SHIFT;

// Offset 0Ch - Protocol Slot Type
const XHCI_PROTO_SLOT_TYPE_MASK: u32 = 0x1F;

// Name String "USB " little-endian.
const XHCI_PROTO_NAME_USB: u32 = 0x2042_5355;

// ============================================================
// PSI Dword fields
// ============================================================

const XHCI_PSI_PSIV_SHIFT: u32 = 0;
const XHCI_PSI_PSIV_MASK: u32 = 0xF << XHCI_PSI_PSIV_SHIFT;

const XHCI_PSI_PSIE_SHIFT: u32 = 4;
const XHCI_PSI_PSIE_MASK: u32 = 0x3 << XHCI_PSI_PSIE_SHIFT;

const XHCI_PSI_PLT_SHIFT: u32 = 6;
const XHCI_PSI_PLT_MASK: u32 = 0x3 << XHCI_PSI_PLT_SHIFT;

const XHCI_PSI_PFD: u32 = 1 << 8;

const XHCI_PSI_LP_SHIFT: u32 = 14;
const XHCI_PSI_LP_MASK: u32 = 0x3 << XHCI_PSI_LP_SHIFT;

const XHCI_PSI_PSIM_SHIFT: u32 = 16;
const XHCI_PSI_PSIM_MASK: u32 = 0xFFFF << XHCI_PSI_PSIM_SHIFT;

// ============================================================
// PSI helpers
// ============================================================

impl XhciProtocolSpeedId {
    pub fn from_raw(raw: u32) -> Self {
        Self {
            psiv: ((raw & XHCI_PSI_PSIV_MASK) >> XHCI_PSI_PSIV_SHIFT) as u8,
            psie: ((raw & XHCI_PSI_PSIE_MASK) >> XHCI_PSI_PSIE_SHIFT) as u8,
            plt: ((raw & XHCI_PSI_PLT_MASK) >> XHCI_PSI_PLT_SHIFT) as u8,
            pfd: raw & XHCI_PSI_PFD != 0,
            lp: ((raw & XHCI_PSI_LP_MASK) >> XHCI_PSI_LP_SHIFT) as u8,
            psim: ((raw & XHCI_PSI_PSIM_MASK) >> XHCI_PSI_PSIM_SHIFT) as u16,
        }
    }

    pub fn bits_per_second(&self) -> Option<u64> {
        let multiplier = match self.psie {
            0 => 1,
            1 => 1_000,
            2 => 1_000_000,
            3 => 1_000_000_000,
            _ => return None,
        };

        Some(self.psim as u64 * multiplier)
    }
}

fn classify_speed_id(
    protocol: XhciPortProtocol,
    psiv: u8,
    bits_per_second: Option<u64>,
) -> XhciPortSpeed {
    match (protocol, psiv) {
        (XhciPortProtocol::Usb2, 1) => XhciPortSpeed::FullSpeed,
        (XhciPortProtocol::Usb2, 2) => XhciPortSpeed::LowSpeed,
        (XhciPortProtocol::Usb2, 3) => XhciPortSpeed::HighSpeed,
        (XhciPortProtocol::Usb3, 4) => XhciPortSpeed::SuperSpeed,
        (XhciPortProtocol::Usb4, _) => XhciPortSpeed::Usb4,
        _ => match bits_per_second {
            Some(bps) if bps >= 10_000_000_000 => XhciPortSpeed::SuperSpeedPlus,
            Some(bps) if bps >= 5_000_000_000 => XhciPortSpeed::SuperSpeed,
            Some(480_000_000) => XhciPortSpeed::HighSpeed,
            Some(12_000_000) => XhciPortSpeed::FullSpeed,
            Some(1_500_000) => XhciPortSpeed::LowSpeed,
            Some(_) => XhciPortSpeed::VendorDefined,
            None => XhciPortSpeed::Unknown,
        },
    }
}

fn default_speed_for_protocol(protocol: XhciPortProtocol) -> (XhciPortSpeed, u8, Option<u64>) {
    match protocol {
        XhciPortProtocol::Usb2 => (XhciPortSpeed::HighSpeed, 3, Some(480_000_000)),
        XhciPortProtocol::Usb3 => (XhciPortSpeed::SuperSpeed, 4, Some(5_000_000_000)),
        XhciPortProtocol::Usb4 => (XhciPortSpeed::Usb4, 0, None),
        XhciPortProtocol::Unknown => (XhciPortSpeed::Unknown, 0, None),
    }
}

fn speed_rank(speed: XhciPortSpeed) -> u8 {
    match speed {
        XhciPortSpeed::Unknown => 0,
        XhciPortSpeed::LowSpeed => 1,
        XhciPortSpeed::FullSpeed => 2,
        XhciPortSpeed::HighSpeed => 3,
        XhciPortSpeed::SuperSpeed => 4,
        XhciPortSpeed::SuperSpeedPlus => 5,
        XhciPortSpeed::Usb4 => 6,
        XhciPortSpeed::VendorDefined => 7,
    }
}

unsafe fn read_supported_protocol_speed(
    cap_addr: VirtAddr,
    cap: XhciSupportedProtocol,
    protocol: XhciPortProtocol,
) -> (XhciPortSpeed, u8, Option<u64>, Option<XhciProtocolSpeedId>) {
    if cap.psic == 0 {
        let (speed, psiv, raw_bps) = default_speed_for_protocol(protocol);
        return (speed, psiv, raw_bps, None);
    }

    let mut best = (XhciPortSpeed::Unknown, 0, None, None);
    for psi_index in 0..cap.psic as usize {
        let psi = read_supported_protocol_psi(cap_addr, psi_index);
        let raw_bps = psi.bits_per_second();
        let speed = classify_speed_id(protocol, psi.psiv, raw_bps);

        if raw_bps > best.2 || (raw_bps == best.2 && speed_rank(speed) > speed_rank(best.0)) {
            best = (speed, psi.psiv, raw_bps, Some(psi));
        }
    }

    best
}

// ============================================================
// Reading supported protocol capability
// ============================================================

pub unsafe fn read_supported_protocol_cap(cap_addr: VirtAddr) -> XhciSupportedProtocol {
    let header = unsafe { mmio_read::<u32>(cap_addr, 0) };

    let name = unsafe { mmio_read::<u32>(cap_addr, XHCI_SUPPORTED_PROTOCOL_NAME_OFFSET as u64) };

    let ports = unsafe { mmio_read::<u32>(cap_addr, XHCI_SUPPORTED_PROTOCOL_PORTS_OFFSET as u64) };

    let slot = unsafe { mmio_read::<u32>(cap_addr, XHCI_SUPPORTED_PROTOCOL_SLOT_OFFSET as u64) };

    XhciSupportedProtocol {
        name,
        major: ((header & XHCI_EXT_CAP_MAJOR_MASK) >> XHCI_EXT_CAP_MAJOR_SHIFT) as u8,
        minor: ((header & XHCI_EXT_CAP_MINOR_MASK) >> XHCI_EXT_CAP_MINOR_SHIFT) as u8,
        port_offset: ((ports & XHCI_PROTO_PORT_OFFSET_MASK) >> XHCI_PROTO_PORT_OFFSET_SHIFT) as u8,
        port_count: ((ports & XHCI_PROTO_PORT_COUNT_MASK) >> XHCI_PROTO_PORT_COUNT_SHIFT) as u8,
        psic: ((ports & XHCI_PROTO_PSIC_MASK) >> XHCI_PROTO_PSIC_SHIFT) as u8,
        protocol_defined: ((ports & XHCI_PROTO_DEFINED_MASK) >> XHCI_PROTO_DEFINED_SHIFT) as u16,
        slot_type: (slot & XHCI_PROTO_SLOT_TYPE_MASK) as u8,
    }
}

pub unsafe fn read_supported_protocol_psi(
    cap_addr: VirtAddr,
    psi_index: usize,
) -> XhciProtocolSpeedId {
    let raw = unsafe {
        mmio_read::<u32>(
            cap_addr,
            (XHCI_SUPPORTED_PROTOCOL_PSI_OFFSET + psi_index * 4) as u64,
        )
    };

    XhciProtocolSpeedId::from_raw(raw)
}

pub fn classify_supported_protocol(cap: XhciSupportedProtocol) -> XhciPortProtocol {
    if cap.name != XHCI_PROTO_NAME_USB {
        return XhciPortProtocol::Unknown;
    }

    match cap.major {
        2 => XhciPortProtocol::Usb2,
        3 => XhciPortProtocol::Usb3,
        4 => XhciPortProtocol::Usb4,
        _ => XhciPortProtocol::Unknown,
    }
}

// ============================================================
//                  PROTOCOLS PARSER
// ============================================================
pub unsafe fn parse_xhci_supported_protocols(
    first_ext_cap_addr: Option<VirtAddr>,
    max_ports: usize,
) -> Vec<XhciPortInfo> {
    let mut port_info = Vec::with_capacity(max_ports);
    port_info.resize_with(max_ports, XhciPortInfo::unknown);

    for (index, info) in port_info.iter_mut().enumerate() {
        info.port_id = (index + 1) as u8;
    }

    let Some(mut cap_addr) = first_ext_cap_addr else {
        return port_info;
    };

    let mut guard = 0usize;
    loop {
        if guard > 256 {
            vgaprintln!("Exceeded chain length of 256 - the ext cap chain is corrupted!");
            break;
        }

        guard += 1;

        let header = mmio_read::<u32>(cap_addr, 0);
        let cap_id = (header & XHCI_EXT_CAP_ID_MASK) as u8;
        let next = ((header & XHCI_EXT_CAP_NEXT_MASK) >> XHCI_EXT_CAP_NEXT_SHIFT) as u8;

        if cap_id == XHCI_EXT_CAP_ID_SUPPORTED_PROTOCOL {
            let supported_protocol_cap = read_supported_protocol_cap(cap_addr);
            let protocol = classify_supported_protocol(supported_protocol_cap);
            let (speed, psiv, raw_bps, psi) =
                read_supported_protocol_speed(cap_addr, supported_protocol_cap, protocol);

            let start = supported_protocol_cap.port_offset;
            let end = supported_protocol_cap
                .port_offset
                .saturating_add(supported_protocol_cap.port_count);

            for port_id in start..end {
                if port_id == 0 {
                    continue;
                }

                let index = (port_id - 1) as usize;

                if index >= port_info.len() {
                    continue;
                }

                port_info[index] = XhciPortInfo {
                    port_id,
                    protocol,
                    major: supported_protocol_cap.major,
                    minor: supported_protocol_cap.minor,
                    slot_type: supported_protocol_cap.slot_type,
                    protocol_defined: supported_protocol_cap.protocol_defined,
                    speed,
                    psiv,
                    raw_bps,
                    psi,
                };
            }
        }

        if next == 0 {
            break;
        }

        cap_addr += (next as u64) * 4;
    }
    port_info
}

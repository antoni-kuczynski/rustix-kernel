#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 02/06/2026
 */
// Bit layout for USB2/USB3 PORTSC:
//
//
// 31      30      29:28  27  26  25  24  23  22  21  20  19  18  17  16
// WPR     DR      RsvdZ  WOE WDE WCE CAS CEC PLC PRC OCC WRC PEC CSC LWS
//
// 15:14   13:10       9   8:5  4   3   2   1    0
// PIC     Port Speed  PP  PLS  PR  OCA TM  PED  CCS

use core::ops::Add;
use core::ptr;
use x86_64::VirtAddr;

const CCS: u32 = 1 << 0;
const PED: u32 = 1 << 1;
const TM: u32 = 1 << 2;
const OCA: u32 = 1 << 3;
const PR: u32 = 1 << 4;
const PLS_MASK: u32 = 0xF << 5;
const PLS_SHIFT: u32 = 5;
const PP: u32 = 1 << 9;
const PORT_SPEED_MASK: u32 = 0xF << 10;
const PORT_SPEED_SHIFT: u32 = 10;
const PIC_MASK: u32 = 0x3 << 14;
const PIC_SHIFT: u32 = 14;
const LWS: u32 = 1 << 16;
const CSC: u32 = 1 << 17;
const PEC: u32 = 1 << 18;
const WRC: u32 = 1 << 19;
const OCC: u32 = 1 << 20;
const PRC: u32 = 1 << 21;
const PLC: u32 = 1 << 22;
const CEC: u32 = 1 << 23;
const CAS: u32 = 1 << 24;
const WCE: u32 = 1 << 25;
const WDE: u32 = 1 << 26;
const WOE: u32 = 1 << 27;
const DR: u32 = 1 << 30;
const WPR: u32 = 1 << 31;

const CHANGE_MASK: u32 = CSC | PEC | WRC | OCC | PRC | PLC | CEC;
const STABLE_WRITE_MASK: u32 = PLS_MASK | PP | PIC_MASK | WCE | WDE | WOE;

/// Decoded Port Link State values from the PORTSC PLS field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PortLinkState {
    /// U0, the active link state.
    U0 = 0,
    /// U1, a low-power link state reported by USB3 protocol ports.
    U1 = 1,
    /// U2, a low-power link state. On USB2 protocol ports, writing this value requests LPM.
    U2 = 2,
    /// U3, device suspended. Writes to U3 selectively suspend the device connected to the port.
    U3 = 3,
    /// Disabled state.
    Disabled = 4,
    /// RxDetect state. USB3 software may write this while disabled to move the port to disconnected.
    RxDetect = 5,
    /// Inactive state, used by USB3 protocol ports.
    Inactive = 6,
    /// Polling state.
    Polling = 7,
    /// Recovery state.
    Recovery = 8,
    /// Hot Reset state.
    HotReset = 9,
    /// Compliance mode state.
    ComplianceMode = 10,
    /// Test mode state.
    TestMode = 11,
    /// Reserved PLS encoding.
    Reserved12 = 12,
    /// Reserved PLS encoding.
    Reserved13 = 13,
    /// Reserved PLS encoding.
    Reserved14 = 14,
    /// Resume state.
    Resume = 15,
}

impl PortLinkState {
    /// Converts the raw 4-bit PLS field to a typed link state.
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0xF {
            0 => Self::U0,
            1 => Self::U1,
            2 => Self::U2,
            3 => Self::U3,
            4 => Self::Disabled,
            5 => Self::RxDetect,
            6 => Self::Inactive,
            7 => Self::Polling,
            8 => Self::Recovery,
            9 => Self::HotReset,
            10 => Self::ComplianceMode,
            11 => Self::TestMode,
            12 => Self::Reserved12,
            13 => Self::Reserved13,
            14 => Self::Reserved14,
            _ => Self::Resume,
        }
    }

    /// Returns the raw 4-bit PLS encoding.
    pub const fn bits(self) -> u8 {
        self as u8
    }
}

/// Decoded Port Indicator Control values from the PORTSC PIC field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PortIndicator {
    /// Port indicators are off.
    Off = 0,
    /// Amber port indicator.
    Amber = 1,
    /// Green port indicator.
    Green = 2,
    /// Undefined indicator encoding.
    Undefined = 3,
}

impl PortIndicator {
    /// Converts the raw 2-bit PIC field to a typed indicator value.
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => Self::Off,
            1 => Self::Amber,
            2 => Self::Green,
            _ => Self::Undefined,
        }
    }

    /// Returns the raw 2-bit PIC encoding.
    pub const fn bits(self) -> u8 {
        self as u8
    }
}

/// Change bits in PORTSC. These fields are RW1CS: write `1` to clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortChange(u32);

impl PortChange {
    /// CSC, Connect Status Change.
    pub const CSC: Self = Self(CSC);
    /// PEC, Port Enabled/Disabled Change.
    pub const PEC: Self = Self(PEC);
    /// WRC, Warm Port Reset Change.
    pub const WRC: Self = Self(WRC);
    /// OCC, Over-current Change.
    pub const OCC: Self = Self(OCC);
    /// PRC, Port Reset Change.
    pub const PRC: Self = Self(PRC);
    /// PLC, Port Link State Change.
    pub const PLC: Self = Self(PLC);
    /// CEC, Config Error Change.
    pub const CEC: Self = Self(CEC);
    /// All defined PORTSC change bits.
    pub const ALL: Self = Self(CHANGE_MASK);

    /// Returns the raw RW1CS bit mask.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Combines two change masks.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Value helper for the xHCI Port Status and Control register (PORTSC).
///
/// PORTSC mixes read-only status, normal writable fields, write-one-to-set commands, and
/// write-one-to-clear change flags. Use `write_from_raw` before modifying a value that will be
/// written back to MMIO, then use the `write_*` methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PortStatusControl(u32);

impl PortStatusControl {
    pub fn from_port(operational_base: VirtAddr, port: u8) -> Self {
        unsafe {
            let port_index = port.saturating_sub(1);
            let addr = operational_base.add(0x400).add((port_index * 0x10) as u64);
            Self(ptr::read_volatile(addr.as_ptr::<u32>()))
        }
    }

    pub fn write_to_port(self, operational_base: VirtAddr, port: u8) {
        unsafe {
            let port_index = port.saturating_sub(1);
            let addr = operational_base.add(0x400).add((port_index * 0x10) as u64);
            ptr::write_volatile(addr.as_mut_ptr::<u32>(), self.0);
        }
    }

    /// Creates a PORTSC helper from a raw 32-bit MMIO value.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw value currently stored in this helper.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Creates a write-safe PORTSC value from a raw MMIO read.
    ///
    /// This clears status-only bits, RW1CS change bits, and RW1S command bits so a read-modify-write
    /// does not accidentally disable/reset the port or clear pending change notifications.
    pub const fn write_from_raw(raw: u32) -> Self {
        Self(raw & STABLE_WRITE_MASK)
    }

    /// Reads CCS, Current Connect Status.
    ///
    /// This bit reflects whether a device is currently connected to the port. It reads as `false`
    /// when Port Power is off.
    pub const fn ccs_read(self) -> bool {
        self.0 & CCS != 0
    }

    /// Reads PED, Port Enabled/Disabled.
    ///
    /// Ports may only be enabled by the xHC. Software can request disable by writing `1` to PED.
    pub const fn ped_read(self) -> bool {
        self.0 & PED != 0
    }

    /// Writes PED, Port Enabled/Disabled.
    ///
    /// For software this is a disable request. PED and PR must not be written together.
    pub fn ped_write(&mut self) {
        self.0 |= PED;
        self.0 &= !PR;
    }

    /// Reads TM, Tunneled Mode.
    ///
    /// This field is valid only when a device is connected. USB2 protocol ports reserve this bit.
    pub const fn tm_read(self) -> bool {
        self.0 & TM != 0
    }

    /// Reads OCA, Over-current Active.
    ///
    /// The xHC clears this status when the over-current condition is removed.
    pub const fn oca_read(self) -> bool {
        self.0 & OCA != 0
    }

    /// Reads PR, Port Reset.
    ///
    /// The root hub clears this bit after reset signaling completes.
    pub const fn pr_read(self) -> bool {
        self.0 & PR != 0
    }

    /// Writes PR, Port Reset.
    ///
    /// For USB2 protocol ports this executes USB2 bus reset; for USB3 protocol ports this executes
    /// hot reset. Software must not set PED and PR together.
    pub fn pr_write(&mut self) {
        self.0 |= PR;
        self.0 &= !PED;
    }

    /// Reads PLS, Port Link State.
    ///
    /// The field is undefined when Port Power is off. State transitions are reflected only after
    /// the transition completes.
    pub const fn pls_read(self) -> PortLinkState {
        PortLinkState::from_bits(((self.0 & PLS_MASK) >> PLS_SHIFT) as u8)
    }

    /// Writes PLS, Port Link State.
    ///
    /// PORTSC requires LWS for writes to PLS, so this method sets LWS automatically.
    pub fn pls_write(&mut self, state: PortLinkState) {
        self.0 = (self.0 & !PLS_MASK) | ((state.bits() as u32) << PLS_SHIFT) | LWS;
    }

    /// Reads PP, Port Power.
    ///
    /// When this bit is clear, the port is nonfunctional and does not report attaches, detaches, or
    /// link-state changes. Controllers without port power switches may ignore writes to this bit.
    pub const fn pp_read(self) -> bool {
        self.0 & PP != 0
    }

    /// Writes PP, Port Power.
    ///
    /// After modifying port power, software should read PORTSC and confirm the target state before
    /// modifying it again.
    pub fn pp_write(&mut self, powered: bool) {
        self.0 &= !PP;
        if powered {
            self.0 |= PP;
        }
    }

    /// Reads PS, Port Speed.
    ///
    /// `0` means undefined speed. Values `1..=15` are Protocol Speed IDs and are meaningful only
    /// when a device is connected. On USB2 ports this field is invalid until after reset.
    pub const fn ps_read(self) -> u8 {
        ((self.0 & PORT_SPEED_MASK) >> PORT_SPEED_SHIFT) as u8
    }

    /// Reads PIC, Port Indicator Control.
    ///
    /// Writes to this field have no effect when the controller does not support port indicators.
    /// The field reads as off when Port Power is off.
    pub const fn pic_read(self) -> PortIndicator {
        PortIndicator::from_bits(((self.0 & PIC_MASK) >> PIC_SHIFT) as u8)
    }

    /// Writes PIC, Port Indicator Control.
    pub fn pic_write(&mut self, indicator: PortIndicator) {
        self.0 = (self.0 & !PIC_MASK) | ((indicator.bits() as u32) << PIC_SHIFT);
    }

    /// Reads LWS, Port Link State Write Strobe.
    ///
    /// This bit is normally used as a write strobe for PLS writes.
    pub const fn lws_read(self) -> bool {
        self.0 & LWS != 0
    }

    /// Writes LWS, Port Link State Write Strobe.
    pub fn lws_write(&mut self) {
        self.0 |= LWS;
    }

    /// Reads one selected PORTSC RW1CS change bit.
    pub const fn change_read(self, change: PortChange) -> bool {
        self.0 & change.bits() != 0
    }

    /// Writes one selected PORTSC RW1CS change bit.
    ///
    /// For RW1CS fields, writing `1` clears the pending change.
    pub fn change_write(&mut self, change: PortChange) {
        self.0 |= change.bits() & CHANGE_MASK;
    }

    /// Writes all defined PORTSC RW1CS change bits.
    pub fn change_all_write(&mut self) {
        self.change_write(PortChange::ALL);
    }

    /// Reads CSC, Connect Status Change.
    pub const fn csc_read(self) -> bool {
        self.0 & CSC != 0
    }

    /// Writes CSC, Connect Status Change.
    ///
    /// CSC is RW1CS, so writing `1` clears the pending change.
    pub fn csc_write(&mut self) {
        self.0 |= CSC;
    }

    /// Reads PEC, Port Enabled/Disabled Change.
    pub const fn pec_read(self) -> bool {
        self.0 & PEC != 0
    }

    /// Writes PEC, Port Enabled/Disabled Change.
    ///
    /// PEC is RW1CS, so writing `1` clears the pending change.
    pub fn pec_write(&mut self) {
        self.0 |= PEC;
    }

    /// Reads WRC, Warm Port Reset Change.
    pub const fn wrc_read(self) -> bool {
        self.0 & WRC != 0
    }

    /// Writes WRC, Warm Port Reset Change.
    ///
    /// WRC is RW1CS, so writing `1` clears the pending change.
    pub fn wrc_write(&mut self) {
        self.0 |= WRC;
    }

    /// Reads OCC, Over-current Change.
    pub const fn occ_read(self) -> bool {
        self.0 & OCC != 0
    }

    /// Writes OCC, Over-current Change.
    ///
    /// OCC is RW1CS, so writing `1` clears the pending change.
    pub fn occ_write(&mut self) {
        self.0 |= OCC;
    }

    /// Reads PRC, Port Reset Change.
    pub const fn prc_read(self) -> bool {
        self.0 & PRC != 0
    }

    /// Writes PRC, Port Reset Change.
    ///
    /// PRC is RW1CS, so writing `1` clears the pending change.
    pub fn prc_write(&mut self) {
        self.0 |= PRC;
    }

    /// Reads PLC, Port Link State Change.
    pub const fn plc_read(self) -> bool {
        self.0 & PLC != 0
    }

    /// Writes PLC, Port Link State Change.
    ///
    /// PLC is RW1CS, so writing `1` clears the pending change.
    pub fn plc_write(&mut self) {
        self.0 |= PLC;
    }

    /// Reads CEC, Config Error Change.
    pub const fn cec_read(self) -> bool {
        self.0 & CEC != 0
    }

    /// Writes CEC, Config Error Change.
    ///
    /// CEC is RW1CS, so writing `1` clears the pending change.
    pub fn cec_write(&mut self) {
        self.0 |= CEC;
    }

    /// Reads CAS, Cold Attach Status.
    ///
    /// USB3 ports set this when receiver terminations are detected in Disconnected state but the
    /// port state machine cannot advance to Enabled. It clears when WPR is written or CCS becomes
    /// set. USB2 protocol ports reserve this bit.
    pub const fn cas_read(self) -> bool {
        self.0 & CAS != 0
    }

    /// Reads WCE, Wake on Connect Enable.
    pub const fn wce_read(self) -> bool {
        self.0 & WCE != 0
    }

    /// Writes WCE, Wake on Connect Enable.
    pub fn wce_write(&mut self, enabled: bool) {
        self.write_bit(WCE, enabled);
    }

    /// Reads WDE, Wake on Disconnect Enable.
    pub const fn wde_read(self) -> bool {
        self.0 & WDE != 0
    }

    /// Writes WDE, Wake on Disconnect Enable.
    pub fn wde_write(&mut self, enabled: bool) {
        self.write_bit(WDE, enabled);
    }

    /// Reads WOE, Wake on Over-current Enable.
    pub const fn woe_read(self) -> bool {
        self.0 & WOE != 0
    }

    /// Writes WOE, Wake on Over-current Enable.
    pub fn woe_write(&mut self, enabled: bool) {
        self.write_bit(WOE, enabled);
    }

    /// Reads DR, Device Removable.
    pub const fn dr_read(self) -> bool {
        self.0 & DR != 0
    }

    /// Reads WPR, Warm Port Reset.
    ///
    /// WPR normally reads back as `0`.
    pub const fn wpr_read(self) -> bool {
        self.0 & WPR != 0
    }

    /// Writes WPR, Warm Port Reset.
    ///
    /// The WPR bit reads back as `0`. Once initiated, PR, PRC, and WRC reflect warm reset progress.
    /// USB2 protocol ports reserve this bit.
    pub fn wpr_write(&mut self) {
        self.0 |= WPR;
    }

    fn write_bit(&mut self, bit: u32, enabled: bool) {
        self.0 &= !bit;
        if enabled {
            self.0 |= bit;
        }
    }
}

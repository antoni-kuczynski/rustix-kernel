#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
/*
 * xHCI Slot Context (32B, CSZ = 0)
 *
 * DWORD 0 (offset 0x00):
 *   bits  0..=19  : Route String
 *   bits 20..=23  : Speed
 *   bit      24   : Reserved (RsvdZ)
 *   bit      25   : Multi-TT (MTT)
 *   bit      26   : Hub
 *   bits 27..=31  : Context Entries
 *
 * DWORD 1 (offset 0x04):
 *   bits  0..=15  : Max Exit Latency
 *   bits 16..=23  : Root Hub Port Number
 *   bits 24..=31  : Number of Ports
 *
 * DWORD 2 (offset 0x08):
 *   bits  0..=7   : Parent Hub Slot ID
 *   bits  8..=15  : Parent Port Number
 *   bits 16..=17  : TT Think Time (TTT)
 *   bits 18..=21  : Reserved (RsvdZ)
 *   bits 22..=31  : Interrupter Target
 *
 * DWORD 3 (offset 0x0C):
 *   bits  0..=7   : USB Device Address
 *   bits  8..=26  : Reserved (RsvdZ)
 *   bits 27..=31  : Slot State
 *
 * DWORD 4–7 (offset 0x10–0x1F):
 *   Reserved for xHC (RsvdO)
 *
 * Note:
 * - CSZ = 1 Slot Context = 64B (DWORD 8–15 = RsvdO)
 */

#[repr(C, packed)]
pub struct SlotContext<const CZ: usize> {
    dword0: u32,
    dword1: u32,
    dword2: u32,
    dword3: u32,
    reserved: [u32; CZ],
}

/*
Route String:
This field is used by hubs to route packets to the correct downstream port. The
format of the Route String is defined in section 8.9 the USB3 specification.
As Input, this field shall be set for all USB devices, irrespective of their speed, to indicate their
location in the USB topolog

Speed:
This field is not applicable to USB3 Gen X and Gen T.
This field indicates the speed of the device. Refer to the PORTSC Port Speed field in Table 5-27
for the definition of the valid values.

Multi-TT:
TT - transaction translator
one tt serves multiple usb ports for hubs
(MTT)113. This flag is set to '1' by software if this is a High-speed hub that supports
Multiple TTs and the Multiple TT Interface has been enabled by software, or if this is a Low-/Full-
speed device or Full-speed hub and connected to the xHC through a parent114 High-speed hub
that supports Multiple TTs and the Multiple TT Interface of the parent hub has been enabled by
software, or ‘0’ if not.

Hub:
This flag is set to '1' by software if this device is a USB hub, or '0' if it is a USB function

Context Entries:
This field identifies the index of the last valid Endpoint Context within this
Device Context structure. The value of ‘0’ is Reserved and is not a valid entry for this field. Valid
entries for this field shall be in the range of 1-31. This field indicates the size of the Device
Context structure. For example, ((Context Entries+1) * 32 bytes) = Total bytes for this structure.
Note, Output Context Entries values are written by the xHC, and Input Context Entries values are
written by software.

Max Exit Latency. The Maximum Exit Latency is in microseconds, and indicates the worst case
time it takes to wake up all the links in the path to the device, given the current USB link level
power management settings.

Root Hub Port Number. This field identifies the Root Hub Port Number used to access the USB
device. Refer to section 4.19.7 for port numbering information.

Number of Ports. If this device is a hub (Hub = ‘1’), then this field is set by software to identify
the number of downstream facing ports supported by the hub. Refer to the bNbrPorts field
description in the Hub Descriptor (Table 11-13) of the USB2 spec. If this device is not a hub (Hub
= ‘0’), then this field shall be ‘0’.



=========Offset 08h – Slot Context Field Definitions=================

Parent Hub Slot ID.
If this device is Low-/Full-speed and connected through a High-speed hub,
then this field shall contain the Slot ID of the parent High-speed hub115.
For SS and SSP bus instance, if this device is connected through a higher rank hub116 then this
field shall contain the Slot ID of the parent hub. For example, a Gen1 x1 connected behind a
Gen1 x2 hub, or Gen1 x2 device connected behind Gen2 x2 hub.
This field shall be ‘0’ if any of the following are true:
• Device is attached to a Root Hub port
• Device is a High-Speed device
• Device is the highest rank SS/SSP device supported by xHCI

Parent Port Number.
If this device is Low-/Full-speed and connected through a High-speed hub,
then this field shall contain the number of the downstream facing port of the parent High-speed
hub
For SS and SSP bus instance, if this device is connected through a higher rank hub116 then this
field shall contain the number of the downstream facing port of the parent hub. For example, a
Gen1 x1 connected behind a Gen1 x2 hub, or Gen1 x2 device connected behind Gen2 x2 hub.
This field shall be ‘0’ if any of the following are true:
• Device is attached to a Root Hub port
• Device is a High-Speed device
• Device is the highest rank SS/SSP device supported by xHCI


TT Think Time (TTT).
If this is a High-speed hub (Hub = ‘1’ and Speed = High-Speed), then this
field shall be set by software to identify the time the TT of the hub requires to proceed to the
next full-/low-speed transaction.
Value Think Time
0 TT requires at most 8 FS bit times of inter-transaction gap on a full-/low-speed
downstream bus.
1 TT requires at most 16 FS bit times.
2 TT requires at most 24 FS bit times.
3 TT requires at most 32 FS bit times.
Refer to the TT Think Time sub-field of the wHubCharacteristics field description in the Hub
Descriptor (Table 11-13) and section 11.18.2 of the USB2 spec for more information on TT
Think Time. If this device is not a High-speed hub (Hub = ‘0’ or Speed != High-speed), then this
field shall be ‘0’.

USB Device Address. This field identifies the address assigned to the USB device by the xHC,
and is set upon the successful completion of a Set Address Command. Refer to the USB2 spec
for a more detailed description.
As Output, this field is invalid if the Slot State = Disabled or Default.
As Input, software shall initialize the field to ‘0’.

Slot State.
This field is updated by the xHC when a Device Slot transitions from one state to another.
Value   Slot State
0   Disabled/Enabled
1   Default
2   Addressed
3   Configured
31-4 Reserved
Slot States are defined in section 4.5.3.
As Output, since software initializes all fields of the Device Context data structure to ‘0’, this field
shall initially indicate the Disabled state.
As Input, software shall initialize the field to ‘0’.
Refer to section 4.5.3 for more information on Slot State.
 */

impl<const CZ: usize> SlotContext<CZ> {
    /* ================= DWORD 0 ================= */

    const ROUTE_STRING_MASK: u32 = 0x000F_FFFF; //bits 0..19
    const SPEED_MASK: u32 = 0x00F0_0000; //bits 20..23
    const SPEED_SHIFT: u32 = 20;
    const MULTI_TT_MASK: u32 = 0x0200_0000; //bit 25
    const HUB_MASK: u32 = 0x0400_0000; //bit 26
    const CONTEXT_ENTRIES_MASK: u32 = 0xF800_0000; //bits 27..31
    const CONTEXT_ENTRIES_SHIFT: u32 = 27;

    pub fn get_route_string(&self) -> u32 {
        self.dword0 & Self::ROUTE_STRING_MASK
    }

    pub fn set_route_string(&mut self, route: u32) {
        self.dword0 = (self.dword0 & !Self::ROUTE_STRING_MASK) | (route & Self::ROUTE_STRING_MASK);
    }

    pub fn get_speed(&self) -> u8 {
        ((self.dword0 & Self::SPEED_MASK) >> Self::SPEED_SHIFT) as u8
    }

    pub fn set_speed(&mut self, speed: u8) {
        self.dword0 = (self.dword0 & !Self::SPEED_MASK) | ((speed as u32) << Self::SPEED_SHIFT);
    }

    pub fn is_multi_tt(&self) -> bool {
        (self.dword0 & Self::MULTI_TT_MASK) != 0
    }

    pub fn set_multi_tt(&mut self, enabled: bool) {
        if enabled {
            self.dword0 |= Self::MULTI_TT_MASK;
        } else {
            self.dword0 &= !Self::MULTI_TT_MASK;
        }
    }

    pub fn is_hub(&self) -> bool {
        (self.dword0 & Self::HUB_MASK) != 0
    }

    pub fn set_hub(&mut self, enabled: bool) {
        if enabled {
            self.dword0 |= Self::HUB_MASK;
        } else {
            self.dword0 &= !Self::HUB_MASK;
        }
    }

    pub fn get_context_entries(&self) -> u8 {
        ((self.dword0 & Self::CONTEXT_ENTRIES_MASK) >> Self::CONTEXT_ENTRIES_SHIFT) as u8
    }

    pub fn set_context_entries(&mut self, entries: u8) {
        self.dword0 = (self.dword0 & !Self::CONTEXT_ENTRIES_MASK)
            | ((entries as u32) << Self::CONTEXT_ENTRIES_SHIFT);
    }

    /* ================= DWORD 1 ================= */

    const MAX_EXIT_LATENCY_MASK: u32 = 0x0000_FFFF; //bits 0..15
    const ROOT_HUB_PORT_MASK: u32 = 0x00FF_0000; //bits 16..23
    const ROOT_HUB_PORT_SHIFT: u32 = 16;
    const NUM_PORTS_MASK: u32 = 0xFF00_0000; //bits 24...31
    const NUM_PORTS_SHIFT: u32 = 24;

    pub fn get_max_exit_latency(&self) -> u16 {
        (self.dword1 & Self::MAX_EXIT_LATENCY_MASK) as u16
    }

    pub fn set_max_exit_latency(&mut self, latency: u16) {
        self.dword1 = (self.dword1 & !Self::MAX_EXIT_LATENCY_MASK) | latency as u32;
    }

    pub fn get_root_hub_port(&self) -> u8 {
        ((self.dword1 & Self::ROOT_HUB_PORT_MASK) >> Self::ROOT_HUB_PORT_SHIFT) as u8
    }

    pub fn set_root_hub_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !Self::ROOT_HUB_PORT_MASK)
            | ((port as u32) << Self::ROOT_HUB_PORT_SHIFT);
    }

    pub fn get_num_ports(&self) -> u8 {
        ((self.dword1 & Self::NUM_PORTS_MASK) >> Self::NUM_PORTS_SHIFT) as u8
    }

    pub fn set_num_ports(&mut self, ports: u8) {
        self.dword1 =
            (self.dword1 & !Self::NUM_PORTS_MASK) | ((ports as u32) << Self::NUM_PORTS_SHIFT);
    }

    /* ================= DWORD 2 ================= */

    const PARENT_HUB_SLOT_MASK: u32 = 0x0000_00FF; //bits 0..7
    const PARENT_PORT_MASK: u32 = 0x0000_FF00; //bits 8..15
    const PARENT_PORT_SHIFT: u32 = 8;
    const TT_TT_MASK: u32 = 0x0003_0000; //bits 16..17
    const TT_TT_SHIFT: u32 = 16;
    const INTERRUPTER_MASK: u32 = 0xFFC0_0000; //bits 22..31
    const INTERRUPTER_SHIFT: u32 = 22;

    pub fn get_parent_hub_slot(&self) -> u8 {
        (self.dword2 & Self::PARENT_HUB_SLOT_MASK) as u8
    }

    pub fn set_parent_hub_slot(&mut self, slot: u8) {
        self.dword2 = (self.dword2 & !Self::PARENT_HUB_SLOT_MASK) | slot as u32;
    }

    pub fn get_parent_port(&self) -> u8 {
        ((self.dword2 & Self::PARENT_PORT_MASK) >> Self::PARENT_PORT_SHIFT) as u8
    }

    pub fn set_parent_port(&mut self, port: u8) {
        self.dword2 =
            (self.dword2 & !Self::PARENT_PORT_MASK) | ((port as u32) << Self::PARENT_PORT_SHIFT);
    }

    pub fn get_tt_think_time(&self) -> u8 {
        ((self.dword2 & Self::TT_TT_MASK) >> Self::TT_TT_SHIFT) as u8
    }

    pub fn set_tt_think_time(&mut self, ttt: u8) {
        self.dword2 = (self.dword2 & !Self::TT_TT_MASK) | ((ttt as u32) << Self::TT_TT_SHIFT);
    }

    pub fn get_interrupter_target(&self) -> u16 {
        ((self.dword2 & Self::INTERRUPTER_MASK) >> Self::INTERRUPTER_SHIFT) as u16
    }

    pub fn set_interrupter_target(&mut self, intr: u16) {
        self.dword2 =
            (self.dword2 & !Self::INTERRUPTER_MASK) | ((intr as u32) << Self::INTERRUPTER_SHIFT);
    }

    /* ================= DWORD 3 ================= */

    const USB_ADDRESS_MASK: u32 = 0x0000_00FF; //bits 0..7
    const SLOT_STATE_MASK: u32 = 0xF800_0000; //bits 27..31
    const SLOT_STATE_SHIFT: u32 = 27;

    pub fn get_usb_address(&self) -> u8 {
        (self.dword3 & Self::USB_ADDRESS_MASK) as u8
    }

    pub fn set_usb_address(&mut self, addr: u8) {
        self.dword3 = (self.dword3 & !Self::USB_ADDRESS_MASK) | addr as u32;
    }

    pub fn get_slot_state(&self) -> u8 {
        ((self.dword3 & Self::SLOT_STATE_MASK) >> Self::SLOT_STATE_SHIFT) as u8
    }
}

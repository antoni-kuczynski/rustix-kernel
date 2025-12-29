/*
 * Created by Antoni Kuczyński
 * 29/12/2025
 */

/*
==============================================================
    SOURCES:
    https://cdrdv2.intel.com/v1/dl/getContent/868296 - XHCI Intel specification Rev 2.0
==============================================================
 */
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::max;
use core::ptr;
use bootloader::BootInfo;
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_device::PciDeviceInitError::{InvalidBarType, TimeoutError};
use crate::interrupts::hardware::pic8259::{get_ticks, pic_get_ticks_per_ms};
/*
Base
│
├─ Capability Registers        (offset 0x00)
│   └─ CAPLENGTH (offset 0x00)
│
├─ Operational Registers       (offset = CAPLENGTH)
│
├─ Runtime Registers           (offset = RTSOFF)
│
└─ Doorbell Registers          (offset = DBOFF)

op_base = base + caplength

 */

//CAPABILITY REGS
const CAP_REG_CAPLENGTH: u8 = 0x00;
const CAP_REG_HCSPARAMS1: u8 = 0x04;


//OPERATIONAL REGS
const OP_REG_USBSTS: u8 = 0x04;
const OP_REG_CONFIG: u8 = 0x38;
const OP_REG_DCBAAP: u8 = 0x30;

//DEVICE CONTEXT BASE ARRAY

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

    const ROUTE_STRING_MASK: u32   = 0x000F_FFFF; //bits 0..19
    const SPEED_MASK: u32          = 0x00F0_0000; //bits 20..23
    const SPEED_SHIFT: u32         = 20;
    const MULTI_TT_MASK: u32       = 0x0200_0000; //bit 25
    const HUB_MASK: u32            = 0x0400_0000; //bit 26
    const CONTEXT_ENTRIES_MASK: u32= 0xF800_0000; //bits 27..31
    const CONTEXT_ENTRIES_SHIFT: u32 = 27;

    pub fn get_route_string(&self) -> u32 {
        self.dword0 & Self::ROUTE_STRING_MASK
    }

    pub fn set_route_string(&mut self, route: u32) {
        self.dword0 =
            (self.dword0 & !Self::ROUTE_STRING_MASK) |
                (route & Self::ROUTE_STRING_MASK);
    }

    pub fn get_speed(&self) -> u8 {
        ((self.dword0 & Self::SPEED_MASK) >> Self::SPEED_SHIFT) as u8
    }

    pub fn set_speed(&mut self, speed: u8) {
        self.dword0 =
            (self.dword0 & !Self::SPEED_MASK) |
                ((speed as u32) << Self::SPEED_SHIFT);
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
        ((self.dword0 & Self::CONTEXT_ENTRIES_MASK)
            >> Self::CONTEXT_ENTRIES_SHIFT) as u8
    }

    pub fn set_context_entries(&mut self, entries: u8) {
        self.dword0 =
            (self.dword0 & !Self::CONTEXT_ENTRIES_MASK) |
                ((entries as u32) << Self::CONTEXT_ENTRIES_SHIFT);
    }

    /* ================= DWORD 1 ================= */

    const MAX_EXIT_LATENCY_MASK: u32 = 0x0000_FFFF; //bits 0..15
    const ROOT_HUB_PORT_MASK: u32    = 0x00FF_0000; //bits 16..23
    const ROOT_HUB_PORT_SHIFT: u32   = 16;
    const NUM_PORTS_MASK: u32        = 0xFF00_0000; //bits 24...31
    const NUM_PORTS_SHIFT: u32       = 24;

    pub fn get_max_exit_latency(&self) -> u16 {
        (self.dword1 & Self::MAX_EXIT_LATENCY_MASK) as u16
    }

    pub fn set_max_exit_latency(&mut self, latency: u16) {
        self.dword1 =
            (self.dword1 & !Self::MAX_EXIT_LATENCY_MASK) |
                latency as u32;
    }

    pub fn get_root_hub_port(&self) -> u8 {
        ((self.dword1 & Self::ROOT_HUB_PORT_MASK)
            >> Self::ROOT_HUB_PORT_SHIFT) as u8
    }

    pub fn set_root_hub_port(&mut self, port: u8) {
        self.dword1 =
            (self.dword1 & !Self::ROOT_HUB_PORT_MASK) |
                ((port as u32) << Self::ROOT_HUB_PORT_SHIFT);
    }

    pub fn get_num_ports(&self) -> u8 {
        ((self.dword1 & Self::NUM_PORTS_MASK)
            >> Self::NUM_PORTS_SHIFT) as u8
    }

    pub fn set_num_ports(&mut self, ports: u8) {
        self.dword1 =
            (self.dword1 & !Self::NUM_PORTS_MASK) |
                ((ports as u32) << Self::NUM_PORTS_SHIFT);
    }

    /* ================= DWORD 2 ================= */

    const PARENT_HUB_SLOT_MASK: u32 = 0x0000_00FF; //bits 0..7
    const PARENT_PORT_MASK: u32     = 0x0000_FF00; //bits 8..15
    const PARENT_PORT_SHIFT: u32    = 8;
    const TT_TT_MASK: u32           = 0x0003_0000; //bits 16..17
    const TT_TT_SHIFT: u32          = 16;
    const INTERRUPTER_MASK: u32     = 0xFFC0_0000; //bits 22..31
    const INTERRUPTER_SHIFT: u32    = 22;

    pub fn get_parent_hub_slot(&self) -> u8 {
        (self.dword2 & Self::PARENT_HUB_SLOT_MASK) as u8
    }

    pub fn set_parent_hub_slot(&mut self, slot: u8) {
        self.dword2 =
            (self.dword2 & !Self::PARENT_HUB_SLOT_MASK) |
                slot as u32;
    }

    pub fn get_parent_port(&self) -> u8 {
        ((self.dword2 & Self::PARENT_PORT_MASK)
            >> Self::PARENT_PORT_SHIFT) as u8
    }

    pub fn set_parent_port(&mut self, port: u8) {
        self.dword2 =
            (self.dword2 & !Self::PARENT_PORT_MASK) |
                ((port as u32) << Self::PARENT_PORT_SHIFT);
    }

    pub fn get_tt_think_time(&self) -> u8 {
        ((self.dword2 & Self::TT_TT_MASK)
            >> Self::TT_TT_SHIFT) as u8
    }

    pub fn set_tt_think_time(&mut self, ttt: u8) {
        self.dword2 =
            (self.dword2 & !Self::TT_TT_MASK) |
                ((ttt as u32) << Self::TT_TT_SHIFT);
    }

    pub fn get_interrupter_target(&self) -> u16 {
        ((self.dword2 & Self::INTERRUPTER_MASK)
            >> Self::INTERRUPTER_SHIFT) as u16
    }

    pub fn set_interrupter_target(&mut self, intr: u16) {
        self.dword2 =
            (self.dword2 & !Self::INTERRUPTER_MASK) |
                ((intr as u32) << Self::INTERRUPTER_SHIFT);
    }

    /* ================= DWORD 3 ================= */

    const USB_ADDRESS_MASK: u32 = 0x0000_00FF; //bits 0..7
    const SLOT_STATE_MASK: u32   = 0xF800_0000; //bits 27..31
    const SLOT_STATE_SHIFT: u32  = 27;

    pub fn get_usb_address(&self) -> u8 {
        (self.dword3 & Self::USB_ADDRESS_MASK) as u8
    }

    pub fn set_usb_address(&mut self, addr: u8) {
        self.dword3 =
            (self.dword3 & !Self::USB_ADDRESS_MASK) |
                addr as u32;
    }

    pub fn get_slot_state(&self) -> u8 {
        ((self.dword3 & Self::SLOT_STATE_MASK)
            >> Self::SLOT_STATE_SHIFT) as u8
    }
}


//ENDPOINT CONTEXT

/*
Endpoint Context – bit fields

DWORD 0 (Offset 00h):
Bits  2:0   EP_STATE
Bits  7:3   Reserved
Bits  9:8   MULT
Bits 14:10  MAX_PSTREAMS
Bit   15    LSA
Bits 23:16  INTERVAL
Bits 31:24  MAX_ESIT_PAYLOAD_HI

DWORD 1 (Offset 04h):
Bits  1:0   CERR
Bits  2     Reserved
Bits  5:3   EP_TYPE
Bit   6     Reserved
Bit   7     HID
Bits 15:8   MAX_BURST_SIZE
Bits 31:16  MAX_PACKET_SIZE

DWORD 2 (Offset 08h):
Bit   0     DCS
Bits 3:1    Reserved
Bits 63:4   TR_DEQUEUE_POINTER

DWORD 3 (Offset 10h):
Bits 15:0   AVERAGE_TRB_LENGTH
Bits 31:16  MAX_ESIT_PAYLOAD_LO

DWORD 4-5 (Offset 14h-1Fh):
Reserved (xHCI)

DWORD 6-7 (Offset 20h-27h):
Reserved (xHCI)
*/
#[repr(C, packed)]
pub struct EndpointContext<const CZ: usize> {
    dword0: u32, // 0x00
    dword1: u32, // 0x04
    dword2: u32, // 0x08 (TR Dequeue Ptr Lo + DCS)
    dword3: u32, // 0x0C (TR Dequeue Ptr Hi)
    dword4: u32, // 0x10
    reserved: [u32; CZ], // 0x14–0x1F
}


/*
Endpoint State (EP State). The Endpoint State identifies the current operational state of the
endpoint.
Value Definition
0 Disabled The endpoint is not operational
1 Running The endpoint is operational, either waiting for a doorbell ring or processing
TDs.
2 HaltedThe endpoint is halted due to a Halt condition detected on the USB. SW shall issue
Reset Endpoint Command to recover from the Halt condition and transition to the Stopped state.
SW may manipulate the Transfer Ring while in this state.
3 Stopped The endpoint is not running due to a Stop Endpoint Command or recovering
from a Halt condition. SW may manipulate the Transfer Ring while in this state.
4 Error The endpoint is not running due to a TRB Error. SW may manipulate the Transfer
Ring while in this state.
5-7 Reserved
As Output, a Running to Halted transition is forced by the xHC if a STALL condition is detected
on the endpoint. A Running to Error transition is forced by the xHC if a TRB Error condition is
detected.
As Input, this field is initialized to ‘0’ by software.
Refer to section 4.8.3 for more information on Endpoint State.

Mult. If LEC = ‘0’, then this field indicates the maximum number of bursts within an Interval that
this endpoint supports. Mult is a “zero-based” value, where 0 to 3 represents 1 to 4 bursts,
respectively. The valid range of values is ‘0’ to ‘2’.117 This field shall be ‘0’ for all endpoint types
except for SS Isochronous.
If LEC = ‘1’, then this field shall be RsvdZ and Mult is calculated as:
ROUNDUP(Max ESIT Payload / Max Packet Size / (Max Burst Size + 1)) - 1.

Max Primary Streams (MaxPStreams). This field identifies the maximum number of Primary
Stream IDs this endpoint supports. Valid values are defined below. If the value of this field is ‘0’,
then the TR Dequeue Pointer field shall point to a Transfer Ring. If this field is > '0' then the TR
Dequeue Pointer field shall point to a Primary Stream Context Array. Refer to section 4.12 for
more information.
A value of ‘0’ indicates that Streams are not supported by this endpoint and the Endpoint
Context TR Dequeue Pointer field references a Transfer Ring.
A value of ‘1’ to ‘15’ indicates that the Primary Stream ID Width is MaxPstreams+1 and the
Primary Stream Array contains 2MaxPStreams+1 entries.
For SS Bulk endpoints, the range of valid values for this field is defined by the MaxPSASize field
in the HCCPARAMS1 register (refer to Table 5-13).
This field shall be '0' for all SS Control, Isoch, and Interrupt endpoints, and for all non-SS
endpoints.

Linear Stream Array (LSA). This field identifies how a Stream ID shall be interpreted.
Setting this bit to a value of ‘1’ shall disable Secondary Stream Arrays and a Stream ID shall be
interpreted as a linear index into the Primary Stream Array, where valid values for MaxPStreams
are ‘1’ to ‘15’.
A value of ‘0’ shall enable Secondary Stream Arrays, where the low order (MaxPStreams+1) bits
of a Stream ID shall be interpreted as a linear index into the Primary Stream Array, where valid
values for MaxPStreams are ‘1’ to ‘7’. And the high order bits of a Stream ID shall be interpreted
as a linear index into the Secondary Stream Array.
If MaxPStreams = ‘0’, this field RsvdZ.
Refer to section 4.12.2 for more information.

Interval. The period between consecutive requests to a USB endpoint to send or receive data.
Expressed in 125 μs. increments. The period is calculated as 125 μs. * 2Interval; e.g., an Interval
value of 0 means a period of 125 μs. (20 = 1 * 125 μs.), a value of 1 means a period of 250 μs. (21
= 2 * 125 μs.), a value of 4 means a period of 2 ms. (24 = 16 * 125 μs.), etc. Refer to Table 6-12 for
legal Interval field values. See further discussion of this field below. Refer to section 6.2.3.6 for
more information.

Max Endpoint Service Time Interval Payload High (Max ESIT Payload Hi). If LEC = '1', then this
field indicates the high order 8 bits of the Max ESIT Payload value. If LEC = '0', then this field
shall be RsvdZ. Refer to section 6.2.3.8 for more information


//==================== Offset 04h – Endpoint Context Field Definitions ==========================

Error Count (CErr)118. This field defines a 2-bit down count, which identifies the number of
consecutive USB Bus Errors allowed while executing a TD. If this field is programmed with a non-
zero value when the Endpoint Context is initialized, the xHC loads this value into an internal Bus
Error Counter before executing a USB transaction and decrements it if the transaction fails. If the
Bus Error Counter counts from ‘1’ to ‘0’, the xHC ceases execution of the TRB, sets the endpoint
to the Halted state, and generates a USB Transaction Error Event for the TRB that caused the
internal Bus Error Counter to decrement to ‘0’. If system software programs this field to ‘0’, the
xHC shall not count errors for TRBs on the Endpoint’s Transfer Ring and there shall be no limit
on the number of TRB retries. Refer to section 4.10.2.7 for more information on the operation of
the Bus Error Counter.
Note: CErr does not apply to Isoch endpoints and shall be set to ‘0’ if EP Type = Isoch Out ('1') or
Isoch In ('5').

Endpoint Type (EP Type). This field identifies whether an Endpoint Context is Valid, and if so,
what type of endpoint the context defines.
Value Endpoint Type Direction
0 Not Valid N/A
1 Isoch Out
2 Bulk Out
3 Interrupt Out
4 Control Bidirectional
5 Isoch In
6 Bulk In
7 Interrupt In

Host Initiate Disable (HID). This field affects Stream enabled endpoints, allowing the Host
Initiated Stream selection feature to be disabled for the endpoint. Setting this bit to a value of ‘1’
shall disable the Host Initiated Stream selection feature. A value of ‘0’ will enable normal Stream
operation. Refer to section 4.12.1.1 for more information

Max Burst Size. This field indicates to the xHC the maximum number of consecutive USB
transactions that should be executed per scheduling opportunity. This is a “zero-based” value,
where 0 to 15 represents burst sizes of 1 to 16, respectively. Refer to section 6.2.3.4 for more
information.

Max Packet Size. This field indicates the maximum packet size in bytes that this endpoint is
capable of sending or receiving when configured. Refer to section 6.2.3.5 for more information


//========================== Offset 08h – Endpoint Context Field Definitions ======================

Dequeue Cycle State (DCS). This bit identifies the value of the xHC Consumer Cycle State (CCS)
flag for the TRB referenced by the TR Dequeue Pointer. Refer to section 4.9.2 for more
information. This field shall be ‘0’ if MaxPStreams > ‘0’

TR Dequeue Pointer. As Input, this field represents the high order bits of the 64-bit base
address of a Transfer Ring or a Stream Context Array associated with this endpoint. If
MaxPStreams = '0' then this field shall point to a Transfer Ring. If MaxPStreams > '0' then this
field shall point to a Stream Context Array.
As Output, if MaxPStreams = ‘0’ this field shall be used by the xHC to store the value of the
Dequeue Pointer when the endpoint enters the Halted or Stopped states, and the value of the
this field shall be undefined when the endpoint is not in the Halted or Stopped states. if
MaxPStreams > ‘0’ then this field shall point to a Stream Context Array.
The memory structure referenced by this physical memory pointer shall be aligned to a 16-byte
boundary

//======================= Offset 10h – Endpoint Context Field Definition ========================

Average TRB Length. This field represents the average Length of the TRBs executed by this
endpoint. The value of this field shall be greater than ‘0’. Refer to section 4.14.1.1 and the
implementation note TRB Lengths and System Bus Bandwidth for more information.
The xHC shall use this parameter to calculate system bus bandwidth requirements.


Max Endpoint Service Time Interval Payload Low (Max ESIT Payload Lo). This field indicates
the low order 16 bits of the Max ESIT Payload. The Max ESIT Payload represents the total
number of bytes this endpoint will transfer during an ESIT. This field is only valid for periodic
endpoints. Refer to section 6.2.3.8 for more information
 */
impl<const CZ: usize> EndpointContext<CZ> {
    /* ================= DWORD 0 (0x00) ================= */

    const EP_STATE_MASK: u32 = 0b111;
    const EP_STATE_SHIFT: u32 = 0;

    const MULT_MASK: u32 = 0b11 << 8;
    const MULT_SHIFT: u32 = 8;

    const MAX_PSTREAMS_MASK: u32 = 0b1_1111 << 10;
    const MAX_PSTREAMS_SHIFT: u32 = 10;

    const LSA_MASK: u32 = 1 << 15;

    const INTERVAL_MASK: u32 = 0xFF << 16;
    const INTERVAL_SHIFT: u32 = 16;

    const MAX_ESIT_PAYLOAD_HI_MASK: u32 = 0xFF << 24;
    const MAX_ESIT_PAYLOAD_HI_SHIFT: u32 = 24;

    pub fn get_ep_state(&self) -> u32 {
        (self.dword0 & Self::EP_STATE_MASK) >> Self::EP_STATE_SHIFT
    }

    pub fn set_ep_state(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::EP_STATE_MASK) |
                ((val << Self::EP_STATE_SHIFT) & Self::EP_STATE_MASK);
    }

    pub fn get_mult(&self) -> u32 {
        (self.dword0 & Self::MULT_MASK) >> Self::MULT_SHIFT
    }

    pub fn set_mult(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::MULT_MASK) |
                ((val << Self::MULT_SHIFT) & Self::MULT_MASK);
    }

    pub fn get_max_pstreams(&self) -> u32 {
        (self.dword0 & Self::MAX_PSTREAMS_MASK) >> Self::MAX_PSTREAMS_SHIFT
    }

    pub fn set_max_pstreams(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::MAX_PSTREAMS_MASK) |
                ((val << Self::MAX_PSTREAMS_SHIFT) & Self::MAX_PSTREAMS_MASK);
    }

    pub fn lsa(&self) -> bool {
        (self.dword0 & Self::LSA_MASK) != 0
    }

    pub fn set_lsa(&mut self, on: bool) {
        if on {
            self.dword0 |= Self::LSA_MASK;
        } else {
            self.dword0 &= !Self::LSA_MASK;
        }
    }

    pub fn get_interval(&self) -> u32 {
        (self.dword0 & Self::INTERVAL_MASK) >> Self::INTERVAL_SHIFT
    }

    pub fn set_interval(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::INTERVAL_MASK) |
                ((val << Self::INTERVAL_SHIFT) & Self::INTERVAL_MASK);
    }

    pub fn get_max_esit_payload_hi(&self) -> u32 {
        (self.dword0 & Self::MAX_ESIT_PAYLOAD_HI_MASK)
            >> Self::MAX_ESIT_PAYLOAD_HI_SHIFT
    }

    pub fn set_max_esit_payload_hi(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::MAX_ESIT_PAYLOAD_HI_MASK) |
                ((val << Self::MAX_ESIT_PAYLOAD_HI_SHIFT)
                    & Self::MAX_ESIT_PAYLOAD_HI_MASK);
    }

    /* ================= DWORD 1 (0x04) ================= */

    const CERR_MASK: u32 = 0b11 << 1;
    const CERR_SHIFT: u32 = 1;

    const EP_TYPE_MASK: u32 = 0b111 << 3;
    const EP_TYPE_SHIFT: u32 = 3;

    const HID_MASK: u32 = 1 << 7;

    const MAX_BURST_MASK: u32 = 0xFF << 8;
    const MAX_BURST_SHIFT: u32 = 8;

    const MAX_PACKET_SIZE_MASK: u32 = 0xFFFF << 16;
    const MAX_PACKET_SIZE_SHIFT: u32 = 16;

    pub fn get_cerr(&self) -> u32 {
        (self.dword1 & Self::CERR_MASK) >> Self::CERR_SHIFT
    }

    pub fn set_cerr(&mut self, val: u32) {
        self.dword1 =
            (self.dword1 & !Self::CERR_MASK) |
                ((val << Self::CERR_SHIFT) & Self::CERR_MASK);
    }

    pub fn get_ep_type(&self) -> u32 {
        (self.dword1 & Self::EP_TYPE_MASK) >> Self::EP_TYPE_SHIFT
    }

    pub fn set_ep_type(&mut self, val: u32) {
        self.dword1 =
            (self.dword1 & !Self::EP_TYPE_MASK) |
                ((val << Self::EP_TYPE_SHIFT) & Self::EP_TYPE_MASK);
    }

    pub fn hid(&self) -> bool {
        (self.dword1 & Self::HID_MASK) != 0
    }

    pub fn set_hid(&mut self, on: bool) {
        if on {
            self.dword1 |= Self::HID_MASK;
        } else {
            self.dword1 &= !Self::HID_MASK;
        }
    }

    pub fn get_max_burst(&self) -> u32 {
        (self.dword1 & Self::MAX_BURST_MASK) >> Self::MAX_BURST_SHIFT
    }

    pub fn set_max_burst(&mut self, val: u32) {
        self.dword1 =
            (self.dword1 & !Self::MAX_BURST_MASK) |
                ((val << Self::MAX_BURST_SHIFT) & Self::MAX_BURST_MASK);
    }

    pub fn get_max_packet_size(&self) -> u32 {
        (self.dword1 & Self::MAX_PACKET_SIZE_MASK)
            >> Self::MAX_PACKET_SIZE_SHIFT
    }

    pub fn set_max_packet_size(&mut self, val: u32) {
        self.dword1 =
            (self.dword1 & !Self::MAX_PACKET_SIZE_MASK) |
                ((val << Self::MAX_PACKET_SIZE_SHIFT)
                    & Self::MAX_PACKET_SIZE_MASK);
    }

    /* ================= TR Dequeue Pointer (0x08–0x0F) ================= */

    const DCS_MASK: u32 = 1;

    pub fn dcs(&self) -> bool {
        (self.dword2 & Self::DCS_MASK) != 0
    }

    pub fn set_dcs(&mut self, on: bool) {
        if on {
            self.dword2 |= Self::DCS_MASK;
        } else {
            self.dword2 &= !Self::DCS_MASK;
        }
    }

    pub fn get_tr_dequeue_ptr(&self) -> u64 {
        let lo = (self.dword2 & !0xF) as u64;
        let hi = self.dword3 as u64;
        (hi << 32) | lo
    }

    pub fn set_tr_dequeue_ptr(&mut self, addr: u64) {
        self.dword2 = (self.dword2 & 0xF) | ((addr as u32) & !0xF);
        self.dword3 = (addr >> 32) as u32;
    }

    /* ================= DWORD 4 (0x10) ================= */

    const AVG_TRB_LEN_MASK: u32 = 0xFFFF;
    const MAX_ESIT_PAYLOAD_LO_MASK: u32 = 0xFFFF << 16;

    pub fn get_avg_trb_len(&self) -> u32 {
        self.dword4 & Self::AVG_TRB_LEN_MASK
    }

    pub fn set_avg_trb_len(&mut self, val: u32) {
        self.dword4 =
            (self.dword4 & !Self::AVG_TRB_LEN_MASK) |
                (val & Self::AVG_TRB_LEN_MASK);
    }

    pub fn get_max_esit_payload_lo(&self) -> u32 {
        (self.dword4 & Self::MAX_ESIT_PAYLOAD_LO_MASK) >> 16
    }

    pub fn set_max_esit_payload_lo(&mut self, val: u32) {
        self.dword4 =
            (self.dword4 & !Self::MAX_ESIT_PAYLOAD_LO_MASK) |
                ((val << 16) & Self::MAX_ESIT_PAYLOAD_LO_MASK);
    }
}

//===================================================================
//              DEVICE CONTEXT
//===================================================================
#[repr(C, packed)]
pub struct DeviceContext<const CZ: usize> {
    slot: SlotContext<CZ>,
    endpoints: [EndpointContext<CZ>; 31],
}

//===================================================================
//              Device Context Base Address Array
//===================================================================
#[repr(C, align(64))]
pub struct Dcbaa {
    entries: [u64; 256], //max 256 entries, < 2kb
}

impl Dcbaa {
    pub fn get_context(&self, slot_id: usize) -> u64 {
        self.entries[slot_id]
    }

    pub fn set_context(&mut self, slot_id: usize, addr: u64) {
        self.entries[slot_id] = addr;
    }

    pub fn clear_context(&mut self, slot_id: usize) {
        self.entries[slot_id] = 0;
    }
}

pub struct XHCI<'a> {
    pci_device: &'a PciDeviceHeader,

    slots: u32,
    context_size: u32,
    dcbaa_ptr: u64
}

impl<'a> XHCI<'a> {
    fn new(dev: &'a PciDeviceHeader) -> Self {
        XHCI {
            pci_device: dev,
            slots: 0,
            context_size: 0,
            dcbaa_ptr: 0
        }
    }
}


impl PciDeviceInitializer for XHCI<'_> {
    fn initialize(pci_device: &PciDeviceHeader, boot_info: &BootInfo) -> Result<(), PciDeviceInitError> {
        let bar = PciBAR::get(pci_device, 0);

        if bar.bar_type() == &BarType::Io {
            return Err(InvalidBarType);
        }
        let xhci_controller = XHCI::new(&pci_device);

        unsafe {
            let base = bar.base_address() + boot_info.physical_memory_offset;
            let op_base = base + ptr::read_volatile((base + CAP_REG_CAPLENGTH as u64) as *const u8) as u64;


            let mut usbsts = ptr::read_volatile((op_base + OP_REG_USBSTS as u64) as *const u32);
            let mut ticks = get_ticks();
            let max_ticks = (pic_get_ticks_per_ms() * 1000) + ticks;

            const CONTROLLER_READY: u32 = 0;
            while ticks < max_ticks {
                usbsts = ptr::read_volatile((op_base + OP_REG_USBSTS as u64) as *const u32);
                if usbsts & 0x800 == CONTROLLER_READY {
                    break;
                }
                ticks = get_ticks();
            }

            if usbsts & 0x800 != CONTROLLER_READY {
                return Err(TimeoutError);
            }

            let hccparams1: u32 = ptr::read_volatile((base + CAP_REG_HCSPARAMS1 as u64) as *mut u32);

            //enable all slots
            {
                let max_slots: u32 =  hccparams1 & 0xFF;
                let config_reg = ptr::read_volatile((op_base + OP_REG_CONFIG as u64) as *const u32);
                ptr::write_volatile((op_base + OP_REG_CONFIG as u64) as *mut u32, config_reg | max_slots);
            }

            let context_size: u8 = if hccparams1 & 0x02 == 1 {
                64
            } else {
                32
            };

            let dcbaa = Box::new(Dcbaa {
                entries: [0; 256]
            });

            ptr::write_volatile((op_base + OP_REG_DCBAAP as u64) as *mut u64, Box::into_raw(dcbaa) as u64);


        }

        Ok(())
    }
}
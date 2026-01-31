/*
 * Created by Antoni Kuczyński
 * 29/12/2025
 */
#![allow(dead_code)]


/*
==============================================================
    SOURCES:
    https://cdrdv2.intel.com/v1/dl/getContent/868296 - XHCI Intel specification Rev 2.0
==============================================================
 */
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use bootloader::BootInfo;
use x86_64::structures::paging::OffsetPageTable;
use x86_64::VirtAddr;
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_device::PciDeviceInitError::{InitializationFailure, InvalidBarType, NoMSIXCapabilities, TimeoutError};
use crate::drivers::pci::pci_io::{pci_read16, pci_read32, pci_read8, pci_write16};
use crate::drivers::usb::interrupts::xhci_interrupt_handler::XHCIInterruptIndex;
use crate::interrupts::hardware::pic8259::{get_ticks, pic_get_ticks_per_ms};
use crate::memory::pages::virtual_to_physical;
use crate::vgaprintln;
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


The Runtime Base shall be 32-
byte aligned and is calculated by adding the value Runtime Register Space
Offset register (refer to Section 5.3.8) to the Capability Base address. All
Runtime registers are multiples of 32 bits in length.

runtime_base = RTSOFF +

 */

//CAPABILITY REGS
const CAP_REG_CAPLENGTH: u8 = 0x00;
const CAP_REG_HCSPARAMS1: u8 = 0x04;
const CAP_REG_RTSOFF: u8 = 0x18;


//OPERATIONAL REGS
const OP_REG_USBSTS: u8 = 0x04;
const OP_REG_CONFIG: u8 = 0x38;
const OP_REG_DCBAAP: u8 = 0x30;
const OP_REG_CRCR: u8 = 0x18;


//RUNTIME REGISTERS
const RT_ERSTSZ: u8 = 0x28;
const RT_ERSTBA: u8 = 0x30;

// ============================================================================
// xHCI Data Structure Requirements (from spec sections 4.x / 6.x)
// ============================================================================
//
//  Name                               Max Size      Boundary      Align   Spec
//  ---------------------------------------------------------------------------
//  Device Context Base Address Array   2048 bytes    PAGESIZE      64     §6.1
//  Device Context                      2048 bytes    PAGESIZE      64     §6.2.1
//  Input Control Context               64 bytes      PAGESIZE      64     §6.2.5.1
//  Slot Context                        64 bytes      PAGESIZE      32     §6.2.2
//  Endpoint Context                    64 bytes      PAGESIZE      32     §6.2.3
//  Stream Context                      16 bytes      PAGESIZE      16     §6.2.4.1
//  Stream Array (Linear)               1 MB          None          16     §6.2.4
//  Stream Array (Primary/Secondary)    4 KB          PAGESIZE      16     §6.2.4
//
//  Transfer Ring segments              64 KB         64 KB         16     §4.9.2
//  Command Ring segments               64 KB         64 KB         64     §4.9.3
//  Event Ring segments                 64 KB         64 KB         64     §4.9.4
//
//  Event Ring Segment Table            512 KB        None          64     §6.5
//
//  Scratchpad Buffer Array             2^48 bytes    PAGESIZE      64     §6.6
//  Scratchpad Buffers                  PAGESIZE      PAGESIZE      Page   §4.20
//
// ============================================================================
//
// Notes:
// - “Boundary Requirement” means the structure must not cross that boundary.
// - “Alignment” is the minimum alignment of the base address.
// - Transfer/Command/Event ring *segments* must be ≤ 64 KB and aligned to 64 KB.
// - Device/Slot/Endpoint contexts must be page-aligned and meet their alignment.
// - Scratchpad buffers must be page-aligned and page-sized.
// ============================================================================
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
If LEC = ‘1’, then this field sA TRB (Transfer Request Block) Ring defines a queue, which is used to transfer
Work Items between producer and consumer entities
26
.

A TRB Ring is defined as a circular queue of TRB data structures. TRB rings are
used to pass

Work Items

from the producer to the consumer. Two pointers
(Enqueue and Dequeue) associated with each ring identify where the producer
will Enqueue the next Work Item on the ring and where the consumer will
Dequeue the next Work Item from the ring.

A Work Item is comprised of one or more TRB data structures. A Work Item may
define an operation to perform, or the result of an operation that has been
performed.

There are 3 basic types or TRB Rings;

Transfer, Event
, and

Command
. Each type
of ring defines an exclusive set of TRB data structures; however they all employ
the underlying TRB Ring mechanism to organize their work items and the basic
TRB template.

Transfer Rings

provide data transport to and from USB devices. There is a 1:1
mapping between Transfer Rings and USB Pipes. They are defined by an
Endpoint Context data structure contained in a Device Context, or the Stream
Context Array pointed to by the Endpoint Conte
xt.

The

Event Ring

provides the xHC with a means of reporting to system software:
data transfer and command completion status, Root Hub port status changes,
and other xHC related events. An Event Ring is defined by the Event Ring
Segment Table Base Address, Segment Table Si
ze, and Dequeue Pointer
registers which reside in the Runtime Registers.

The

Command Ring

provides system software the ability to issue commands to
enumerate USB Devices, configure the xHC to support those devices, and to
coordinate virtualization features. The Command Ring is managed by the
Command Ring Control Register that resides in the Op
erational Registers.

The

Enqueue Pointer

and

Dequeue Pointer

are terms used to refer to the
logical beginning and end of the valid entries in a TRB Ring. The size of a TRB

26

Note: The xHCI Producer/Consumer model is not related to the

PCI

Producer/Consumer model.
200

Document Number:
868296
, Revision:

2.0

ring is determined by the number and size of the segments that comprise the
ring.hall be RsvdZ and Mult is calculated as:
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

//=========================================
//  TRB
//=========================================
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Trb {
    pub parameter: u64, //pointer / value
    pub status: u32,    //length, residual...
    pub control: u32,   //type, cycle, flags
}

impl Trb {
    // ====== CONSTANTS ======
    const CYCLE_BIT: u32 = 1 << 0;
    const CHAIN_BIT: u32 = 1 << 4;

    const TRB_TYPE_SHIFT: u32 = 10;
    const TRB_TYPE_MASK:  u32 = 0x3F << Self::TRB_TYPE_SHIFT;

    // Length is usually bits 0–16 of status
    const LENGTH_MASK: u32 = 0x1FFFF;

    // ===== TRB TYPE CONSTANTS (Table 6‑91) =====
    pub const TRB_RESERVED0: u8 = 0;
    pub const TRB_NORMAL: u8 = 1;
    pub const TRB_SETUP_STAGE: u8 = 2;
    pub const TRB_DATA_STAGE: u8 = 3;
    pub const TRB_STATUS_STAGE: u8 = 4;
    pub const TRB_ISOCH: u8 = 5;
    pub const TRB_LINK: u8 = 6;
    pub const TRB_EVENT_DATA: u8 = 7;
    pub const TRB_NO_OP: u8 = 8;

    pub const TRB_ENABLE_SLOT: u8 = 9;
    pub const TRB_DISABLE_SLOT: u8 = 10;
    pub const TRB_ADDRESS_DEVICE: u8 = 11;
    pub const TRB_CONFIGURE_ENDPOINT: u8 = 12;
    pub const TRB_EVALUATE_CONTEXT: u8 = 13;
    pub const TRB_RESET_ENDPOINT: u8 = 14;
    pub const TRB_STOP_ENDPOINT: u8 = 15;
    pub const TRB_SET_DEQUEUE_PTR: u8 = 16;
    pub const TRB_RESET_DEVICE: u8 = 17;
    pub const TRB_FORCE_EVENT: u8 = 18;
    pub const TRB_NEGOTIATE_BW: u8 = 19;
    pub const TRB_SET_LTV: u8 = 20;
    pub const TRB_GET_PORT_BW: u8 = 21;
    pub const TRB_FORCE_HEADER: u8 = 22;
    pub const TRB_NO_OP_CMD: u8 = 23;
    pub const TRB_GET_EXT_PROP: u8 = 24;
    pub const TRB_SET_EXT_PROP: u8 = 25;

    pub const TRB_TRANSFER_EVENT: u8 = 32;
    pub const TRB_COMMAND_COMPLETION_EVENT: u8 = 33;
    pub const TRB_PORT_STATUS_CHANGE_EVENT: u8 = 34;
    pub const TRB_BW_REQUEST_EVENT: u8 = 35;
    pub const TRB_DOORBELL_EVENT: u8 = 36;
    pub const TRB_HOST_CONTROLLER_EVENT: u8 = 37;
    pub const TRB_DEVICE_NOTIFICATION_EVENT: u8 = 38;
    pub const TRB_MFINDEX_WRAP_EVENT: u8 = 39;

    // ====== PARAMETER ======
    pub fn parameter(&self) -> u64 {
        self.parameter
    }

    pub fn set_parameter(&mut self, value: u64) {
        self.parameter = value;
    }

    // ====== LENGTH (status low bits) ======
    pub fn length(&self) -> u32 {
        self.status & Self::LENGTH_MASK
    }

    pub fn set_length(&mut self, len: u32) {
        self.status = (self.status & !Self::LENGTH_MASK) | (len & Self::LENGTH_MASK);
    }

    // ====== CYCLE BIT ======
    pub fn cycle(&self) -> bool {
        (self.control & Self::CYCLE_BIT) != 0
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        if cycle {
            self.control |= Self::CYCLE_BIT;
        } else {
            self.control &= !Self::CYCLE_BIT;
        }
    }

    // ====== CHAIN BIT ======
    pub fn chain(&self) -> bool {
        (self.control & Self::CHAIN_BIT) != 0
    }

    pub fn set_chain(&mut self, chain: bool) {
        if chain {
            self.control |= Self::CHAIN_BIT;
        } else {
            self.control &= !Self::CHAIN_BIT;
        }
    }

    // ====== TRB TYPE ======
    pub fn trb_type(&self) -> u8 {
        ((self.control & Self::TRB_TYPE_MASK) >> Self::TRB_TYPE_SHIFT) as u8
    }

    pub fn set_trb_type(&mut self, ty: u8) {
        let ctrl = self.control & !Self::TRB_TYPE_MASK;
        self.control = ctrl | ((ty as u32) << Self::TRB_TYPE_SHIFT);
    }

    // ====== RAW CONTROL ======
    pub fn control(&self) -> u32 {
        self.control
    }

    pub fn set_control(&mut self, value: u32) {
        self.control = value;
    }

    // ====== RAW STATUS ======
    pub fn status(&self) -> u32 {
        self.status
    }

    pub fn set_status(&mut self, value: u32) {
        self.status = value;
    }
}
//====================================================
//          TRB RING
//====================================================

/*
A TRB (Transfer Request Block) Ring defines a queue, which is used to transfer
Work Items between producer and consumer entities26.
A TRB Ring is defined as a circular queue of TRB data structures. TRB rings are
used to pass Work Items from the producer to the consumer. Two pointers
(Enqueue and Dequeue) associated with each ring identify where the producer
will Enqueue the next Work Item on the ring and where the consumer will
Dequeue the next Work Item from the ring.
A Work Item is comprised of one or more TRB data structures. A Work Item may
define an operation to perform, or the result of an operation that has been
performed.
There are 3 basic types or TRB Rings; Transfer, Event, and Command. Each type
of ring defines an exclusive set of TRB data structures; however they all employ
the underlying TRB Ring mechanism to organize their work items and the basic
TRB template.
Transfer Rings provide data transport to and from USB devices. There is a 1:1
mapping between Transfer Rings and USB Pipes. They are defined by an
Endpoint Context data structure contained in a Device Context, or the Stream
Context Array pointed to by the Endpoint Context.
The Event Ring provides the xHC with a means of reporting to system software:
data transfer and command completion status, Root Hub port status changes,
and other xHC related events. An Event Ring is defined by the Event Ring
Segment Table Base Address, Segment Table Size, and Dequeue Pointer
registers which reside in the Runtime Registers.
The Command Ring provides system software the ability to issue commands to
enumerate USB Devices, configure the xHC to support those devices, and to
coordinate virtualization features. The Command Ring is managed by the
Command Ring Control Register that resides in the Operational Registers.
The Enqueue Pointer and Dequeue Pointer are terms used to refer to the
logical beginning and end of the valid entries in a TRB Ring. The size of a TRB
26 Note: The xHCI Producer/Consumer model is not related to the PCI Producer/Consumer model.
200 Document Number:868296, Revision: 2.0
ring is determined by the number and size of the segments that comprise the
ring.

 */
#[derive(Debug)]
struct TrbCreationError();

pub struct TrbRing {
    trbs: &'static mut [Trb],
    enqueue_index: usize,
    cycle_state: bool,
}

impl TrbRing {
    pub fn alloc_trb_array(len: usize) -> &'static mut [Trb] {
        let mut v = Vec::with_capacity(len);
        v.resize(len, Trb { parameter: 0, status: 0, control: 0 });

        let slice = v.leak(); //turns Vec into &'static mut [Trb]
        slice
    }


    pub fn new(trbs: &'static mut [Trb], offset_page_table: &OffsetPageTable) -> Result<TrbRing, TrbCreationError> {
        let last_index = trbs.len() - 1;

        //set the last TRB in ring to LINK type
        trbs[last_index].set_trb_type(Trb::TRB_LINK);
        trbs[last_index].set_cycle(true);

        let virt = VirtAddr::new(trbs.as_mut_ptr() as *mut u64 as u64);
        let phys = match virtual_to_physical(virt, offset_page_table) {
            Some(x) => x,
            None => return Err(TrbCreationError())
        };

        trbs[last_index].set_parameter(phys.as_u64());


        Ok(
            TrbRing {
                trbs,
                enqueue_index: 0,
                cycle_state: true
            }
        )
    }
}

//=======================================================
//          EVENT RING SEGMENT TABLE
//=======================================================
/*
The Event Ring Segment Table (ERST) is used to define multi -segment Event
Rings and to enable runtime expansion and shrinking of the Event Ring. The
location of the Event Ring Segment Table is defined by the Event Ring Segment
Table Base Address Register (section 5.5.2.3.2). The size of the Event Ring
Segment Table is defined by the Event Ring Segment Table Base Size Register
(section 5.5.2.3.1).
 */
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ERST {
    ring_addr_low: u32,
    ring_addr_high: u32,
    ring_segment_size: u32,
    rsvdz: u32
}

impl ERST {
    fn new(ring_addr: u64, ring_segment_size: u32) -> ERST {
        assert_eq!(ring_addr & 0x1F, 0, "Event ring must be 32-byte aligned");

        let ring_addr_high: u32 = (ring_addr >> 32) as u32;
        let ring_addr_low: u32 = (ring_addr & 0xFFFFFFE0) as u32;

        Self {
            ring_addr_low,
            ring_addr_high,
            ring_segment_size,
            rsvdz: 0u32
        }
    }

    #[inline(always)]
    pub fn ring_addr_low(&self) -> u32 {
        unsafe {
            let base = self as *const _ as *const u8;
            let ptr = base.add(0) as *const u32;
            ptr::read_unaligned(ptr)
        }
    }

    #[inline(always)]
    pub fn set_ring_addr_low(&mut self, val: u32) {
        unsafe {
            let base = self as *mut _ as *mut u8;
            let ptr = base.add(0) as *mut u32;
            ptr::write_unaligned(ptr, val);
        }
    }

    #[inline(always)]
    pub fn ring_addr_high(&self) -> u32 {
        unsafe {
            let base = self as *const _ as *const u8;
            let ptr = base.add(4) as *const u32;
            ptr::read_unaligned(ptr)
        }
    }

    #[inline(always)]
    pub fn set_ring_addr_high(&mut self, val: u32) {
        unsafe {
            let base = self as *mut _ as *mut u8;
            let ptr = base.add(4) as *mut u32;
            ptr::write_unaligned(ptr, val);
        }
    }

    #[inline(always)]
    pub fn ring_segment_size(&self) -> u32 {
        unsafe {
            let base = self as *const _ as *const u8;
            let ptr = base.add(8) as *const u32;
            ptr::read_unaligned(ptr)
        }
    }

    #[inline(always)]
    pub fn set_ring_segment_size(&mut self, val: u32) {
        unsafe {
            let base = self as *mut _ as *mut u8;
            let ptr = base.add(8) as *mut u32;
            ptr::write_unaligned(ptr, val);
        }
    }
}

//=======================================================
//      MSI-X CONFIGURATION CAPABILITY STRUCTURE
//=======================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MsixCapability {
    pub cap_id: u8,          //0x11
    pub next: u8,            //next capability pointer
    pub message_control: u16,
    pub table: u32,
    pub pba: u32,
}

impl MsixCapability {
    pub unsafe fn new(ptr: *const u8) -> Self {
        ptr::read_unaligned(ptr as *const MsixCapability)
    }

    //Number of MSI-X vectors supported
    pub fn table_size(&self) -> u16 {
        (self.message_control & 0x07FF) + 1
    }

    //Is MSI-X enabled?
    pub fn enabled(&self) -> bool {
        (self.message_control & (1 << 15)) != 0
    }

    //Enable MSI-X
    pub fn enable(&mut self) {
        self.message_control |= 1 << 15;
    }

    //Mask all MSI-X vectors
    pub fn mask_all(&mut self) {
        self.message_control |= 1 << 14;
    }

    //Unmask all MSI-X vectors
    pub fn unmask_all(&mut self) {
        self.message_control &= !(1 << 14);
    }

    //BAR index of the MSI-X table
    pub fn table_bir(&self) -> u8 {
        (self.table & 0x7) as u8
    }

    //Offset of MSI-X table in BAR
    pub fn table_offset(&self) -> u32 {
        self.table & !0x7
    }

    //BAR index of the PBA
    pub fn pba_bir(&self) -> u8 {
        (self.pba & 0x7) as u8
    }

    //Offset of the PBA in BAR
    pub fn pba_offset(&self) -> u32 {
        self.pba & !0x7
    }

    pub fn write_back(&self, pci: &PciDeviceHeader, cap_ptr: u8) {
        // Message Control is at offset +2
        pci_write16(
            pci.base_id(),
            (cap_ptr as u32) + 0x02,
            self.message_control,
        );
    }

    pub fn print(&self) {
        let msg_control = self.message_control;
        let table_bir = (self.table & 0x7) as u8;
        let table_offset = self.table & !0x7;

        let pba_bir = (self.pba & 0x7) as u8;
        let pba_offset = self.pba & !0x7;

        let table_size = (self.message_control & 0x07FF) + 1;
        let function_mask = (self.message_control & (1 << 14)) != 0;
        let msix_enabled = (self.message_control & (1 << 15)) != 0;

        vgaprintln!("MSI-X Capability:");
        vgaprintln!("  Cap ID          : 0x{:02X}", self.cap_id);
        vgaprintln!("  Next Pointer    : 0x{:02X}", self.next);
        vgaprintln!("  Message Control : 0x{:04X}", msg_control);
        vgaprintln!("    Table Size    : {}", table_size);
        vgaprintln!("    Function Mask : {}", function_mask);
        vgaprintln!("    MSI-X Enabled : {}", msix_enabled);
        vgaprintln!("  Table:");
        vgaprintln!("    BIR           : {}", table_bir);
        vgaprintln!("    Offset        : 0x{:08X}", table_offset);
        vgaprintln!("  PBA:");
        vgaprintln!("    BIR           : {}", pba_bir);
        vgaprintln!("    Offset        : 0x{:08X}", pba_offset);
    }
}

//=======================================================
//      MSI-X TABLE ENTRY
//=======================================================
#[repr(C)]
struct MsixTableEntry {
    msg_addr_low:  u32,
    msg_addr_high: u32,
    msg_data:      u32,
    vector_ctrl:   u32,
}

impl MsixTableEntry {
    pub const MASK_BIT: u32 = 1 << 0;

    pub fn is_masked(&self) -> bool {
        self.vector_ctrl & Self::MASK_BIT != 0
    }

    pub fn set_masked(&mut self, masked: bool) {
        if masked {
            self.vector_ctrl |= Self::MASK_BIT;
        } else {
            self.vector_ctrl &= !Self::MASK_BIT;
        }
    }
}

pub struct MsiXTableView {
    base: *mut MsixTableEntry,
}

impl MsiXTableView {
    fn new(base: *mut MsixTableEntry) -> Self {
        Self { base }
    }

    unsafe fn entry(&self, vector: u16) -> &mut MsixTableEntry {
        unsafe {
            &mut *self.base.add(vector as usize)
        }
    }

    unsafe fn mask(&self, vector: u16) {
        unsafe {
            let e = self.entry(vector);
            ptr::write_volatile(&mut e.vector_ctrl, e.vector_ctrl | 1);
        }
    }

    unsafe fn unmask(&self, vector: u16) {
        unsafe {
            let e = self.entry(vector);
            ptr::write_volatile(&mut e.vector_ctrl, e.vector_ctrl & !1);
        }
    }

    pub unsafe fn print(&self, count: usize) {
        for i in 0..count {
            let e = self.entry(i as u16);

            let addr = ((e.msg_addr_high as u64) << 32) | (e.msg_addr_low as u64);
            let masked = if e.is_masked() { "yes" } else { "no" };

            vgaprintln!(
                "MSI-X Entry {:>3}: addr=0x{:016x}, data=0x{:08x}, masked={}",
                i,
                addr,
                e.msg_data,
                masked
            );
        }
    }

}



//=======================================================
//      MSI-X PENDING BIT ARRAY
//=======================================================
struct MsixPBA {
    base: *const u32,
    vectors: usize
}

impl MsixPBA {
    fn new(pba_bar_base: *mut u8, pba_offset: u32, vectors: usize) -> Self {
        let base = unsafe {
            pba_bar_base.add(pba_offset as usize) as *const u32
        };

        MsixPBA { base, vectors }
    }

    unsafe fn is_pending(&self, vector: u16) -> bool {
        unsafe {
            let vector_index: usize = (vector as usize) >> 6;
            let bit = (vector as usize) % 64;

            let val = ptr::read_volatile(self.base.add(vector_index));
            (val >> bit) & 0x01 != 0
        }
    }
}

//=======================================================
//      MSI-X VECTOR
//=======================================================
struct MsiXVector {
    vector: u16,
    table: MsiXTableView,
    pba: MsixPBA
}

impl MsiXVector {
    fn new(vector: u16, table: MsiXTableView, pba: MsixPBA) -> MsiXVector {
        Self {
            vector,
            table,
            pba
        }
    }
}


pub struct XHCI<'a> {
    pci_device: &'a PciDeviceHeader,

    slots: u32,
    context_size: u32,
    dcbaa: &'a Dcbaa,
    command_ring: &'a TrbRing,
    msix_capability: &'a MsixCapability,
    msix_pba: &'a MsixPBA
}

impl<'a> XHCI<'a> {
    fn new(pci_device: &'a PciDeviceHeader,
           slots: u32,
           context_size: u32,
           dcbaa: &'a Dcbaa,
           command_ring: &'a TrbRing,
           msix_capability: &'a MsixCapability,
           msix_pba: &'a MsixPBA) -> Self {
        XHCI {
            pci_device,
            slots,
            context_size,
            dcbaa,
            command_ring,
            msix_capability,
            msix_pba
        }
    }


}


impl PciDeviceInitializer for XHCI<'_> {
    fn initialize(pci_device: &PciDeviceHeader, boot_info: &BootInfo, offset_page_table: &OffsetPageTable) -> Result<(), PciDeviceInitError> {
        let bar = PciBAR::get(pci_device, 0);

        if bar.bar_type() == &BarType::Io {
            return Err(InvalidBarType);
        }

        unsafe {
            let base = bar.base_address() + boot_info.physical_memory_offset;
            let operational_base = base + ptr::read_volatile((base + CAP_REG_CAPLENGTH as u64) as *const u8) as u64;
            let runtime_base = base + (ptr::read_volatile((base + CAP_REG_RTSOFF as u64) as *const u32) as u64) & !0x1F;


            let mut usbsts = ptr::read_volatile((operational_base + OP_REG_USBSTS as u64) as *const u32);
            let mut ticks = get_ticks();
            let max_ticks = (pic_get_ticks_per_ms() * 1000) + ticks;

            const CONTROLLER_READY: u32 = 0;
            while ticks < max_ticks {
                usbsts = ptr::read_volatile((operational_base + OP_REG_USBSTS as u64) as *const u32);
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
            let max_slots: u32 =  hccparams1 & 0xFF;
            {
                let config_reg = ptr::read_volatile((operational_base + OP_REG_CONFIG as u64) as *const u32);
                ptr::write_volatile((operational_base + OP_REG_CONFIG as u64) as *mut u32, config_reg | max_slots);
            }

            let context_size = if (hccparams1 & (1 << 2)) != 0 {
                64
            } else {
                32
            };

            //=========================================================================
            //init DCBAA
            //NOTE: idk if this is correct, lets leave it for now
            let mut dcbaa = Box::new(Dcbaa {
                entries: [0; 256]
            });

            //setting up dma address
            let virt_address = VirtAddr::from_ptr(Box::as_mut(&mut dcbaa));
            let dma_addr = match virtual_to_physical(virt_address, offset_page_table) {
                None => {
                    return Err(PciDeviceInitError::InitializationFailure);
                },
                Some(x) => x
            };

            //writing the address
            ptr::write_volatile((operational_base + OP_REG_DCBAAP as u64) as *mut u64, dma_addr.as_u64());
            //==================================================
            //COMMAND RING
            let crcr_addr = operational_base + OP_REG_CRCR as u64;
            let crcr = ptr::read_volatile(crcr_addr as *const u64);

            //command ring is runnning
            // if crcr & 0x03 == 1 {
            //
            // }
            let mut trb_arr = alloc_aligned_trb_array(256, 64);
            let command_ring = TrbRing::new(trb_arr, offset_page_table).expect("allocating memory for TRB ring failed!");


            let trb_virt = VirtAddr::new(command_ring.trbs.as_mut_ptr() as u64);
            let trb_dma = match virtual_to_physical(trb_virt, offset_page_table) {
                None => { return Err(PciDeviceInitError::InitializationFailure)},
                Some(x) => x.as_u64()
            };

            ptr::write_volatile(crcr_addr as *mut u64,
                                (trb_dma & !0b111111) | (crcr & 0b111111)
            );
            //========================================================================


            //MSI-X CONFIGURATION
            let status = pci_read16(pci_device.base_id(), 0x06); //read status register
            if status & (0x01 << 4) == 0 {
                return Err(NoMSIXCapabilities);
            }

            //read capabilities pointer
            let mut cap_ptr = pci_read8(pci_device.base_id(), 0x34);

            //search for MSI-X ptr
            while cap_ptr != 0 {
                let cap_id = pci_read8(pci_device.base_id(), cap_ptr as u32);

                if cap_id == 0x11 {
                    //we found MSI-X so exit
                    break;
                }

                cap_ptr = pci_read8(pci_device.base_id(), (cap_ptr + 1) as u32);
            }

            if cap_ptr == 0 {
                return Err(InitializationFailure);
            }

            let mut msix_capability = MsixCapability {
                cap_id: pci_read8(pci_device.base_id(), cap_ptr as u32),
                next: pci_read8(pci_device.base_id(), (cap_ptr + 1) as u32),
                message_control: pci_read16(pci_device.base_id(), (cap_ptr + 2) as u32),
                table: pci_read32(pci_device.base_id(), (cap_ptr + 4) as u32),
                pba: pci_read32(pci_device.base_id(), (cap_ptr + 8) as u32),
            };

            msix_capability.mask_all();
            pci_write16(pci_device.base_id(), (cap_ptr + 2) as u32, msix_capability.message_control);

            if msix_capability.table_size() < 2 {
                return Err(InitializationFailure);
            }

            let table_bar = PciBAR::from_bir(pci_device, msix_capability.table_bir()).
                expect("Table bar not found");

            let table_mmio = table_bar.mmio_addr(boot_info.physical_memory_offset, msix_capability.table_offset());


            let msix_table_ptr = table_mmio as *mut MsixTableEntry;
            let msix_table_view: MsiXTableView = MsiXTableView::new(msix_table_ptr);
            const LAPIC_MSI_ADDR: u64 = 0xFEE0_0000;

            unsafe {
                let mut entry0 = ptr::read_unaligned(msix_table_ptr);
                let mut entry1 = ptr::read_unaligned(msix_table_ptr.wrapping_add(1));

                entry0.msg_addr_low = LAPIC_MSI_ADDR as u32;
                entry0.msg_addr_high = (LAPIC_MSI_ADDR >> 32) as u32;
                entry0.msg_data = XHCIInterruptIndex::MsiXCommandPortData as u32;
                entry0.vector_ctrl = 0; //enabled

                entry1.msg_addr_low = LAPIC_MSI_ADDR as u32;
                entry1.msg_addr_high = (LAPIC_MSI_ADDR >> 32) as u32;
                entry1.msg_data = XHCIInterruptIndex::MsiXTransferEvents as u32;
                entry1.vector_ctrl = 0; //enabled

                ptr::write_unaligned(msix_table_ptr, entry0);
                ptr::write_unaligned(msix_table_ptr.wrapping_add(1), entry1);
            }

            let pba_bar = PciBAR::from_bir(&pci_device, msix_capability.pba_bir()).expect("no bar for bir");
            let msix_pba = MsixPBA::new(
                pba_bar.base_address() as *mut u8,
                msix_capability.pba_offset(),
                2
            );

            msix_capability.enable();
            pci_write16(pci_device.base_id(), (cap_ptr + 2) as u32, msix_capability.message_control);


            //Event Ring Segment Table Size Register (ERSTSZ)
            /*
            Event Ring Segment Table Size – RW. Default = ‘0’. This field identifies the number of valid
            Event Ring Segment Table entries in the Event Ring Segment Table pointed to by the Event Ring
            Segment Table Base Address register. The maximum value supported by an xHC implementation
            for this register is defined by the ERST Max field in the HCSPARAMS2 register (section 5.3.4).
            For Secondary Interrupters: Writing a value of ‘0’ to this field disables the Event Ring. Any events
            targeted at this Event Ring when it is disabled shall result in undefined behavior of the Event
            Ring.
            For the Primary Interrupter: Writing a value of ‘0’ to this field shall result in undefined behavior
            of the Event Ring. The Primary Event Ring cannot be disabled.
             */
            let erstsz_addr_base = runtime_base + RT_ERSTSZ as u64;

            let erstsz_primary = erstsz_addr_base + 0;
            let erstsz_secondary_first = erstsz_addr_base + (1 << 5);

            ptr::write_volatile(erstsz_primary as *mut u32, 0x01);
            ptr::write_volatile(erstsz_secondary_first as *mut u32, 0x01);

            /*
            The Event Ring Segment Table Base Address Register identifies the start address
            of the Event Ring Segment Table (ERST). Refer to section 6.5 for the definition of
            an ERST entry.
             */
            let erstba_base = runtime_base + RT_ERSTBA as u64;
            let erstba_primary = erstba_base;
            let erstba_secondary = erstba_base + (1 << 5);

            let erst_primary = alloc_aligned_erst();
            let erst_secondary = alloc_aligned_erst();
            let erst_p_addr = virtual_to_physical(
                VirtAddr::new((erst_primary as *mut ERST as u64) & !0x1F),
                offset_page_table
            );
            let erst_sec_addr = virtual_to_physical(
                VirtAddr::new((erst_secondary as *mut ERST as u64) & !0x1F),
                offset_page_table
            );

            // ptr::write_volatile(erstba_primary as *mut u64, erst_p_addr);
            // ptr::write_volatile(erstba_secondary as *mut u64, );



            let xhci_controller = XHCI::new(
                &pci_device,
                max_slots,
                context_size,
                &dcbaa,
                &command_ring,
                &msix_capability,
                &msix_pba
            );
        }

        Ok(())
    }
}

fn alloc_aligned_trb_array(len: usize, align: usize) -> &'static mut [Trb] {
    //allocate extra space - len TRBs + alignment padding
    let total_bytes = len * size_of::<Trb>() + align;
    let mut raw = Vec::<u8>::with_capacity(total_bytes);
    raw.resize(total_bytes, 0);

    let base_ptr = raw.as_mut_ptr() as usize;
    let aligned = (base_ptr + (align - 1)) & !(align - 1);

    let slice_ptr = aligned as *mut Trb;

    forget(raw);

    unsafe { core::slice::from_raw_parts_mut(slice_ptr, len) }
}

use core::mem::{size_of, forget};

pub fn alloc_aligned_erst() -> &'static mut ERST {
    let total_bytes = size_of::<ERST>() + 64;

    let mut raw = Vec::<u8>::with_capacity(total_bytes);
    raw.resize(total_bytes, 0);

    let base_ptr = raw.as_mut_ptr() as usize;
    let aligned = (base_ptr + (64 - 1)) & !(64 - 1);

    let ptr = aligned as *mut ERST;

    forget(raw);

    unsafe { &mut *ptr }
}


#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
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
    dword0: u32,         // 0x00
    dword1: u32,         // 0x04
    dword2: u32,         // 0x08 (TR Dequeue Ptr Lo + DCS)
    dword3: u32,         // 0x0C (TR Dequeue Ptr Hi)
    dword4: u32,         // 0x10
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
        self.dword0 = (self.dword0 & !Self::EP_STATE_MASK)
            | ((val << Self::EP_STATE_SHIFT) & Self::EP_STATE_MASK);
    }

    pub fn get_mult(&self) -> u32 {
        (self.dword0 & Self::MULT_MASK) >> Self::MULT_SHIFT
    }

    pub fn set_mult(&mut self, val: u32) {
        self.dword0 =
            (self.dword0 & !Self::MULT_MASK) | ((val << Self::MULT_SHIFT) & Self::MULT_MASK);
    }

    pub fn get_max_pstreams(&self) -> u32 {
        (self.dword0 & Self::MAX_PSTREAMS_MASK) >> Self::MAX_PSTREAMS_SHIFT
    }

    pub fn set_max_pstreams(&mut self, val: u32) {
        self.dword0 = (self.dword0 & !Self::MAX_PSTREAMS_MASK)
            | ((val << Self::MAX_PSTREAMS_SHIFT) & Self::MAX_PSTREAMS_MASK);
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
        self.dword0 = (self.dword0 & !Self::INTERVAL_MASK)
            | ((val << Self::INTERVAL_SHIFT) & Self::INTERVAL_MASK);
    }

    pub fn get_max_esit_payload_hi(&self) -> u32 {
        (self.dword0 & Self::MAX_ESIT_PAYLOAD_HI_MASK) >> Self::MAX_ESIT_PAYLOAD_HI_SHIFT
    }

    pub fn set_max_esit_payload_hi(&mut self, val: u32) {
        self.dword0 = (self.dword0 & !Self::MAX_ESIT_PAYLOAD_HI_MASK)
            | ((val << Self::MAX_ESIT_PAYLOAD_HI_SHIFT) & Self::MAX_ESIT_PAYLOAD_HI_MASK);
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
            (self.dword1 & !Self::CERR_MASK) | ((val << Self::CERR_SHIFT) & Self::CERR_MASK);
    }

    pub fn get_ep_type(&self) -> u32 {
        (self.dword1 & Self::EP_TYPE_MASK) >> Self::EP_TYPE_SHIFT
    }

    pub fn set_ep_type(&mut self, val: u32) {
        self.dword1 = (self.dword1 & !Self::EP_TYPE_MASK)
            | ((val << Self::EP_TYPE_SHIFT) & Self::EP_TYPE_MASK);
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
        self.dword1 = (self.dword1 & !Self::MAX_BURST_MASK)
            | ((val << Self::MAX_BURST_SHIFT) & Self::MAX_BURST_MASK);
    }

    pub fn get_max_packet_size(&self) -> u32 {
        (self.dword1 & Self::MAX_PACKET_SIZE_MASK) >> Self::MAX_PACKET_SIZE_SHIFT
    }

    pub fn set_max_packet_size(&mut self, val: u32) {
        self.dword1 = (self.dword1 & !Self::MAX_PACKET_SIZE_MASK)
            | ((val << Self::MAX_PACKET_SIZE_SHIFT) & Self::MAX_PACKET_SIZE_MASK);
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
        self.dword4 = (self.dword4 & !Self::AVG_TRB_LEN_MASK) | (val & Self::AVG_TRB_LEN_MASK);
    }

    pub fn get_max_esit_payload_lo(&self) -> u32 {
        (self.dword4 & Self::MAX_ESIT_PAYLOAD_LO_MASK) >> 16
    }

    pub fn set_max_esit_payload_lo(&mut self, val: u32) {
        self.dword4 = (self.dword4 & !Self::MAX_ESIT_PAYLOAD_LO_MASK)
            | ((val << 16) & Self::MAX_ESIT_PAYLOAD_LO_MASK);
    }
}

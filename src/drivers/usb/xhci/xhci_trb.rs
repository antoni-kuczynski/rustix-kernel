#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
use crate::drivers::pci::pci_device::PciDeviceInitError;
use crate::drivers::pci::{dma_alloc_zeroed, dma_as_slice_mut};
use crate::memory::dma::DmaAlloc;
use core::mem::size_of;
use x86_64::PhysAddr;

//=========================================
//  TRB
//=========================================
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Trb {
    pub parameter: u64, //pointer / value
    pub status: u32,    //length, residual...
    pub control: u32,   //type, cycle, flags
}

impl Trb {
    // ====== CONSTANTS ======
    const CYCLE_BIT: u32 = 1 << 0;
    const TOGGLE_CYCLE_BIT: u32 = 1 << 1;
    const CHAIN_BIT: u32 = 1 << 4;

    const TRB_TYPE_SHIFT: u32 = 10;
    const TRB_TYPE_MASK: u32 = 0x3F << Self::TRB_TYPE_SHIFT;

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

    pub fn cycle_val(&self) -> u32 {
        self.control & Self::CYCLE_BIT
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        if cycle {
            self.control |= Self::CYCLE_BIT;
        } else {
            self.control &= !Self::CYCLE_BIT;
        }
    }

    pub fn set_toggle_cycle(&mut self, toggle: bool) {
        if toggle {
            self.control |= Self::TOGGLE_CYCLE_BIT;
        } else {
            self.control &= !Self::TOGGLE_CYCLE_BIT;
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

    pub fn try_as_port_status_change_event(
        &self,
    ) -> Result<PortStatusChangeEventTrb, TrbParseError> {
        PortStatusChangeEventTrb::new(*self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrbParseError {
    UnexpectedType { expected: u8, actual: u8 },
}

pub trait EventTrb {
    const TRB_TYPE: u8;

    fn raw(&self) -> &Trb;

    /// Returns the raw TRB type from the control field.
    fn read_trb_type(&self) -> u8 {
        self.raw().trb_type()
    }

    /// Returns the Cycle bit from the event TRB control field.
    fn read_cycle(&self) -> bool {
        self.raw().cycle()
    }

    /// Returns the Completion Code from status bits 31:24.
    fn read_completion_code(&self) -> u8 {
        ((self.raw().status() >> 24) & 0xFF) as u8
    }

    /// Returns `true` when the wrapped raw TRB has the type expected by this event wrapper.
    fn has_expected_type(&self) -> bool {
        self.read_trb_type() == Self::TRB_TYPE
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PortStatusChangeEventTrb {
    raw: Trb,
}

impl PortStatusChangeEventTrb {
    const PORT_ID_SHIFT: u64 = 24;
    const PORT_ID_MASK: u64 = 0xFF << Self::PORT_ID_SHIFT;

    /// Creates a typed Port Status Change Event TRB from a raw TRB.
    ///
    /// This accepts only TRB type 34. The raw TRB is copied, so the wrapper does not borrow the
    /// event ring memory.
    pub fn new(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_PORT_STATUS_CHANGE_EVENT {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_PORT_STATUS_CHANGE_EVENT,
                actual,
            });
        }

        Ok(Self { raw })
    }

    /// Creates a typed Port Status Change Event TRB without checking the TRB type.
    ///
    /// Use this only after checking `Trb::trb_type()` or when the caller already knows the event
    /// ring entry is type 34.
    pub const unsafe fn new_unchecked(raw: Trb) -> Self {
        Self { raw }
    }

    /// Returns the wrapped raw TRB by value.
    pub const fn into_raw(self) -> Trb {
        self.raw
    }

    /// Returns the Port ID that generated the status change event.
    ///
    /// xHCI port IDs are one-based and correspond to the operational port register index plus one.
    pub fn read_port_id(&self) -> u8 {
        ((self.raw.parameter() & Self::PORT_ID_MASK) >> Self::PORT_ID_SHIFT) as u8
    }

    /// Returns `true` when this event references the given one-based xHCI Port ID.
    pub fn is_for_port(&self, port_id: u8) -> bool {
        self.read_port_id() == port_id
    }

    /// Returns the raw parameter field.
    ///
    /// For Port Status Change Event TRBs, the Port ID is encoded in bits 31:24.
    pub fn read_parameter(&self) -> u64 {
        self.raw.parameter()
    }

    /// Returns the raw status field.
    ///
    /// The common event TRB Completion Code is available through `read_completion_code`.
    pub fn read_status(&self) -> u32 {
        self.raw.status()
    }

    /// Returns the raw control field.
    ///
    /// The common event TRB type and Cycle bit are available through the `EventTrb` helpers.
    pub fn read_control(&self) -> u32 {
        self.raw.control()
    }
}

impl EventTrb for PortStatusChangeEventTrb {
    const TRB_TYPE: u8 = Trb::TRB_PORT_STATUS_CHANGE_EVENT;

    fn raw(&self) -> &Trb {
        &self.raw
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
pub struct TrbCreationError();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    Empty,
    Unsupported,
}

pub trait Ring {
    fn enqueue(&mut self, trb: Trb) -> Result<(), RingError>;
    fn dequeue(&mut self) -> Result<Trb, RingError>;
    fn head(&self) -> Result<usize, RingError>;
    fn tail(&self) -> Result<usize, RingError>;
    fn head_phys(&self) -> Result<PhysAddr, RingError>;
    fn tail_phys(&self) -> Result<PhysAddr, RingError>;
}

pub struct TrbRing {
    trbs: &'static mut [Trb],
    ring_phys: PhysAddr,
    enqueue_index: usize,
    cycle_state: bool,
}

impl TrbRing {
    pub fn new(trbs: &'static mut [Trb], ring_phys: PhysAddr) -> Result<TrbRing, TrbCreationError> {
        let last_index = trbs.len() - 1;

        //set the last TRB in ring to LINK type
        trbs[last_index].set_trb_type(Trb::TRB_LINK);
        trbs[last_index].set_cycle(true);
        trbs[last_index].set_toggle_cycle(true);
        trbs[last_index].set_parameter(ring_phys.as_u64());

        Ok(TrbRing {
            trbs,
            ring_phys,
            enqueue_index: 0,
            cycle_state: true,
        })
    }

    pub fn enqueue_phys(&self) -> PhysAddr {
        self.tail_phys().unwrap()
    }
}

impl Ring for TrbRing {
    fn enqueue(&mut self, mut trb: Trb) -> Result<(), RingError> {
        let link_index = self.trbs.len() - 1;
        if self.enqueue_index == link_index {
            self.trbs[link_index].set_cycle(self.cycle_state);
            self.enqueue_index = 0;
            self.cycle_state = !self.cycle_state;
        }

        trb.set_cycle(self.cycle_state);
        unsafe {
            core::ptr::write_volatile(&mut self.trbs[self.enqueue_index], trb);
        }
        self.enqueue_index += 1;

        Ok(())
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        Err(RingError::Unsupported)
    }

    fn head(&self) -> Result<usize, RingError> {
        Err(RingError::Unsupported)
    }

    fn tail(&self) -> Result<usize, RingError> {
        Ok(self.enqueue_index)
    }

    fn head_phys(&self) -> Result<PhysAddr, RingError> {
        Err(RingError::Unsupported)
    }

    fn tail_phys(&self) -> Result<PhysAddr, RingError> {
        Ok(self.ring_phys + (self.enqueue_index * size_of::<Trb>()) as u64)
    }
}

pub struct EventRing {
    trbs: &'static mut [Trb],
    ring_phys: PhysAddr,
    dequeue_index: usize,
    cycle_state: bool,
}

impl EventRing {
    pub const fn new(trbs: &'static mut [Trb], ring_phys: PhysAddr) -> Self {
        Self {
            trbs,
            ring_phys,
            dequeue_index: 0,
            cycle_state: true,
        }
    }

    pub fn dequeue_phys(&self) -> PhysAddr {
        self.head_phys().unwrap()
    }
}

impl Ring for EventRing {
    fn enqueue(&mut self, _: Trb) -> Result<(), RingError> {
        Err(RingError::Unsupported)
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        let trb = unsafe { core::ptr::read_volatile(&self.trbs[self.dequeue_index]) };
        if trb.cycle() != self.cycle_state {
            return Err(RingError::Empty);
        }

        self.dequeue_index += 1;
        if self.dequeue_index == self.trbs.len() {
            self.dequeue_index = 0;
            self.cycle_state = !self.cycle_state;
        }

        Ok(trb)
    }

    fn head(&self) -> Result<usize, RingError> {
        Ok(self.dequeue_index)
    }

    fn tail(&self) -> Result<usize, RingError> {
        Err(RingError::Unsupported)
    }

    fn head_phys(&self) -> Result<PhysAddr, RingError> {
        Ok(self.ring_phys + (self.dequeue_index * size_of::<Trb>()) as u64)
    }

    fn tail_phys(&self) -> Result<PhysAddr, RingError> {
        Err(RingError::Unsupported)
    }
}

pub fn alloc_dma_trb_ring(
    len: usize,
) -> Result<(DmaAlloc, &'static mut [Trb]), PciDeviceInitError> {
    let alloc = dma_alloc_zeroed(len * size_of::<Trb>(), 64)?;
    let trbs = unsafe { dma_as_slice_mut::<Trb>(&alloc, len) };
    Ok((alloc, trbs))
}

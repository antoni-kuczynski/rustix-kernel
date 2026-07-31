#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 07/07/2025
 */

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
use core::{mem, ptr};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{compiler_fence, fence, Ordering};
use x86_64::{PhysAddr, VirtAddr};
use crate::drivers::usb::xhci::xhci_trb::Trb;
use crate::drivers::usb::xhci::xhci_trb_ring::RingError::Unsupported;
use crate::kprintln;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};
use crate::memory::page_tables::PageSize;

#[derive(Debug)]
pub struct TrbCreationError();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    Empty,
    Unsupported,
    InvalidTrbOnEventRing,
    InvalidTrbOnCommandRing,
    InvalidTrbOnTransferRing,
    Full
}

pub trait Ring {
    fn new(trbs: *mut [Trb], ring_phys: PhysAddr) -> Self;

    /// Enqueues a TRB and returns the physical address of the slot it was written to.
    fn enqueue(&mut self, trb: Trb) -> Result<PhysAddr, RingError>;
    fn dequeue(&mut self) -> Result<Trb, RingError>;
    fn get_dequeue_phys(&self) -> Result<PhysAddr, RingError>;
    fn ring_mut(&mut self) -> &mut TrbRing;
    fn ring(&self) -> &TrbRing;

    fn trbs(&mut self) -> *mut [Trb] {
        self.ring_mut().trbs
    }

    fn enqueue_index(&self) -> usize {
        self.ring().enqueue_index
    }

    fn dequeue_index(&self) -> usize {
        self.ring().dequeue_index
    }

    fn cycle_state(&self) -> bool {
        self.ring().cycle_state
    }

    fn advance_dequeue_index(&mut self) {
        let ring = self.ring_mut();
        let len = ring.trbs.len();

        ring.dequeue_index += 1;
        if ring.dequeue_index == len {
            ring.dequeue_index = 0;
            ring.cycle_state = !ring.cycle_state;
        }
    }
}

pub struct TrbRing {
    trbs: *mut [Trb],
    ring_phys: PhysAddr,
    enqueue_index: usize,
    dequeue_index: usize,
    cycle_state: bool,
}

impl TrbRing {
    pub unsafe fn new(trbs: *mut [Trb], ring_phys: PhysAddr) -> Result<TrbRing, TrbCreationError> {
        let len = trbs.len();
        let last_index = len - 1;

        //set the last TRB in ring to LINK type
        (*trbs)[last_index].set_trb_type(Trb::TRB_LINK);
        (*trbs)[last_index].set_cycle(true);
        (*trbs)[last_index].set_toggle_cycle(true);
        (*trbs)[last_index].set_parameter(ring_phys.as_u64());

        Ok(TrbRing {
            trbs,
            ring_phys,
            enqueue_index: 0,
            dequeue_index: 0,
            cycle_state: true,
        })
    }

    pub fn dma_alloc(len_in_trbs: usize) -> Option<(DmaAlloc, *mut [Trb])> {
        let alloc = dma_alloc_zeroed(len_in_trbs * size_of::<Trb>(), PageSize::SIZE_4KB as usize)?;
        let trbs = unsafe { alloc.as_slice_mut::<Trb>(len_in_trbs) };
        Some((alloc, trbs))
    }

    /// Enqueues trb on a ring.
    ///
    /// `dequeue_index_check` enables checking for ring being full via
    /// dequeue_index == next_enqueue_index, should be false on event ring and true on command ring.
    pub unsafe fn enqueue(&mut self, mut trb: Trb, dequeue_index_check: bool) -> Result<PhysAddr, RingError> {
        let len = self.trbs.len();
        let link_index = len - 1;

        let mut next_index = self.enqueue_index + 1;
        if next_index == link_index {
            next_index = 0;
        }

        if dequeue_index_check && next_index == self.dequeue_index {
            kprintln!(Error, "Ring full, dequeue index={}, enqueue index={}", self.dequeue_index, self.enqueue_index);
            return Err(RingError::Full);
        }

        let written_index = self.enqueue_index;
        let is_last_slot = written_index == link_index - 1;


        trb.set_cycle(!self.cycle_state);
        write_volatile(&mut (*self.trbs)[written_index], trb);

        if is_last_slot {
            let mut link_trb = read_volatile(&(*self.trbs)[link_index]);
            link_trb.set_cycle(self.cycle_state);
            write_volatile(&mut (*self.trbs)[link_index], link_trb);
        }

        //memory barrier
        fence(Ordering::Release);

        let correct_control_dword = trb.control() ^ 1;
        let trb_ptr = &mut (*self.trbs)[written_index] as *mut Trb as *mut u32;
        let dword3_ptr = trb_ptr.add(3);
        write_volatile(dword3_ptr, correct_control_dword);


        if is_last_slot {
            self.enqueue_index = 0;
            self.cycle_state = !self.cycle_state;
        } else {
            self.enqueue_index += 1;
        }

        Ok(self.trb_phys(written_index))
    }

    pub fn trb_phys(&self, index: usize) -> PhysAddr {
        self.ring_phys + (index * size_of::<Trb>()) as u64
    }


    pub fn dequeue(&mut self) -> Result<Trb, RingError> {
        let trb_ptr = unsafe { &(*self.trbs)[self.dequeue_index] };
        let control_ptr = unsafe {
            ptr::addr_of!((*self.trbs)[self.dequeue_index].control)
        };
        let control_dword = unsafe { read_volatile(control_ptr) };

        let cycle_bit = (control_dword & 1) != 0;

        if cycle_bit != self.cycle_state {
            return Err(RingError::Empty);
        }

        compiler_fence(Ordering::Acquire);

        let trb = unsafe { read_volatile(trb_ptr) };

        // kprintln!("Dequeue index: {}, Cycle bit: {}, Trb type: {}",self.dequeue_index, trb.cycle(), trb.trb_type());
        self.dequeue_index += 1;

        if self.dequeue_index == self.trbs.len() {
            self.dequeue_index = 0;
            self.cycle_state = !self.cycle_state;
        }

        Ok(trb)
    }

    pub fn head_phys(&self) -> PhysAddr {
        self.ring_phys + (self.dequeue_index * size_of::<Trb>()) as u64
    }

    pub fn tail_phys(&self) -> PhysAddr {
        self.ring_phys + (self.enqueue_index * size_of::<Trb>()) as u64
    }

    pub fn get_enqueue_phys(&self) -> PhysAddr {
        self.tail_phys()
    }

    pub fn get_dequeue_phys(&self) -> PhysAddr {
        self.head_phys()
    }
}

//==================================================================================================
// EVENT RING
//==================================================================================================
pub struct EventRing {
    ring: TrbRing,
    erdp_offset: Option<(VirtAddr, u64)>,
}

impl Ring for EventRing {
    fn new(trbs: *mut [Trb], ring_phys: PhysAddr) -> Self {
        let ring = TrbRing {
            trbs,
            ring_phys,
            enqueue_index: 0,
            dequeue_index: 0,
            cycle_state: true,
        };

        Self {
            ring,
            erdp_offset: None
        }
    }

    fn enqueue(&mut self, _: Trb) -> Result<PhysAddr, RingError> {
        Err(Unsupported)
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        let a = self.ring.dequeue()?;
        if a.is_event_trb() {
            Ok(a)
        } else {
            Err(RingError::InvalidTrbOnEventRing)
        }
    }

    fn get_dequeue_phys(&self) -> Result<PhysAddr, RingError> {
        Ok(self.ring.get_dequeue_phys())
    }

    fn ring_mut(&mut self) -> &mut TrbRing {
        &mut self.ring
    }

    fn ring(&self) -> &TrbRing {
        &self.ring
    }
}

//==================================================================================================
//  COMMAND RING
//==================================================================================================
pub struct CommandRing {
    ring: TrbRing
}

impl Ring for CommandRing {
    fn new(trbs: *mut [Trb], ring_phys: PhysAddr) -> Self {
        let ring = unsafe { TrbRing::new(trbs, ring_phys) }
            .expect("Ring creation for command ring failed.");
        Self { ring }
    }

    fn enqueue(&mut self, trb: Trb) -> Result<PhysAddr, RingError> {
        // kprintln!(Debug, "Command ring enqueue index {}.", self.enqueue_index());
        if trb.is_command_trb() {
            unsafe { self.ring.enqueue(trb, true) }
        } else {
            Err(RingError::InvalidTrbOnCommandRing)
        }
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        Err(Unsupported)
    }

    fn get_dequeue_phys(&self) -> Result<PhysAddr, RingError> {
        Err(Unsupported)
    }

    fn ring_mut(&mut self) -> &mut TrbRing {
        &mut self.ring
    }

    fn ring(&self) -> &TrbRing {
        &self.ring
    }
}
//==================================================================================================
//  TRANSFER RING
//==================================================================================================
pub struct TransferRing {
    ring: TrbRing
}

impl Ring for TransferRing {
    fn new(trbs: *mut [Trb], ring_phys: PhysAddr) -> Self {
        let ring = unsafe { TrbRing::new(trbs, ring_phys) }
            .expect("Ring creation for transfer ring failed.");
        Self { ring }
    }

    fn enqueue(&mut self, trb: Trb) -> Result<PhysAddr, RingError> {
        if trb.is_transfer_trb() {
            unsafe { self.ring.enqueue(trb, false) }
        } else {
            Err(RingError::InvalidTrbOnTransferRing)
        }
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        self.ring.dequeue()
    }

    fn get_dequeue_phys(&self) -> Result<PhysAddr, RingError> {
        Err(Unsupported)
    }

    fn ring_mut(&mut self) -> &mut TrbRing {
        &mut self.ring
    }

    fn ring(&self) -> &TrbRing {
        &self.ring
    }
}


pub struct ShadowRing<const SIZE: usize> {
    entries: [CommandContext; SIZE],
    command_ring_phys_base: u64,
}

impl<const SIZE: usize> ShadowRing<SIZE> {
    pub fn new(command_ring_phys_base: PhysAddr) -> Self {
        Self {
            entries: [CommandContext::Empty; SIZE],
            command_ring_phys_base: command_ring_phys_base.as_u64(),
        }
    }

    /// Records the context of a command by the physical address of its TRB.
    pub fn save_context_by_phys_addr(&mut self, phys_addr: PhysAddr, context: CommandContext) {
        match self.calculate_index(phys_addr.as_u64()) {
            Some(index) => self.entries[index] = context,
            //unreachable for addresses returned by our own command ring
            None => debug_assert!(false, "Command TRB address outside of the command ring"),
        }
    }

    pub fn take_context_by_phys_addr(&mut self, phys_addr: PhysAddr) -> CommandContext {
        match self.calculate_index(phys_addr.as_u64()) {
            Some(index) => mem::replace(&mut self.entries[index], CommandContext::Empty),
            None => CommandContext::Empty,
        }
    }

    fn calculate_index(&self, phys_addr: u64) -> Option<usize> {
        if phys_addr < self.command_ring_phys_base {
            return None;
        }

        let offset_bytes = phys_addr - self.command_ring_phys_base;
        if offset_bytes % size_of::<Trb>() as u64 != 0 {
            return None;
        }

        let index = (offset_bytes / size_of::<Trb>() as u64) as usize;
        if index >= SIZE {
            return None;
        }

        Some(index)
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
pub struct ERST {
    ring_addr_low: u32,
    ring_addr_high: u32,
    ring_segment_size: u32,
    rsvdz: u32,
}

impl ERST {
    pub(crate) fn new(ring_addr: u64, ring_segment_size: u32) -> ERST {
        assert_eq!(ring_addr & 0x1F, 0, "Event ring must be 32-byte aligned");

        let ring_addr_high: u32 = (ring_addr >> 32) as u32;
        let ring_addr_low: u32 = (ring_addr & 0xFFFFFFE0) as u32;

        Self {
            ring_addr_low,
            ring_addr_high,
            ring_segment_size,
            rsvdz: 0u32,
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

/// Allocates an Event Ring Segment Table describing a single event ring segment.
pub fn xhci_alloc_dma_erst(
    event_ring_phys: PhysAddr,
    ring_segment_size: u32,
) -> Option<DmaAlloc> {
    let alloc = dma_alloc_zeroed(size_of::<ERST>(), PageSize::SIZE_4KB as usize)?;
    unsafe {
        write_volatile(
            alloc.virt.as_mut_ptr::<ERST>(),
            ERST::new(event_ring_phys.as_u64(), ring_segment_size),
        );
    }
    Some(alloc)
}

// Not quite a ring, but a helper for command ring saving command context for future access
#[derive(Clone, Copy, Debug)]
pub enum CommandContext {
    Empty,
    EnableSlot { port_id: u8 },
    DisableSlot { port_id: u8, slot_id: u8 },
    AddressDevice { slot_id: u8 },
}

impl Default for CommandContext {
    fn default() -> Self {
        Self::Empty
    }
}

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
use x86_64::PhysAddr;
use crate::drivers::pci::pci_device::PciDeviceInitError;
use crate::drivers::usb::xhci::xhci_trb::Trb;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};

#[derive(Debug)]
pub struct TrbCreationError();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    Empty,
    Unsupported,
}

pub trait Ring<'a> {
    fn new(trbs: &'a mut [Trb], ring_phys: PhysAddr) -> Self;
    fn enqueue(&mut self, trb: Trb) -> Result<(), RingError>;
    fn dequeue(&mut self) -> Result<Trb, RingError>;
    fn get_enqueue_phys(&mut self, trb: Trb) -> Result<PhysAddr, RingError>;
    fn get_dequeue_phys(&mut self) -> Result<PhysAddr, RingError>;
    fn ring_mut(&mut self) -> &'a mut TrbRing;
    fn ring(&self) -> &'a TrbRing;

    fn trbs(&'a mut self) -> &'a mut [Trb] {
        let ring: &mut TrbRing = self.ring_mut();
        ring.trbs
    }

    fn enqueue_index(&'a self) -> usize {
        self.ring().enqueue_index
    }

    fn dequeue_index(&'a self) -> usize {
        self.ring().dequeue_index
    }

    fn cycle_state(&'a self) -> bool {
        self.ring().cycle_state
    }

    fn advance_dequeue_index(&'a mut self) {
        let ring = self.ring_mut();
        ring.dequeue_index += 1;
        if ring.dequeue_index == ring.trbs.len() {
            ring.dequeue_index = 0;
            ring.cycle_state = !ring.cycle_state;
        }
    }
}

pub struct TrbRing<'a> {
    trbs: &'a mut [Trb],
    ring_phys: PhysAddr,
    enqueue_index: usize,
    dequeue_index: usize,
    cycle_state: bool,
}

impl<'a> TrbRing<'a> {
    pub fn new(trbs: &'a mut [Trb], ring_phys: PhysAddr) -> Result<TrbRing, TrbCreationError> {
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
            dequeue_index: 0, //TODO: this is probably invalid value for that
            cycle_state: true,
        })
    }

    pub fn dma_alloc(len: usize) -> Option<(DmaAlloc, &'a mut [Trb])> {
        let alloc = dma_alloc_zeroed(len * size_of::<Trb>(), 64)?;
        let trbs = unsafe { alloc.as_slice_mut::<Trb>(len) };
        Some((alloc, trbs))
    }

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
        let trb = unsafe { core::ptr::read_volatile(&self.trbs[self.dequeue_index]) };
        if trb.cycle() != self.cycle_state {
            return Err(RingError::Empty);
        }

        self.dequeue_index += 1;
        if self.dequeue_index == self.trbs.len() {
            self.dequeue_index = 0;
            self.cycle_state = !self.cycle_state;
        }        Ok(trb)
    }

    fn head_phys(&'a self) -> PhysAddr {
        let ring_phys = self.ring_phys;
        let dequeue_index = self.dequeue_index;

        ring_phys + (dequeue_index * size_of::<Trb>()) as u64
    }

    fn tail_phys(&'a self) -> PhysAddr {
        let ring_phys = self.ring_phys;
        let enqueue_index = self.enqueue_index;

        ring_phys + (enqueue_index * size_of::<Trb>()) as u64
    }

    fn get_enqueue_phys(&self) -> PhysAddr {
        self.tail_phys()
    }

    fn get_dequeue_phys(&self) -> PhysAddr {
        self.head_phys()
    }
}
//==================================================================================================
// EVENT RING
//==================================================================================================
pub struct EventRing<'a> {
    ring: TrbRing<'a>
}

impl<'a> Ring<'a> for EventRing<'a> {
    fn new(trbs: &'a mut [Trb], ring_phys: PhysAddr) -> Self {
        let ring = TrbRing::new(trbs, ring_phys).
            expect("Ring creation for event ring failed.");
        Self {
            ring
        }
    }

    fn enqueue(&mut self, _: Trb) -> Result<(), RingError> {
        Err(RingError::Unsupported)
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        self.ring.dequeue()
    }

    fn get_enqueue_phys(&mut self, trb: Trb) -> Result<PhysAddr, RingError> {
        Err(RingError::Unsupported)
    }

    fn get_dequeue_phys(&mut self) -> Result<PhysAddr, RingError> {
        Ok(self.ring.get_dequeue_phys())
    }


    fn ring_mut(&mut self) -> &'a mut TrbRing {
        &mut self.ring
    }

    fn ring(&self) -> &'_ TrbRing {
        &self.ring
    }
}
//==================================================================================================
//  COMMAND RING
//==================================================================================================
pub struct CommandRing<'a> {
    ring: TrbRing<'a>
}

impl<'a> Ring<'a> for CommandRing<'a> {
    fn new(trbs: &'a mut [Trb], ring_phys: PhysAddr) -> Self {
        let ring = TrbRing::new(trbs, ring_phys).
            expect("Ring creation for event ring failed.");
        Self {
            ring
        }
    }

    fn enqueue(&mut self, trb: Trb) -> Result<(), RingError> {
        self.ring.enqueue(trb)
    }

    fn dequeue(&mut self) -> Result<Trb, RingError> {
        self.ring.dequeue()
    }

    fn get_enqueue_phys(&mut self, trb: Trb) -> Result<PhysAddr, RingError> {
        Ok(self.ring.get_enqueue_phys())
    }

    fn get_dequeue_phys(&mut self) -> Result<PhysAddr, RingError> {
        Err(RingError::Unsupported)
    }

    fn ring_mut(&mut self) -> &'a mut TrbRing {
        &mut self.ring
    }

    fn ring(&self) -> &'a TrbRing {
        &self.ring
    }
}


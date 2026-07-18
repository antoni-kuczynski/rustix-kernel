/*
 * Created by Antoni Kuczyński
 * 17/07/2026
 */
use x86_64::{PhysAddr, VirtAddr};
use crate::drivers::usb::xhci::xhci_endpoint_context::EndpointContext;
use crate::drivers::usb::xhci::xhci_slot_context::SlotContext;
use crate::memory::dir_mapping::physical_to_virtual;

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
//===================================================================
//              DEVICE CONTEXT
//===================================================================
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct DeviceContext {
    data: [u8; 2048],
}

impl DeviceContext {
    pub fn new() -> Self {
        Self { data: [0; 2048] }
    }

    pub fn slot(&self) -> &SlotContext {
        unsafe { &*(self.data.as_ptr() as *const SlotContext) }
    }

    pub fn slot_mut(&mut self) -> &mut SlotContext {
        unsafe { &mut *(self.data.as_mut_ptr() as *mut SlotContext) }
    }

    pub fn endpoint(&self, dci: usize, context_size: u32) -> &EndpointContext {
        assert!(dci > 0 && dci <= 31, "DCI dla Endpointu musi być w przedziale 1..=31");

        unsafe { &*(self.data.as_ptr().add(dci * context_size as usize) as *const EndpointContext) }
    }

    pub fn endpoint_mut(&mut self, dci: usize, csz: bool) -> &mut EndpointContext {
        assert!(dci > 0 && dci <= 31, "DCI dla Endpointu musi być w przedziale 1..=31");

        let step = if csz { 64 } else { 32 };
        unsafe { &mut *(self.data.as_mut_ptr().add(dci * step) as *mut EndpointContext) }
    }
}
//===================================================================
//              Device Context Base Address Array
//===================================================================
#[repr(C, align(64))]
pub struct Dcbaa {
    entries: [u64; 256], //max 256 entries, < 2kb
}

impl Dcbaa {
    pub fn get_context(&self, slot_id: usize) -> PhysAddr {
        PhysAddr::new(self.entries[slot_id])
    }

    pub fn get_context_virt(&self, slot_id: usize) -> VirtAddr {
        physical_to_virtual(PhysAddr::new(self.entries[slot_id]))
    }

    pub fn set_context(&mut self, slot_id: usize, addr: u64) {
        self.entries[slot_id] = addr;
    }

    pub fn clear_context(&mut self, slot_id: usize) {
        self.entries[slot_id] = 0;
    }
}

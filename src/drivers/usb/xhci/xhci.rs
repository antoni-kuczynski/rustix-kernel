#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
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

//! # Locking
//!
//! The controller is shared as a plain `Arc<XHCI>`; every piece of mutable state inside it
//! sits behind its own [`IrqMutex`]. There is deliberately no lock covering the controller as
//! a whole, so an event ring interrupt never waits for unrelated work such as a transfer
//! submission, and the two interrupters never wait for each other.
//!
//! Locks are always taken in the order below, and one is never held across a call that takes
//! an earlier one:
//!
//! 1. [`XHCI_CONTROLLERS`] (released before the controller itself is used)
//! 2. [`XHCI::ports`]
//! 3. [`XHCI::devices`]
//! 4. a single [`XhciDevice`]
//! 5. [`XHCI::dcbaa`]
//! 6. [`XHCI::commands`]
//!
//! `XhciInterrupter::event_ring` is a leaf held only while draining TRBs into a local batch,
//! never while dispatching them. A `UsbTransferRequest` lock is likewise only held for short
//! field updates and never together with any of the above, so a completion callback cannot
//! deadlock against a submission in progress.

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::arch::x86_64::_mm_mfence;
use core::mem::size_of;
use core::ops::Add;
use core::ptr;
use core::sync::atomic::{compiler_fence, AtomicUsize, Ordering};
use spin::once::Once;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::{PhysAddr, VirtAddr};

use crate::drivers::apic::apic::{set_timeout, timer_lapic_uptime_ms, LAPIC};
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::PciDeviceInitError::{
    InsufficientMsixVectors, InvalidBarType, XhciControllerNotReadyTimeout,
    XhciControllerResetTimeout, XhciControllerStartTimeout, XhciControllerStopTimeout,
    XhciMsiCapabilityNotFound,
};
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_io::PciVendor;
use crate::drivers::pci::pci_msi::{MsiCapability, MsixCapability, MsixPBA};
use crate::drivers::pci::*;
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_core::usb_register_device;
use crate::drivers::usb::core::usb_transfers::{
    UsbHostController, UsbTransferRequest, UsbTransferStatus,
};
use crate::drivers::usb::xhci::xhci_context::{Dcbaa, DeviceContext};
use crate::drivers::usb::xhci::xhci_endpoint_context::*;
use crate::drivers::usb::xhci::xhci_ext_cap::XhciPortProtocol::Usb2;
use crate::drivers::usb::xhci::xhci_ext_cap::{parse_xhci_supported_protocols, XhciPortInfo, XhciPortProtocol, XhciPortSpeed};
use crate::drivers::usb::xhci::xhci_input_context::InputContext;
use crate::drivers::usb::xhci::xhci_portsc::PortLinkState::{U0, U3};
use crate::drivers::usb::xhci::xhci_portsc::{PortLinkState, PortStatusControl};
use crate::drivers::usb::xhci::xhci_slot_context::*;
use crate::drivers::usb::xhci::xhci_trb::*;
use crate::drivers::usb::xhci::xhci_trb_ring::{
    xhci_alloc_dma_erst, CommandContext, CommandRing, EventRing, Ring, RingError, ShadowRing,
    TransferRing, TrbRing,
};
use crate::drivers::usb::xhci::*;
use crate::drivers::usb::{UsbBmRequestType, UsbBRequest, WValue};
use crate::drivers::usb::xhci::xhci::PortState::{Configured, PendingSetConfiguration};
use crate::interrupts::router::register_handler_with_context;
use crate::interrupts::vector::InterruptVector;
use crate::kprintln;
use crate::memory::dir_mapping::physical_to_virtual;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};
use crate::memory::page_tables::PageSize;

const PCI_STATUS_REGISTER: u32 = 0x06;
const PCI_STATUS_CAPABILITIES_LIST: u16 = 1 << 4;
const PCI_CAPABILITY_POINTER_REGISTER: u32 = 0x34;
const PCI_CAPABILITY_ID_MSIX: u8 = 0x11;
const PCI_CAPABILITY_ID_MSI: u8 = 0x05;
const PCI_CAPABILITY_NEXT_POINTER_OFFSET: u32 = 0x01;
const XHCI_INIT_TIMEOUT_MS: u64 = 1000;
const RUNTIME_BASE_ALIGNMENT_MASK: u64 = !0x1f;
const MAX_SLOTS_MASK: u32 = 0xFF;
const MAX_PORTS_SHIFT: u32 = 24;
const MAX_PORTS_MASK: u32 = 0xFF << MAX_PORTS_SHIFT;
const CONTEXT_SIZE_64_BYTE_FLAG: u32 = 1 << 2;
const CONTEXT_SIZE_64_BYTES: u32 = 64;
const CONTEXT_SIZE_32_BYTES: u32 = 32;
const COMMAND_RING_RESERVED_BITS: u64 = 0b111111;
const COMMAND_RING_CYCLE_STATE: u64 = 1;
const PRIMARY_INTERRUPTER: u8 = 0;
const TRANSFER_INTERRUPTER: u8 = 1;
const SINGLE_ERST_SEGMENT: u32 = 1;
const ERDP_RESERVED_BITS: u64 = 0xf;
const ERDP_PTR_MASK: u64 = !0xF;
const ERDP_EHB: u64 = 1 << 3;
const XHCI_EXT_CAP_ID_MASK: u32 = 0xFF;
const XHCI_EXT_CAP_NEXT_MASK: u32 = 0xFF00;
const XHCI_EXT_CAP_NEXT_SHIFT: u32 = 8;
const XHCI_EXT_CAP_LEGACY_SUPPORT: u8 = 0x01;
const XHCI_LEGACY_BIOS_OWNED: u32 = 1 << 16;
const XHCI_LEGACY_OS_OWNED: u32 = 1 << 24;
const XHCI_LEGACY_CTLSTS_OFFSET: u64 = 0x04;
const XHCI_LEGACY_CTLSTS_CLEAR: u32 = 0xE000_0000;
const XHCI_LEGACY_HANDOFF_TIMEOUT_MS: u64 = 100;

const MAX_PORT_ENTRIES: usize = 256;
const EVENT_BATCH_TRBS: usize = 32;
const EP0_DCI: u8 = 1;
const MAX_DCI: u8 = 31;

fn runtime_interrupter_offset(interrupter: u8) -> u64 {
    interrupter as u64 * INTERRUPTER_REGISTER_STRIDE
}

fn first_ext_cap_addr(base: VirtAddr, hccparams1: u32) -> Option<VirtAddr> {
    let ext_cap_offset = (hccparams1 & HCCPARAMS1_XECP_MASK) >> HCCPARAMS1_XECP_SHIFT;
    if ext_cap_offset == 0 {
        None
    } else {
        Some(base.add((ext_cap_offset << 2) as u64))
    }
}

unsafe fn wait_until(
    mut condition: impl FnMut() -> bool,
    timeout_ms: u64,
    timeout_error: PciDeviceInitError,
) -> Result<(), PciDeviceInitError> {
    let start_ms = timer_lapic_uptime_ms();
    loop {
        if condition() {
            return Ok(());
        }
        if timer_lapic_uptime_ms().wrapping_sub(start_ms) >= timeout_ms {
            return Err(timeout_error);
        }
    }
}

unsafe fn xhci_stop_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
    let usbcmd = mmio_read::<u32>(operational_base, OP_REG_USBCMD as u64);
    mmio_write::<u32>(
        operational_base,
        OP_REG_USBCMD as u64,
        usbcmd & !USB_CMD_RUN_STOP,
    );

    wait_until(
        || mmio_read::<u32>(operational_base, OP_REG_USBSTS as u64) & XHCI_STATUS_HALTED != 0,
        XHCI_INIT_TIMEOUT_MS,
        XhciControllerStopTimeout,
    )
}

unsafe fn xhci_reset_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
    let usbcmd = mmio_read::<u32>(operational_base, OP_REG_USBCMD as u64);
    mmio_write::<u32>(
        operational_base,
        OP_REG_USBCMD as u64,
        usbcmd | USB_CMD_HOST_CONTROLLER_RESET,
    );

    wait_until(
        || {
            mmio_read::<u32>(operational_base, OP_REG_USBCMD as u64) & USB_CMD_HOST_CONTROLLER_RESET
                == 0
        },
        XHCI_INIT_TIMEOUT_MS,
        XhciControllerResetTimeout,
    )?;

    wait_until(
        || {
            mmio_read::<u32>(operational_base, OP_REG_USBSTS as u64) & XHCI_CONTROLLER_NOT_READY
                == 0
        },
        XHCI_INIT_TIMEOUT_MS,
        XhciControllerNotReadyTimeout,
    )
}

unsafe fn xhci_start_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
    let usbcmd = mmio_read::<u32>(operational_base, OP_REG_USBCMD as u64);
    mmio_write::<u32>(
        operational_base,
        OP_REG_USBCMD as u64,
        usbcmd | USB_CMD_INTERRUPTER_ENABLE | USB_CMD_RUN_STOP,
    );

    wait_until(
        || mmio_read::<u32>(operational_base, OP_REG_USBSTS as u64) & XHCI_STATUS_HALTED == 0,
        XHCI_INIT_TIMEOUT_MS,
        XhciControllerStartTimeout,
    )
}

unsafe fn xhci_legacy_handoff(first_ext_cap_addr: Option<VirtAddr>) {
    let Some(mut cap_addr) = first_ext_cap_addr else {
        return;
    };

    for _ in 0..256 {
        let header = mmio_read::<u32>(cap_addr, 0);
        let cap_id = (header & XHCI_EXT_CAP_ID_MASK) as u8;
        let next = ((header & XHCI_EXT_CAP_NEXT_MASK) >> XHCI_EXT_CAP_NEXT_SHIFT) as u8;

        if cap_id == XHCI_EXT_CAP_LEGACY_SUPPORT {
            if header & XHCI_LEGACY_BIOS_OWNED != 0 {
                mmio_write::<u32>(cap_addr, 0, header | XHCI_LEGACY_OS_OWNED);

                let start_ms = timer_lapic_uptime_ms();
                loop {
                    let current = mmio_read::<u32>(cap_addr, 0);
                    if current & XHCI_LEGACY_BIOS_OWNED == 0 {
                        break;
                    }

                    if timer_lapic_uptime_ms().wrapping_sub(start_ms)
                        >= XHCI_LEGACY_HANDOFF_TIMEOUT_MS
                    {
                        kprintln!(Info, "xHCI legacy handoff timeout: USBLEGSUP={:#010x}", current);
                        break;
                    }
                }
            }

            mmio_write::<u32>(
                cap_addr,
                XHCI_LEGACY_CTLSTS_OFFSET,
                XHCI_LEGACY_CTLSTS_CLEAR,
            );
            return;
        }

        if next == 0 {
            return;
        }

        cap_addr += (next as u64) * 4;
    }

    kprintln!(Warn,"xHCI legacy handoff: extended capability chain too long");
}

fn xhci_enable_usb3_port_power(operational_base: VirtAddr, supported_protocols: &[XhciPortInfo]) {
    let mut powered_ports = 0usize;

    for port_info in supported_protocols {
        if port_info.protocol != XhciPortProtocol::Usb3 || port_info.port_id == 0 {
            continue;
        }

        let portsc = PortStatusControl::from_port(operational_base, port_info.port_id);
        if portsc.pp_read() {
            continue;
        }

        let mut write = PortStatusControl::write_from_raw(portsc.raw());
        write.pp_write(true);
        write.write_to_port(operational_base, port_info.port_id);
        powered_ports += 1;
    }

    if powered_ports != 0 {
        kprintln!(Info,"xHCI powered {} USB3 root hub ports", powered_ports);
    }
}

fn find_pci_capability(dev: &PciDevice, capability_id: u8) -> Option<u8> {
    let status = dev.pci_read16(PCI_STATUS_REGISTER);
    if status & PCI_STATUS_CAPABILITIES_LIST == 0 {
        return None;
    }

    let mut cap_ptr = dev.pci_read8(PCI_CAPABILITY_POINTER_REGISTER);
    while cap_ptr != 0 {
        let cap_id = dev.pci_read8(cap_ptr as u32);
        if cap_id == capability_id {
            return Some(cap_ptr);
        }

        cap_ptr = dev.pci_read8(cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET);
    }

    None
}

fn xhci_configure_interrupts(dev: &PciDevice) -> Result<XhciInterruptConfig, PciDeviceInitError> {
    if let Some(msix_cap_ptr) = find_pci_capability(dev, PCI_CAPABILITY_ID_MSIX) {
        //msix capability found, so configure msix
        let config = dev.configure_msix(msix_cap_ptr, 2).expect("XHCI msi-x configuration failed");

        return Ok(XhciInterruptConfig::Msix {
            capability: config.capability,
            pba: config.pba,
            command_vector: config.vectors[0],
            transfer_vector: config.vectors[1],
        });
    }

    //msi-x was not found so use msi instead
    let msi_cap_ptr = find_pci_capability(dev, PCI_CAPABILITY_ID_MSI).ok_or(XhciMsiCapabilityNotFound)?;
    let msi_config = dev.configure_msi(msi_cap_ptr, 1).expect("XHCI msi configuration failed");
    Ok(
        XhciInterruptConfig::Msi {
            capability: msi_config.capability,
            vector: msi_config.vectors[0],
        }
    )
}


unsafe fn xhci_alloc_scratchpad(cap_base: VirtAddr, dcbaa: &mut DcbaaOwner) -> ScratchpadArea {
    let hcsparams2 = mmio_read::<u32>(cap_base, 0x08);
    let hi = (hcsparams2 >> 21) & 0x1F;
    let lo = (hcsparams2 >> 27) & 0x1F;
    let count = ((hi << 5) | lo) as usize;
    kprintln!(Debug, "Scratchpad buffers required: {}", count);

    if count == 0 {
        return ScratchpadArea {
            _array: None,
            _buffers: Vec::new(),
        };
    }

    let array = dma_alloc_zeroed(count * 8, 4096).expect("scratchpad array");
    let entries = array.virt.as_mut_ptr::<u64>();
    let mut buffers = Vec::with_capacity(count);

    for index in 0..count {
        let buffer = dma_alloc_zeroed(4096, 4096).expect("scratchpad buffer");
        ptr::write_volatile(entries.add(index), buffer.phys.as_u64());
        buffers.push(buffer);
    }

    dcbaa.set_context(0, array.phys.as_u64()); // DCBAA[0] = scratchpad array

    ScratchpadArea {
        _array: Some(array),
        _buffers: buffers,
    }
}

/// Allocates an event ring plus its segment table and programs the interrupter registers.
unsafe fn xhci_setup_interrupter(
    operational_base: VirtAddr,
    runtime_base: VirtAddr,
    index: u8,
    kind: XhciInterrupterKind,
    name: &'static str,
) -> Arc<XhciInterrupter> {
    let offset = runtime_interrupter_offset(index);

    let (ring_dma, ring_trbs) =
        TrbRing::dma_alloc(EVENT_RING_TRBS).expect("Failed to allocate dma for an event ring.");

    kprintln!(Debug, "Allocated DMA for the {} event ring at phys{:#011x}; virt: {:#011x} with align {}",
        name, ring_dma.phys, ring_dma.virt, ring_dma.layout.align());

    let erst_dma = xhci_alloc_dma_erst(ring_dma.phys, EVENT_RING_TRBS as u32)
        .expect("Failed to allocate dma for an erst.");

    kprintln!(Debug, "Allocated DMA for the {} erst at phys{:#011x}; virt: {:#011x} with align {}",
        name, erst_dma.phys, erst_dma.virt, erst_dma.layout.align());

    mmio_write::<u32>(
        runtime_base,
        RT_ERSTSZ as u64 + offset,
        SINGLE_ERST_SEGMENT,
    );
    mmio_write::<u64>(
        runtime_base,
        RT_ERSTBA as u64 + offset,
        erst_dma.phys.as_u64(),
    );
    mmio_write::<u64>(
        runtime_base,
        RT_ERDP as u64 + offset,
        ring_dma.phys.as_u64() & !ERDP_RESERVED_BITS,
    );

    let iman = mmio_read::<u32>(runtime_base, RT_IMAN as u64 + offset);
    mmio_write::<u32>(
        runtime_base,
        RT_IMAN as u64 + offset,
        iman | INTERRUPTER_MANAGEMENT_ENABLE,
    );

    let ring = EventRing::new(ring_trbs, ring_dma.phys);

    Arc::new(XhciInterrupter::new(
        operational_base,
        runtime_base,
        offset,
        ring,
        ring_dma,
        erst_dma,
        kind,
        name,
    ))
}

#[derive(Clone, Copy, Debug)]
pub enum XhciInterrupterKind {
    Primary,
    Transfer,
}

enum XhciInterruptConfig {
    Msix {
        capability: MsixCapability,
        pba: MsixPBA,
        command_vector: InterruptVector,
        transfer_vector: InterruptVector,
    },
    Msi {
        capability: MsiCapability,
        vector: InterruptVector,
    },
}

unsafe impl Send for XhciInterruptConfig {}
unsafe impl Sync for XhciInterruptConfig {}

//==================================================================================================
//  REGISTERS
//==================================================================================================
pub struct XhciRegs {
    cap_base: VirtAddr,
    operational_base: VirtAddr,
    runtime_base: VirtAddr,
    doorbell_offset: u32,
    context_size: u32,
    max_slots: u32,
    max_ports: usize,
    supported_protocols: Vec<XhciPortInfo>,
}

impl XhciRegs {
    fn is_valid_port(&self, port: u8) -> bool {
        port != 0 && (port as usize) <= self.max_ports
    }

    fn port_info(&self, port: u8) -> Option<XhciPortInfo> {
        if !self.is_valid_port(port) {
            return None;
        }

        self.supported_protocols.get((port - 1) as usize).copied()
    }

    fn portsc(&self, port: u8) -> PortStatusControl {
        PortStatusControl::from_port(self.operational_base, port)
    }

    fn portsc_ack_changes(&self, port: u8, snapshot: PortStatusControl) {
        PortStatusControl::ack_changes_of(snapshot).write_to_port(self.operational_base, port);
    }

    fn portsc_issue_reset(&self, port: u8) {
        let portsc = self.portsc(port);
        let mut cmd = PortStatusControl::write_from_raw(portsc.raw());
        cmd.pr_write();
        cmd.write_to_port(self.operational_base, port);
    }

    fn portsc_set_link_state(&self, port: u8, state: PortLinkState) {
        let portsc = self.portsc(port);
        let mut cmd = PortStatusControl::write_from_raw(portsc.raw());
        cmd.pls_write(state);
        cmd.write_to_port(self.operational_base, port);
    }

    #[inline(always)]
    fn ring_command_doorbell(&self) {
        unsafe {
            mmio_write::<u32>(self.cap_base, self.doorbell_offset as u64, 0);
        }
    }

    #[inline(always)]
    fn ring_doorbell(&self, slot_id: u8, endpoint_id: u8, stream_id: u16) {
        let doorbell_value = (endpoint_id as u32) | ((stream_id as u32) << 16);
        unsafe {
            mmio_write::<u32>(
                self.cap_base,
                (self.doorbell_offset + slot_id as u32 * 4) as u64,
                doorbell_value,
            );
        }
    }
}

//==================================================================================================
//  PORT STATE
//==================================================================================================
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ResumePhase {
    Resume,
    RExit,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum SetConfigurationPhase {
    PendingEndpoint,
    PendingDma,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum PortState {
    Idle,
    Debounce { generation: usize },
    ResetInProgress { attempts: u8 },
    Enabled,
    Resuming { phase: ResumePhase },
    PendingSetConfiguration,
    Configured
}

/// Per-port state machine plus the slot currently assigned to each port.
struct PortTable {
    state: [PortState; MAX_PORT_ENTRIES],
    slot: [Option<u8>; MAX_PORT_ENTRIES],
}

impl PortTable {
    fn new() -> Self {
        Self {
            state: [PortState::Idle; MAX_PORT_ENTRIES],
            slot: [None; MAX_PORT_ENTRIES],
        }
    }

    fn state(&self, port: u8) -> PortState {
        self.state[port as usize]
    }

    fn set_state(&mut self, port: u8, state: PortState) {
        self.state[port as usize] = state;
    }

    fn slot(&self, port: u8) -> Option<u8> {
        self.slot[port as usize]
    }

    fn set_slot(&mut self, port: u8, slot_id: u8) {
        self.slot[port as usize] = Some(slot_id);
    }

    fn clear_slot(&mut self, port: u8) {
        self.slot[port as usize] = None;
    }
}

//==================================================================================================
//  DCBAA / COMMAND RING / SCRATCHPAD OWNERSHIP
//==================================================================================================
struct DcbaaOwner {
    dma: DmaAlloc,
}

impl DcbaaOwner {
    fn new(dma: DmaAlloc) -> Self {
        Self { dma }
    }

    fn phys(&self) -> PhysAddr {
        self.dma.phys
    }

    fn set_context(&mut self, slot_id: usize, addr: u64) {
        unsafe { (*self.dma.virt.as_mut_ptr::<Dcbaa>()).set_context(slot_id, addr) }
    }

    fn clear_context(&mut self, slot_id: usize) {
        unsafe { (*self.dma.virt.as_mut_ptr::<Dcbaa>()).clear_context(slot_id) }
    }

    fn context_virt(&self, slot_id: usize) -> VirtAddr {
        unsafe { (*self.dma.virt.as_ptr::<Dcbaa>()).get_context_virt(slot_id) }
    }
}

//one lock because they're used together
struct CommandQueue {
    ring: CommandRing,
    shadow: ShadowRing<COMMAND_RING_TRBS>,
    dma: DmaAlloc,
}

impl CommandQueue {
    fn new(dma: DmaAlloc, ring: CommandRing) -> Self {
        let shadow = ShadowRing::new(dma.phys);
        Self { ring, shadow, dma }
    }

    fn submit(&mut self, trb: Trb, context: CommandContext) -> Result<PhysAddr, RingError> {
        let phys = self.ring.enqueue(trb)?;
        self.shadow.save_context_by_phys_addr(phys, context);
        Ok(phys)
    }

    fn take_context(&mut self, phys: PhysAddr) -> CommandContext {
        self.shadow.take_context_by_phys_addr(phys)
    }

    fn contains(&self, phys: PhysAddr) -> bool {
        let base = self.dma.phys.as_u64();
        let end = base + (COMMAND_RING_TRBS * size_of::<Trb>()) as u64;
        let addr = phys.as_u64();

        addr >= base && addr < end && (addr - base) % size_of::<Trb>() as u64 == 0
    }
}

unsafe impl Send for CommandQueue {}

/// Scratchpad buffers - for controller only
struct ScratchpadArea {
    _array: Option<DmaAlloc>,
    _buffers: Vec<DmaAlloc>,
}

//==================================================================================================
//  DEVICES
//==================================================================================================
#[derive(Ord, Eq, PartialEq, PartialOrd, Clone, Copy, Debug)]
struct TrbPhysAddr(u64);

struct TransferRingSlot {
    ring: TransferRing,
    _dma: DmaAlloc,
}

pub struct XhciDevice {
    input_context_dma: DmaAlloc,
    device_context_dma: DmaAlloc,
    transfer_rings: Vec<Option<TransferRingSlot>>,
    port: u8,
    slot_id: u8,
    first_8_bytes_of_device_descriptor: Option<DmaAlloc>,
    descriptor_probe_trb: Option<TrbPhysAddr>,
    pending_requests: BTreeMap<TrbPhysAddr, Arc<IrqMutex<UsbTransferRequest>>>,
}

unsafe impl Send for XhciDevice {}

impl XhciDevice {
    fn new(
        input_context_dma: DmaAlloc,
        ep0_transfer_ring_dma: DmaAlloc,
        ep0_transfer_ring: TransferRing,
        device_context_dma: DmaAlloc,
        port: u8,
        slot_id: u8,
    ) -> Self {
        let mut transfer_rings = Vec::with_capacity(MAX_DCI as usize + 1);
        transfer_rings.push(None); //dci=0 is slot context
        transfer_rings.push(Some(TransferRingSlot {
            ring: ep0_transfer_ring,
            _dma: ep0_transfer_ring_dma,
        }));

        for i in 0..(MAX_DCI as usize + 1 - 2) {
            transfer_rings.push(None); //I just want to finish this...
        }

        XhciDevice {
            input_context_dma,
            device_context_dma,
            transfer_rings,
            port,
            slot_id,
            first_8_bytes_of_device_descriptor: None,
            descriptor_probe_trb: None,
            pending_requests: BTreeMap::new(),
        }
    }

    fn transfer_ring_mut(&mut self, dci: u8) -> Option<&mut TransferRing> {
        self.transfer_rings
            .get_mut(dci as usize)?
            .as_mut()
            .map(|slot| &mut slot.ring)
    }

    fn insert_pending(&mut self, trb: TrbPhysAddr, request: Arc<IrqMutex<UsbTransferRequest>>) {
        self.pending_requests.insert(trb, request);
    }

    fn take_pending(&mut self, trb: TrbPhysAddr) -> Option<Arc<IrqMutex<UsbTransferRequest>>> {
        self.pending_requests.remove(&trb)
    }

    /// Removes one outstanding request so the caller can fail it.
    fn pop_pending(&mut self) -> Option<Arc<IrqMutex<UsbTransferRequest>>> {
        self.pending_requests.pop_first().map(|(_, request)| request)
    }

    /// Whether `trb` completes the descriptor read the driver issued for itself, consuming the
    /// record if it does.
    fn take_descriptor_probe(&mut self, trb: TrbPhysAddr) -> bool {
        if self.descriptor_probe_trb == Some(trb) {
            self.descriptor_probe_trb = None;
            return true;
        }

        false
    }

    /// The `bMaxPacketSize0` field the device reported, or `None` if the descriptor was never
    /// read back.
    fn reported_max_packet_size_0(&self) -> Option<u8> {
        let buffer = self.first_8_bytes_of_device_descriptor.as_ref()?;
        // Byte 7 of the device descriptor, the last one of the eight that were requested.
        Some(unsafe { ptr::read_volatile(buffer.virt.as_ptr::<u8>().add(7)) })
    }

    /// Queues a control transfer on EP0 and returns the address of the Status Stage TRB,
    /// which is the TRB the resulting transfer event points at.
    unsafe fn ep0_issue_request(
        &mut self,
        interrupter_target: u16,
        bm_request_type: u8,
        b_request: u8,
        transfer_length: usize,
        w_value: u16,
        w_index: u16,
        buffer_phys: Option<u64>,
    ) -> Result<TrbPhysAddr, RingError> {
        let has_data = transfer_length > 0;
        let is_in = (bm_request_type & 0x80) != 0;

        let data_buffer_phys = if has_data {
            match buffer_phys {
                Some(phys) => Some(phys),
                None => return Err(RingError::Unsupported),
            }
        } else {
            None
        };

        let transfer_ring = match self.transfer_ring_mut(EP0_DCI) {
            Some(ring) => ring,
            None => return Err(RingError::Unsupported),
        };

        let mut setup_stage_td = SetupStageTrb::new();
        let trt = if !has_data {
            SetupTransferType::NoDataStage
        } else if is_in {
            SetupTransferType::InDataStage
        } else {
            SetupTransferType::OutDataStage
        };

        setup_stage_td.set_trt(trt);
        setup_stage_td.set_transfer_length(8);
        setup_stage_td.set_ioc(false);
        setup_stage_td.set_idt(true);
        setup_stage_td.set_bm_request_type(bm_request_type);
        setup_stage_td.set_b_request(b_request);
        setup_stage_td.set_w_value(w_value); //low byte - descriptor index, high byte - descriptor type
        setup_stage_td.set_w_index(w_index);
        setup_stage_td.set_w_length(transfer_length as u16);
        setup_stage_td.set_interrupter_target(interrupter_target);

        transfer_ring.enqueue(*setup_stage_td.raw())?;

        if let Some(dma_alloc) = data_buffer_phys {
            let mut data_stage_td = DataStageTrb::new();
            let direction = if is_in { 1 } else { 0 };

            data_stage_td.set_direction(direction);
            data_stage_td.set_transfer_length(transfer_length as u32);
            data_stage_td.set_chain(false);
            data_stage_td.set_ioc(false);
            data_stage_td.set_idt(false);
            data_stage_td.set_data_buffer(dma_alloc);
            data_stage_td.set_interrupter_target(interrupter_target);

            transfer_ring.enqueue(*data_stage_td.raw())?;
        }

        let mut status_stage_td = StatusStageTrb::new();
        let status_direction = if has_data && is_in { 0 } else { 1 }; //reversed to the data stage

        status_stage_td.set_direction(status_direction);
        status_stage_td.set_chain(false);
        status_stage_td.set_ioc(true);
        status_stage_td.set_interrupter_target(interrupter_target);

        let status_phys = transfer_ring.enqueue(*status_stage_td.raw())?;

        Ok(TrbPhysAddr(status_phys.as_u64()))
    }

    unsafe fn issue_normal_transfer(
        &mut self,
        interrupter_target: u16,
        dci: u8,
        dma_buf: Option<u64>,
        transfer_length: usize,
    ) -> Result<TrbPhysAddr, RingError> {
        let transfer_ring = match self.transfer_ring_mut(dci) {
            Some(ring) => ring,
            None => return Err(RingError::Unsupported),
        };

        let buf_addr = if dma_buf.is_none() {
            0
        } else {
            dma_buf.unwrap()
        };

        let mut normal_trb = NormalTrb::new();
        normal_trb.set_data_buffer(buf_addr);
        normal_trb.set_transfer_length(transfer_length as u32);
        normal_trb.set_interrupt_on_completion(true);
        normal_trb.set_interrupt_on_short_packet(true);
        normal_trb.set_chain(false);
        normal_trb.set_interrupter_target(interrupter_target);

        let phys = transfer_ring.enqueue(*normal_trb.raw())?;

        Ok(TrbPhysAddr(phys.as_u64()))
    }
}

//==================================================================================================
//  INTERRUPTER
//==================================================================================================
struct EventRingState {
    ring: EventRing,
    _ring_dma: DmaAlloc,
    _erst_dma: DmaAlloc,
}

unsafe impl Send for EventRingState {}

pub struct XhciInterrupter {
    controller: Once<Weak<XHCI>>, //prevents reference counting loop
    operational_base: VirtAddr,
    runtime_base: VirtAddr,
    interrupter_offset: u64,
    event_ring: IrqMutex<EventRingState>,
    kind: XhciInterrupterKind,
    name: &'static str,
}

impl XhciInterrupter {
    fn new(
        operational_base: VirtAddr,
        runtime_base: VirtAddr,
        interrupter_offset: u64,
        ring: EventRing,
        ring_dma: DmaAlloc,
        erst_dma: DmaAlloc,
        kind: XhciInterrupterKind,
        name: &'static str,
    ) -> Self {
        Self {
            controller: Once::new(),
            operational_base,
            runtime_base,
            interrupter_offset,
            event_ring: IrqMutex::new(EventRingState {
                ring,
                _ring_dma: ring_dma,
                _erst_dma: erst_dma,
            }),
            kind,
            name,
        }
    }

    fn set_controller(&self, xhci: &Arc<XHCI>) {
        self.controller.call_once(|| Arc::downgrade(xhci));
    }

    fn controller(&self) -> Option<Arc<XHCI>> {
        self.controller.get().and_then(Weak::upgrade)
    }

    /// Advances the Event Ring Dequeue Pointer and clears the Event Handler Busy flag.
    fn ack(&self, dequeue_phys: u64) {
        unsafe {
            mmio_write::<u64>(
                self.runtime_base,
                RT_ERDP as u64 + self.interrupter_offset,
                (dequeue_phys & ERDP_PTR_MASK) | ERDP_EHB,
            );
        }
    }

    /// Acknowledges IMAN.IP and USBSTS.EINT.
    fn acknowledge_interrupt(&self) {
        unsafe {
            let iman = mmio_read::<u32>(self.runtime_base, RT_IMAN as u64 + self.interrupter_offset);
            mmio_write::<u32>(
                self.runtime_base,
                RT_IMAN as u64 + self.interrupter_offset,
                iman | INTERRUPTER_MANAGEMENT_PENDING,
            );

            mmio_write::<u32>(
                self.operational_base,
                OP_REG_USBSTS as u64,
                XHCI_STATUS_EVENT_INTERRUPT,
            );
        }
    }

    /// Copies up to [`EVENT_BATCH_TRBS`] events out of the ring and advances ERDP.
    /// Event ring lock is released before events are dispatched, so a handler that
    /// needs the command ring or a device cant block the other interrupter's ring.
    fn drain_batch(&self, batch: &mut [Trb; EVENT_BATCH_TRBS]) -> usize {
        let mut count = 0;

        let dequeue_phys = {
            let mut state = self.event_ring.lock();
            let mut attempts = 0;

            while count < EVENT_BATCH_TRBS && attempts < EVENT_RING_TRBS {
                attempts += 1;

                match state.ring.dequeue() {
                    Ok(trb) => {
                        batch[count] = trb;
                        count += 1;
                    }
                    Err(RingError::Empty) => break,
                    Err(x) => {
                        kprintln!(Error, "Malformed TRB detected. RingError: {:?}", x);
                    }
                }
            }

            state.ring.get_dequeue_phys()
        };

        if let Ok(phys) = dequeue_phys {
            // kprintln!("Acknowledged command.");
            self.ack(phys.as_u64());
        }

        count
    }

    unsafe fn handle(&self) {
        self.acknowledge_interrupt();
        // kprintln!("Handle interrupt");

        let controller = self.controller();
        let mut batch = [Trb::new(); EVENT_BATCH_TRBS];

        loop {
            let count = self.drain_batch(&mut batch);

            if let Some(xhci) = &controller {
                for trb in &batch[..count] {
                    xhci.dispatch_event(*trb);
                }
            }

            if count < EVENT_BATCH_TRBS {
                break;
            }
        }

        if controller.is_none() {
            kprintln!(
                Warn,
                "XHCI {} interrupter fired with controller present in software.",
                self.name
            );
        }
    }

    fn debug_print_first_event_trb(&self) {
        let state = self.event_ring.lock();
        let Ok(dequeue) = state.ring.get_dequeue_phys() else {
            return;
        };

        unsafe {
            let trb = ptr::read_volatile(physical_to_virtual(dequeue).as_ptr::<Trb>());
            kprintln!(
                Debug,
                "xHCI {} event TRB: type={} cycle={} param={:#018x} runtime_base={:#011x}",
                self.name,
                trb.trb_type(),
                trb.cycle(),
                trb.parameter(),
                self.runtime_base.as_u64()
            );
        }
    }
}

fn xhci_irq_handler(_: InterruptVector, _: InterruptStackFrame, context: usize) {
    if context == 0 {
        kprintln!(Warn, "xHCI IRQ without interrupter context");
        unsafe {
            if let Some(lapic) = LAPIC.get() {
                lapic.eoi();
            }
        }
        return;
    }

    unsafe {
        let interrupter = &*(context as *const XhciInterrupter);
        interrupter.handle();

        if let Some(lapic) = LAPIC.get() {
            lapic.eoi();
        }
    }
}

//==================================================================================================
//  CONTROLLER
//==================================================================================================
pub struct XHCI {
    regs: XhciRegs,
    pci_device: PciDevice,
    dcbaa: IrqMutex<DcbaaOwner>,
    commands: IrqMutex<CommandQueue>,
    devices: IrqMutex<BTreeMap<u8, Arc<IrqMutex<XhciDevice>>>>,
    ports: IrqMutex<PortTable>,
    interrupters: Vec<Arc<XhciInterrupter>>,
    interrupt_config: XhciInterruptConfig,
    transfer_interrupter_target: u16,
    _scratchpad: ScratchpadArea,
    id: usize,
}

impl XHCI {
    fn send_command(&self, trb: Trb, context: CommandContext) -> Result<PhysAddr, RingError> {
        let phys = {
            let mut commands_guard = self.commands.lock();
            commands_guard.submit(trb, context)?
        };
        compiler_fence(Ordering::SeqCst);

        unsafe { _mm_mfence() }
        self.regs.ring_command_doorbell();

        Ok(phys)
    }

    fn device(&self, slot_id: u8) -> Option<Arc<IrqMutex<XhciDevice>>> {
        self.devices.lock().get(&slot_id).cloned()
    }

    /// Requests slot teardown for `port`. Expects the caller to already hold the port table.
    unsafe fn disable_slot_locked(&self, ports: &mut PortTable, port: u8) {
        let Some(slot_id) = ports.slot(port) else {
            kprintln!(
                Warn,
                "[PORT {}] Tried detaching device with no slot assigned (or not present in software)!",
                port
            );
            ports.set_state(port, PortState::Idle);
            return;
        };

        let disable_slot_trb = DisableSlotCommandTrb::new(slot_id);
        if let Err(error) = self.send_command(
            *disable_slot_trb.raw(),
            CommandContext::DisableSlot {
                port_id: port,
                slot_id,
            },
        ) {
            kprintln!(
                Error,
                "[SLOT {}] [PORT {}] Sending disable slot command failed: {:?}",
                slot_id,
                port,
                error
            );
        }

        ports.set_state(port, PortState::Idle);
    }

    unsafe fn disable_slot(&self, port: u8) {
        let mut ports = self.ports.lock();
        self.disable_slot_locked(&mut ports, port);
    }

    unsafe fn resume_port(&self, ports: &mut PortTable, port: u8) {
        self.regs.portsc_set_link_state(port, PortLinkState::Resume);
        kprintln!(Debug, "[PORT {}] Transitioning port to Resume state.", port);
        ports.set_state(
            port,
            PortState::Resuming {
                phase: ResumePhase::Resume,
            },
        );
        schedule_resume_to_rexit(self, port);
    }

    /// Continues the reset state machine after a Port Reset Change event.
    unsafe fn complete_port_reset(
        &self,
        ports: &mut PortTable,
        port: u8,
        portsc: PortStatusControl,
    ) {
        if !portsc.ccs_read() {
            kprintln!(
                Debug,
                "[PORT {}] Device was disconnected before reset could be completed.",
                port
            );
            ports.set_state(port, PortState::Idle);
            return;
        }

        if portsc.ped_read() && !portsc.pr_read() && portsc.pls_read() == U0 {
            kprintln!(Debug, "[PORT {}] Succesfully reset port.", port); //TODO: usb3
            ports.set_state(port, PortState::Enabled);
        } else {
            if let PortState::ResetInProgress { attempts } = ports.state(port) {
                if attempts < PORT_RESET_MAX_ATTEMPTS && portsc.ccs_read() {
                    kprintln!(
                        Warn,
                        "[PORT {}] Port reset was incomplete, retry {}.",
                        port,
                        attempts + 1
                    );
                    ports.set_state(
                        port,
                        PortState::ResetInProgress {
                            attempts: attempts + 1,
                        },
                    );
                    schedule_reset_timeout(self, port, attempts + 1);
                    self.regs.portsc_issue_reset(port);
                    return;
                }
            }

            kprintln!(
                Warn,
                "[PORT {}] Port reset timed out, PORTSC={:#010x} (PED={} PLS={:?} PRC={} CCS={})",
                port,
                portsc.raw(),
                portsc.ped_read(),
                portsc.pls_read(),
                portsc.prc_read(),
                portsc.ccs_read()
            );
            ports.set_state(port, PortState::Idle);
            return;
        }

        //we did reset the slot, so now we're in enabled state
        //TODO: usb3 logic (but it's probably the same, prioritizing usb2 for keyboard support)
        kprintln!(Debug, "[PORT {}] Sending enable slot command for port.", port);

        let slot_type = 0; //99,9999999999% cases its just zero
        let trb = EnableSlotCommandTrb::new_command(slot_type);
        if let Err(error) =
            self.send_command(*trb.raw(), CommandContext::EnableSlot { port_id: port })
        {
            kprintln!(
                Error,
                "[PORT {}] Sending enable slot command failed: {:?}",
                port,
                error
            );
            ports.set_state(port, PortState::Idle);
        }
    }


    unsafe fn dispatch_event(self: &Arc<Self>, trb: Trb) {
        let trb_type = trb.trb_type();

        if trb_type == Trb::TRB_PORT_STATUS_CHANGE_EVENT {
            match trb.try_as_port_status_change_event() {
                Ok(event) => self.on_port_status_change(event),
                Err(error) => {
                    kprintln!(Error, "Cannot parse port status change TRB: {:?}", error);
                }
            }
        } else if trb_type == Trb::TRB_COMMAND_COMPLETION_EVENT {
            match trb.try_as_command_completion_event() {
                Ok(event) => self.on_command_completion(event),
                Err(error) => {
                    kprintln!(Error, "Cannot parse command completion TRB: {:?}", error);
                }
            }
        } else if trb_type == Trb::TRB_TRANSFER_EVENT {
            match trb.try_as_transfer_event() {
                Ok(event) => self.on_transfer_event(event),
                Err(error) => {
                    kprintln!(Error, "Cannot parse transfer event TRB: {:?}", error);
                }
            }
        } else {
            kprintln!(Debug, "Trb type {} received.", trb_type);
        }
    }

    unsafe fn on_port_status_change(self: &Arc<Self>, trb: PortStatusChangeEventTrb) {
        let port = trb.port_id();

        if !self.regs.is_valid_port(port) {
            kprintln!(
                Info,
                "[INVALID PORT {}] Ignoring port status change for this invalid port.",
                port
            );
            return;
        }

        let portsc = self.regs.portsc(port);
        let mut ports = self.ports.lock();

        kprintln!(Debug,
            "[PORT {}] PSC event for port: PORTSC={:#010x} [CSC={} PEC={} PRC={} PLC={} WRC={} OCC={}] CCS={} PED={} PLS={:?} state={:?}",
            port, portsc.raw(),
            portsc.csc_read(), portsc.pec_read(), portsc.prc_read(),
            portsc.plc_read(), portsc.wrc_read(), portsc.occ_read(),
            portsc.ccs_read(), portsc.ped_read(), portsc.pls_read(),
            ports.state(port)
        );

        self.regs.portsc_ack_changes(port, portsc);
        kprintln!(Debug, "[PORT {}] Cleared port status change event.", port);

        let mut reset_completed = false;
        if portsc.prc_read() {
            if matches!(ports.state(port), PortState::ResetInProgress { .. }) {
                self.complete_port_reset(&mut ports, port, portsc);
                reset_completed = true;
            } else {
                kprintln!(
                    Warn,
                    "[PORT {}] Unexpected PRC in state {:?}.",
                    port,
                    ports.state(port)
                );
            }
        }

        if !portsc.csc_read() {
            return;
        }

        let protocol = self
            .regs
            .port_info(port)
            .map(|info| info.protocol)
            .unwrap_or(XhciPortProtocol::Unknown);

        if !portsc.ccs_read() {
            kprintln!(
                Info,
                "[PORT {}] Detach detected with {} protocol.",
                port,
                protocol
            );
            self.disable_slot_locked(&mut ports, port);
            return;
        }

        if reset_completed || matches!( ports.state(port),
                PortState::Debounce { .. } | PortState::ResetInProgress { .. }
            ) {
            kprintln!(
                Debug,
                "[PORT {}] Connect status change belongs to the connection already being enumerated.",
                port
            );
            return;
        }

        if matches!(ports.state(port), PortState::Enabled) {
            kprintln!(
                Warn,
                "[PORT {}] Device on addressed port disconnected/bounced.",
                port
            );

            if ports.slot(port).is_some() {
                self.disable_slot_locked(&mut ports, port);
            }
        }

        ports.set_state(port, PortState::Debounce { generation: 0 });
        schedule_debounce(self, port, 0);

        kprintln!(
            Info,
            "[PORT {}] Attach detected with {} protocol.",
            port,
            protocol
        );
    }

    unsafe fn on_endpoint_configured(self: &Arc<Self>, trb: CommandCompletionEventTrb) {
        if trb.completion_code() != TrbCompletionCode::SUCCESS {
            kprintln!(Error, "[SLOT {}] Configure endpoint command failed with code {}", trb.slot_id(), trb.completion_code());
            return; //TODO: better way to handle this without a deadlock
        }

        let phys_addr = TrbPhysAddr(trb.command_trb_pointer());
        let (port, req) = {
            let dev_lock = self.device(trb.slot_id());
            let mut dev = dev_lock.as_ref().unwrap().lock();
            let port = dev.port;

            //we already inserted the request to the map with the key of command trb
            let req = dev.pending_requests.remove(&phys_addr)
                .expect("No pending request present for configure event");

            (port, req)
        };


        let port_state = {
            let ports = self.ports.lock();
            ports.state[port as usize]
        };

        if !matches!(port_state, PendingSetConfiguration) {
            kprintln!(Warn, "Tried sending SET_CONFIGURATION while not in pending set configuration state.");
            return;
        }

        kprintln!(Debug, "Endpoint configured, sending SET_CONFIGURATION request.");
        let dev_lock = self.device(trb.slot_id());
        let mut dev = dev_lock.as_ref().unwrap().lock();

        let setup = {
            let req_lock = req.lock();
            req_lock.setup_packet
                .expect("no setup packet in set configuration request")
        };
        kprintln!("Created setup packer.");

        let phys_address_new = dev.ep0_issue_request(
            self.transfer_interrupter_target,
            setup.bm_request_type,
            setup.b_request,
            0,
            setup.w_value,
            setup.w_index,
            None, //you stupid idiot there's no dma buffer inside set configuration
        )
            .expect("Failed to issue set configuration request after configure endpoint command.");

        kprintln!("Issued ep0 set configuration request.");

        //TODO: this can cause deadlocks :(((((((((((((((((((((((((((((((((((((((((((((((((((((((((
        dev.pending_requests.insert(
            phys_address_new,
            req
        );

        kprintln!("Inserted request to pending map.");

        self.regs.ring_doorbell(dev.slot_id, 1,0);
        kprintln!("Rang doorbell for set configuration request.");
    }

    unsafe fn on_command_completion(self: &Arc<Self>, trb: CommandCompletionEventTrb) {
        let command_trb_phys = PhysAddr::new(trb.command_trb_pointer());

        let context = {
            let mut commands = self.commands.lock();
            if !commands.contains(command_trb_phys) {
                kprintln!(
                    Error,
                    "Command completion points at {:#x}, which is outside the command ring.",
                    command_trb_phys.as_u64()
                );
                return;
            }
            commands.take_context(command_trb_phys)
        };

        let command_trb = ptr::read_volatile(physical_to_virtual(command_trb_phys).as_ptr::<Trb>());

        if !command_trb.is_command_trb() {
            kprintln!(
                Error,
                "Command completion points at a non-command TRB (type {}) at {:#x}.",
                command_trb.trb_type(),
                command_trb_phys.as_u64()
            );
            return;
        }

        match command_trb.trb_type() {
            Trb::TRB_ENABLE_SLOT_COMMAND => self.on_enable_slot_completion(trb, context),
            Trb::TRB_ADDRESS_DEVICE_COMMAND => {
                self.on_address_device_completion(trb, context, command_trb)
            }
            Trb::TRB_DISABLE_SLOT_COMMAND => self.on_disable_slot_completion(trb, context),
            Trb::TRB_EVALUATE_CONTEXT_COMMAND => self.on_evaluate_context_completion(trb),
            Trb::TRB_CONFIGURE_ENDPOINT_COMMAND => self.on_endpoint_configured(trb),
            other => {
                kprintln!(
                    Warn,
                    "[SLOT {}] Received unhandled command completion event with type {}.",
                    trb.slot_id(),
                    other
                );
            }
        }
    }

    unsafe fn on_enable_slot_completion(
        self: &Arc<Self>,
        trb: CommandCompletionEventTrb,
        context: CommandContext,
    ) {
        let slot_id = trb.slot_id();
        let CommandContext::EnableSlot { port_id: port } = context else {
            kprintln!(
                Error,
                "[SLOT {}] Invalid command context type! Expected enable slot, found {:?}.",
                slot_id,
                context
            );
            return;
        };

        {
            let mut ports = self.ports.lock();

            if trb.completion_code() != TrbCompletionCode::SUCCESS || slot_id == 0 {
                kprintln!(
                    Warn,
                    "[SLOT {}][PORT {}] Handling enable slot command resulted in non successful exit code {}.",
                    slot_id, port, trb.completion_code()
                );
                self.disable_slot_locked(&mut ports, port);
                return;
            }

            //check if the port has a slot bound to it already to prevent weird stuff
            if let Some(existing) = ports.slot(port) {
                kprintln!(
                    Warn,
                    "[SLOT {}][PORT {}] Port already owns slot {}, giving the duplicate slot back.",
                    slot_id,
                    port,
                    existing
                );

                let disable_slot_trb = DisableSlotCommandTrb::new(slot_id);
                if let Err(error) = self.send_command(
                    *disable_slot_trb.raw(),
                    CommandContext::DisableSlot {
                        port_id: port,
                        slot_id,
                    },
                ) {
                    kprintln!(
                        Error,
                        "[SLOT {}][PORT {}] Sending disable slot command failed: {:?}",
                        slot_id,
                        port,
                        error
                    );
                }

                return;
            }

            kprintln!(
                Info,
                "[SLOT {}][PORT {}] Successfully assigned slot {}.",
                slot_id,
                port,
                slot_id
            );
            ports.set_slot(port, slot_id);
        }

        defer_with_xhci(self.id, 10, move |xhci| unsafe {
            xhci.allocate_device(port, slot_id)
        });
    }

    /// Builds the input context, EP0 transfer ring and output device context for a freshly
    /// enabled slot, then issues Address Device.
    unsafe fn allocate_device(self: &Arc<Self>, port: u8, slot_id: u8) {
        let ports = self.ports.lock();
        if ports.slot(port) != Some(slot_id) {
            kprintln!(
                Debug,
                "[SLOT {}][PORT {}] Aborting allocation - port state changed.",
                slot_id,
                port
            );
            return;
        }
        drop(ports);

        kprintln!(
            Debug,
            "[SLOT {}][PORT {}] Began allocating data structures for device.",
            slot_id,
            port
        );

        let port_speed = self.regs.portsc(port).ps_read();

        let Some(input_context_dma) = dma_alloc_zeroed(size_of::<InputContext>(), 4096) else {
            kprintln!(
                Error,
                "[SLOT {}][PORT {}] Failed to allocate input context.",
                slot_id,
                port
            );
            self.disable_slot(port);
            return;
        };

        let Some((transfer_ring_dma, transfer_trbs)) = TrbRing::dma_alloc(TRANSFER_RING_TRBS) else {
            kprintln!(
                Error,
                "[SLOT {}][PORT {}] Failed to allocate transfer ring for device.",
                slot_id,
                port
            );
            self.disable_slot(port);
            return;
        };

        let Some(device_context_dma) = dma_alloc_zeroed(size_of::<DeviceContext>(), 4096) else {
            kprintln!(
                Error,
                "[SLOT {}][PORT {}] Failed to allocate device context.",
                slot_id,
                port
            );
            self.disable_slot(port);
            return;
        };

        let input_context = &mut *(input_context_dma.virt.as_mut_ptr::<InputContext>());
        let ic_control = input_context.control();
        ic_control.add_context(0);
        ic_control.add_context(1);

        let input_slot_context = input_context.slot::<SlotContext>(self.regs.context_size);
        input_slot_context.set_root_hub_port(port);
        // only devices directly attached to the root hub are handled here, so the route
        // string stays zero
        input_slot_context.set_route_string(0);
        input_slot_context.set_context_entries(1);
        input_slot_context.set_speed(port_speed);

        let max_packet_size = match port_speed {
            1 => 8,
            2 => 8,
            3 => 64,
            4 => 512,
            _ => 8, //fallback
        };

        let transfer_ring = TransferRing::new(transfer_trbs, transfer_ring_dma.phys);

        //ici=2 is the index of ep0
        let ep_0 = input_context.endpoint::<EndpointContext>(2, self.regs.context_size);
        ep_0.set_ep_type(EndpointContext::EP_TYPE_CONTROL);
        ep_0.set_max_packet_size(max_packet_size);
        ep_0.set_max_burst(0);
        ep_0.set_tr_dequeue_ptr(transfer_ring_dma.phys.as_u64());
        ep_0.set_dequeue_cycle_state(true);
        ep_0.set_interval(0);
        ep_0.set_max_pstreams(0);
        ep_0.set_mult(0);
        ep_0.set_error_count(3);

        let mut address_command_trb = AddressDeviceCommandTrb::new();
        address_command_trb.set_slot_id(slot_id);
        address_command_trb.set_input_context_pointer(input_context_dma.phys.as_u64());
        address_command_trb.set_bsr(false);

        let device_context_phys = device_context_dma.phys.as_u64();
        let device = XhciDevice::new(
            input_context_dma,
            transfer_ring_dma,
            transfer_ring,
            device_context_dma,
            port,
            slot_id,
        );

        self.devices
            .lock()
            .insert(slot_id, Arc::new(IrqMutex::new(device)));
        self.dcbaa
            .lock()
            .set_context(slot_id as usize, device_context_phys);

        kprintln!(
            Debug,
            "[SLOT {}][PORT {}] Succesfully allocated required data structures for device.",
            slot_id,
            port
        );

        if let Err(error) = self.send_command(
            *address_command_trb.raw(),
            CommandContext::AddressDevice { slot_id },
        ) {
            kprintln!(
                Error,
                "[SLOT {}][PORT {}] Failed to send address device command: {:?}",
                slot_id,
                port,
                error
            );
            self.disable_slot(port);
        }
    }

    unsafe fn on_address_device_completion(
        self: &Arc<Self>,
        trb: CommandCompletionEventTrb,
        context: CommandContext,
        command_trb: Trb,
    ) {
        let CommandContext::AddressDevice { slot_id } = context else {
            kprintln!(
                Error,
                "Invalid context type! Expected AddressDevice found {:?}.",
                context
            );
            return;
        };

        if slot_id == 0 {
            kprintln!(Error, "[SLOT 0] 0 is not a valid slot ID inside command context!");
            return;
        }

        let Some(device) = self.device(slot_id) else {
            kprintln!(
                Error,
                "[SLOT {}] Tried handling address device completion when device is not present in software!",
                slot_id
            );
            return;
        };

        let port = device.lock().port;

        match trb.completion_code() {
            TrbCompletionCode::SUCCESS => {}
            TrbCompletionCode::CONTEXT_STATE_ERROR => {
                kprintln!(Error, "[SLOT {}] [PORT {}] Not in enabled state!", slot_id, port);
                return;
            }
            TrbCompletionCode::USB_TRANSACTION_ERROR => {
                //TODO: some usb2 mice end up here and cant finish initialization - investigate
                kprintln!(
                    Debug,
                    "[SLOT {}] [PORT {}] SET_ADDRESS request was not successful. The device was likely removed.",
                    slot_id, port
                );
                self.disable_slot(port);
                return;
            }
            TrbCompletionCode::SLOT_NOT_ENABLED_ERROR => {
                kprintln!(
                    Debug,
                    "[SLOT {}] [PORT {}] Was not enabled by enable slot command. The device was likely removed.",
                    slot_id, port
                );
                return;
            }
            code => {
                kprintln!(
                    Error,
                    "[SLOT {}] [PORT {}] Address Device Command FAILED with code: {}",
                    slot_id,
                    port,
                    code
                );
                self.disable_slot(port);
                return;
            }
        }

        let bsr = match AddressDeviceCommandTrb::from_raw(command_trb) {
            Ok(command) => command.bsr(),
            Err(error) => {
                kprintln!(
                    Error,
                    "[SLOT {}] Cannot parse the completed address device command: {:?}",
                    slot_id,
                    error
                );
                return;
            }
        };

        if !self.verify_addressed_device(slot_id, bsr) {
            self.disable_slot(port);
            return;
        }

        kprintln!(
            Info,
            "[SLOT {}] Successfully handled address device command.",
            slot_id
        );

        //now, we need to obtain the max packet size. for all the speeds except for full speed
        //this is already known and set before
        let Some(device_descriptor_8b) = dma_alloc_zeroed(8, 8) else {
            kprintln!(
                Error,
                "[SLOT {}] Failed to allocate memory for 8bytes of usb descriptor.",
                slot_id
            );
            return;
        };

        let transfer_length = 8;
        let w_index = 0;
        let interrupter_target = self.transfer_interrupter_target;

        let issued = {
            let mut dev = device.lock();
            let issued = dev.ep0_issue_request(
                interrupter_target,
                UsbBmRequestType::new(
                    UsbBmRequestType::DIR_DEVICE_TO_HOST,
                    UsbBmRequestType::TYPE_STANDARD,
                    UsbBmRequestType::REC_DEVICE,
                ),
                UsbBRequest::GET_DESCRIPTOR,
                transfer_length,
                WValue::new(WValue::DESC_DEVICE, 0),
                w_index,
                Some(device_descriptor_8b.phys.as_u64()),
            );

            if let Ok(status_stage_trb) = issued {
                dev.first_8_bytes_of_device_descriptor = Some(device_descriptor_8b);
                dev.descriptor_probe_trb = Some(status_stage_trb);
            }

            issued
        };

        match issued {
            Ok(_) => self.regs.ring_doorbell(slot_id, EP0_DCI, 0),
            Err(error) => {
                kprintln!(
                    Error,
                    "[SLOT {}] Failed to request the device descriptor: {:?}",
                    slot_id,
                    error
                );
            }
        }
    }

    /// Compares the slot context the controller produced against what was asked for.
    unsafe fn verify_addressed_device(&self, slot_id: u8, bsr: bool) -> bool {
        let output_context_virt = self.dcbaa.lock().context_virt(slot_id as usize);
        let output_device_context = &*(output_context_virt.as_ptr::<DeviceContext>());
        let output_slot_context = output_device_context.slot();
        let output_ep0 = output_device_context.endpoint(1, self.regs.context_size);

        let ep0_state = output_ep0.get_ep_state();
        if ep0_state != EndpointState::RUNNING {
            kprintln!(
                Error,
                "[SLOT {}] EP0 is in state {:?} after Address Device, expected RUNNING.",
                slot_id,
                ep0_state
            );
            return false;
        }

        let slot_state = output_slot_context.get_slot_state();
        let usb_address = output_slot_context.get_usb_address();

        if bsr {
            if slot_state != SlotState::DEFAULT || usb_address != 0 {
                kprintln!(
                    Error,
                    "[SLOT {}] Expected DEFAULT state with address 0 after BSR Address Device, got {:?}/{}.",
                    slot_id, slot_state, usb_address
                );
                return false;
            }
            return true;
        }

        if slot_state != SlotState::ADDRESSED || usb_address == 0 {
            kprintln!(
                Error,
                "[SLOT {}] Expected ADDRESSED state with a non-zero address, got {:?}/{}.",
                slot_id,
                slot_state,
                usb_address
            );
            return false;
        }

        kprintln!(
            Info,
            "[SLOT {}] SET_ADDRESS request completed successfully.",
            slot_id
        );
        true
    }

    unsafe fn on_disable_slot_completion(
        &self,
        trb: CommandCompletionEventTrb,
        context: CommandContext,
    ) {
        let CommandContext::DisableSlot {
            port_id: port,
            slot_id,
        } = context
        else {
            kprintln!(
                Error,
                "Invalid context type! Expected SlotDisable found {:?}.",
                context
            );
            return;
        };

        if trb.completion_code() == TrbCompletionCode::SLOT_NOT_ENABLED_ERROR {
            kprintln!(
                Warn,
                "[SLOT {}] [PORT {}] Disabling slot failed! Slot has not been enabled by an Enable Slot command.",
                slot_id, port
            );
        } else if trb.completion_code() != TrbCompletionCode::SUCCESS {
            kprintln!(
                Error,
                "[SLOT {}] [PORT {}]  Disabling slot failed! Unexpected trb completion code {}.",
                slot_id,
                port,
                trb.completion_code()
            );
            return;
        }

        kprintln!(
            Debug,
            "[SLOT {}] [PORT {}]  Received successful Disable Slot command trb completion code.",
            slot_id,
            port
        );

        self.release_slot(port, slot_id);

        kprintln!(
            Debug,
            "[SLOT {}] [PORT {}]  Finished deallocating slot's memory.",
            slot_id,
            port
        );
    }

    /// Drops all state belonging to a slot the controller has finished disabling.
    fn release_slot(&self, port: u8, slot_id: u8) {
        self.dcbaa.lock().clear_context(slot_id as usize);

        {
            // Only reset the port when it is really this slot that is going away. A duplicate
            // slot handed back to the controller must not disturb the slot the port still uses.
            let mut ports = self.ports.lock();
            if ports.slot(port) == Some(slot_id) {
                ports.set_state(port, PortState::Idle);
                ports.clear_slot(port);
            }
        }

        let device = self.devices.lock().remove(&slot_id);
        let Some(device) = device else {
            return;
        };

        // Requests are failed before `device` goes out of scope, because dropping it frees the
        // transfer rings and DMA buffers those requests refer to.
        loop {
            let pending = device.lock().pop_pending();
            let Some(request) = pending else {
                break;
            };

            complete_request(request, UsbTransferStatus::Error, 0);
        }
    }

    unsafe fn on_evaluate_context_completion(self: &Arc<Self>, trb: CommandCompletionEventTrb) {
        let slot_id = trb.slot_id();

        if trb.completion_code() != TrbCompletionCode::SUCCESS {
            kprintln!(
                Error,
                "[SLOT {}] Evaluate context command completed with completion code {}.",
                slot_id,
                trb.completion_code()
            );
            return;
        }

        self.register_with_usb_core(slot_id);
    }

    unsafe fn on_transfer_event(self: &Arc<Self>, trb: TransferEventTrb) {
        let slot_id = trb.slot_id();
        let event_trb = TrbPhysAddr(trb.trb_pointer());
        let completion_code = trb.completion_code();

        let Some(device) = self.device(slot_id) else {
            kprintln!(
                Warn,
                "[SLOT {}] Transfer event for a device not present in software.",
                slot_id
            );
            return;
        };

        let status = match completion_code {
            TrbCompletionCode::SUCCESS | TrbCompletionCode::SHORT_PACKET => {
                UsbTransferStatus::Completed
            }
            TrbCompletionCode::STALL_ERROR => UsbTransferStatus::Stalled,
            code => {
                kprintln!(
                    Warn,
                    "[SLOT {}] Transfer failed with completion code {}.",
                    slot_id,
                    code
                );
                UsbTransferStatus::Error
            }
        };

        //this reports number of bytes that were NOT TRANSFERRED
        let residual = trb.transfer_length() as usize;

        match self.finished_trb_type(event_trb) {
            Trb::TRB_STATUS_STAGE => {
                self.on_control_transfer_completion(&device, slot_id, event_trb, status, residual)
            }
            Trb::TRB_NORMAL => {
                self.on_data_transfer_completion(&device, slot_id, event_trb, status, residual)
            }
            other => {
                kprintln!(
                    Debug,
                    "[SLOT {}] Transfer event for unhandled TRB type {} with completion code {}.",
                    slot_id,
                    other,
                    completion_code
                );
            }
        }
    }

    /// Type of the TRB a transfer event points at.
    /// Returns [`Trb::TRB_RESERVED0`] if there's no reported pointer.
    unsafe fn finished_trb_type(&self, event_trb: TrbPhysAddr) -> u8 {
        if event_trb.0 == 0 {
            return Trb::TRB_RESERVED0;
        }

        ptr::read_volatile(physical_to_virtual(PhysAddr::new(event_trb.0)).as_ptr::<Trb>())
            .trb_type()
    }

    unsafe fn on_pending_set_configuration(&self, pending: Arc<IrqMutex<UsbTransferRequest>>, xhci_device: &Arc<IrqMutex<XhciDevice>>) {
        kprintln!(Debug, "Began endpoint configuration for device");

        let target_dev = {
            let req = pending.lock();
            req.target_device.clone()
        };

        if target_dev.configuration_tree.get().is_none() {
            kprintln!(Error, "Device has no interfaces present inside configuration descriptor.");
            return;
        }

        let context_size = self.regs.context_size;

        let mut dev = xhci_device.lock();
        let input_context = &mut *dev.input_context_dma.virt.as_mut_ptr::<InputContext>();

        input_context.control().reset_flags();

        let output_context = &mut *dev.device_context_dma.virt.as_mut_ptr::<DeviceContext>();
        let output_slot_context = output_context.slot();
        let input_slot_context = input_context.slot::<SlotContext>(context_size);


        input_slot_context.dword0 = output_slot_context.dword0;
        input_slot_context.dword1 = output_slot_context.dword1;
        input_slot_context.dword2 = output_slot_context.dword2;
        input_slot_context.dword3 = output_slot_context.dword3;

        let slot_context = input_context.slot::<SlotContext>(context_size);
        let mut max_dci = slot_context.get_context_entries();

        for interface in &target_dev.configuration_tree.get().unwrap().interfaces {
            for ep in &interface.endpoints {
                let (transfer_ring_dma, trbs) = TrbRing::dma_alloc(TRANSFER_RING_TRBS)
                    .expect("Dma alloc for transfer ring for new endpoint failed.");

                let transfer_ring = TransferRing::new(trbs, transfer_ring_dma.phys);

                let dci = xhci_endpoint_address_to_dci(ep.b_endpoint_address);
                let ici = dci + 1;

                if dci > max_dci {
                    max_dci = dci;
                }

                input_context.control().add_context(dci);

                let ctx = input_context.endpoint::<EndpointContext>(ici as usize, context_size);
                ctx.set_tr_dequeue_ptr(transfer_ring_dma.phys.as_u64());
                ctx.set_dequeue_cycle_state(true);

                let mps = (ep.w_max_packet_size & 0x7FF) as u32;
                let extra = ((ep.w_max_packet_size >> 11) & 0x3) as u32;

                ctx.set_max_packet_size(mps);

                dev.transfer_rings[dci as usize] = Some(TransferRingSlot {
                    ring: transfer_ring,
                    _dma: transfer_ring_dma,
                });


                let port_speed = PortStatusControl::from_port(
                    self.regs.operational_base, dev.port
                ).ps_read();



                let xhci_interval = if port_speed == XhciPortSpeed::LOW_SPEED || port_speed == XhciPortSpeed::FULL_SPEED {
                    //full/low speed
                    if ep.b_interval > 0 {
                        let log2 = 31 - (ep.b_interval as u32).leading_zeros();
                        log2 + 3
                    } else {
                        0
                    }
                } else {
                    //high speed and others
                    (ep.b_interval - 1) as u32
                };


                ctx.set_interval(xhci_interval);

                let transfer_type = ep.bm_attributes & 0b11;
                let is_in = (ep.b_endpoint_address & 0x80) != 0;
                let ep_type = match transfer_type {
                    1 => if is_in { 5 } else { 1 }, // isoch
                    2 => if is_in { 6 } else { 2 }, // bulk
                    3 => if is_in { 7 } else { 3 }, // interrupt
                    _ => {
                        kprintln!(Error, "Invalid ep type");
                        continue;
                    },
                };

                let ep_is_isochronous = (ep.bm_attributes & 0b11) == 0b01;
                let error_count = if ep_is_isochronous {
                    0
                } else {
                    3
                };


                ctx.set_ep_type(ep_type);
                ctx.set_error_count(error_count);
                ctx.set_max_burst(0);
                ctx.set_avg_trb_len(ep.w_max_packet_size as u32);
                ctx.set_max_esit_payload(extra);
            }
        }

        let slot_context = input_context.slot::<SlotContext>(context_size);
        slot_context.set_context_entries(max_dci);
        input_context.control().add_context(0);

        let mut cmd = ConfigureEndpointCommandTrb::new();
        cmd.set_input_context_pointer(dev.input_context_dma.phys.as_u64());
        cmd.set_slot_id(dev.slot_id);
        cmd.set_deconfigure(false);

        let phys = self.send_command(*cmd.raw(), CommandContext::Empty)
            .expect("Sending configure endpoint command failed");

        kprintln!(Debug, "Sent configure endpoint command.");

        //insert that to device's pending requets, so that we can use it inside command completion handler
        dev.pending_requests.insert(
            TrbPhysAddr(phys.as_u64()),
            pending
        );
    }

    /// Handles the end of a control transfer, which the controller reports against the Status
    /// Stage TRB that closes the descriptor.
    unsafe fn on_control_transfer_completion(
        self: &Arc<Self>,
        device: &Arc<IrqMutex<XhciDevice>>,
        slot_id: u8,
        event_trb: TrbPhysAddr,
        status: UsbTransferStatus,
        residual: usize,
    ) {
        let (pending, is_descriptor_probe, port) = {
            let mut dev = device.lock();
            (
                dev.take_pending(event_trb),
                dev.take_descriptor_probe(event_trb),
                dev.port
            )
        };

        let port_state = {
            self.ports.lock().state[port as usize]
        };

        if port_state == PendingSetConfiguration {
            self.ports.lock().state[port as usize] = Configured;
        }

        if let Some(request) = pending {
            complete_request(request, status, residual);
            return;
        }

        if is_descriptor_probe {
            self.finish_max_packet_size_probe(device, slot_id, status);
            return;
        }

        kprintln!(
            Warn,
            "[SLOT {}] Control transfer finished with nothing waiting for it.",
            slot_id
        );
    }

    /// Handles the end of a bulk or interrupt transfer.
    fn on_data_transfer_completion(
        &self,
        device: &Arc<IrqMutex<XhciDevice>>,
        slot_id: u8,
        event_trb: TrbPhysAddr,
        status: UsbTransferStatus,
        residual: usize,
    ) {
        let Some(request) = device.lock().take_pending(event_trb) else {
            kprintln!(
                Warn,
                "[SLOT {}] Data transfer finished with nothing waiting for it.",
                slot_id
            );
            return;
        };

        complete_request(request, status, residual);
    }

    /// Finishes enumeration once the device has reported its EP0 max packet size.
    unsafe fn finish_max_packet_size_probe(
        self: &Arc<Self>,
        device: &Arc<IrqMutex<XhciDevice>>,
        slot_id: u8,
        status: UsbTransferStatus,
    ) {
        let port = device.lock().port;

        if status != UsbTransferStatus::Completed {
            kprintln!(
                Error,
                "[SLOT {}] [PORT {}] Reading the device descriptor for the max packet size failed.",
                slot_id,
                port
            );
            self.disable_slot(port);
            return;
        }

        let reported = device.lock().reported_max_packet_size_0();
        let port_speed = self.regs.portsc(port).ps_read();

        let Some(max_packet_size) = reported.and_then(|value| max_packet_size_0(port_speed, value))
        else {
            kprintln!(
                Warn,
                "[SLOT {}] Device reported an unusable EP0 max packet size, keeping the programmed one.",
                slot_id
            );
            self.register_with_usb_core(slot_id);
            return;
        };

        let input_context_phys = {
            let dev = device.lock();
            let input_context = &mut *(dev.input_context_dma.virt.as_mut_ptr::<InputContext>());
            let ep_0 = input_context.endpoint::<EndpointContext>(2, self.regs.context_size);
            let programmed = ep_0.get_max_packet_size();

            if programmed == max_packet_size {
                None
            } else {
                kprintln!(
                    Info,
                    "[SLOT {}] Correcting EP0 max packet size from {} to {}.",
                    slot_id,
                    programmed,
                    max_packet_size
                );
                ep_0.set_max_packet_size(max_packet_size);

                let control = input_context.control();
                control.reset_flags();
                control.add_context(1);

                Some(dev.input_context_dma.phys.as_u64())
            }
        };

        let Some(input_context_phys) = input_context_phys else {
            self.register_with_usb_core(slot_id);
            return;
        };

        let mut evaluate_context_trb = EvaluateContextCmdTrb::new();
        evaluate_context_trb.set_slot_id(slot_id);
        evaluate_context_trb.set_input_context_pointer(input_context_phys);

        if let Err(error) = self.send_command(*evaluate_context_trb.raw(), CommandContext::Empty) {
            kprintln!(
                Error,
                "[SLOT {}] Sending evaluate context command failed: {:?}",
                slot_id,
                error
            );
            self.disable_slot(port);
        }
    }

    /// Hands a fully addressed device over to layer 2.
    fn register_with_usb_core(self: &Arc<Self>, slot_id: u8) {
        let controller: Weak<dyn UsbHostController> = Arc::downgrade(self) as _;
        set_timeout(10, move || {
            usb_register_device(slot_id, controller);
        });
    }

    //==============================================================================
    //  INITIALIZATION
    //==============================================================================
    fn register_interrupt_handlers(&self) -> Result<(), PciDeviceInitError> {
        let vectors: [InterruptVector; 2] = match &self.interrupt_config {
            XhciInterruptConfig::Msix {
                command_vector,
                transfer_vector,
                ..
            } => [*command_vector, *transfer_vector],
            XhciInterruptConfig::Msi { vector, .. } => [*vector, *vector],
        };

        for (index, interrupter) in self.interrupters.iter().enumerate() {
            let context = Arc::into_raw(interrupter.clone()) as usize;

            if !register_handler_with_context(vectors[index], xhci_irq_handler, context) {
                unsafe { drop(Arc::from_raw(context as *const XhciInterrupter)) };
                return Err(InsufficientMsixVectors);
            }
        }

        Ok(())
    }

    fn cancel_request_for_device(&self, slot_id: u8, dci: u8) -> Result<(), RingError> {
        let mut stop_cmd = StopEndpointCommandTrb::new();
        stop_cmd.set_slot_id(slot_id);
        stop_cmd.set_endpoint_id(dci);
        stop_cmd.set_suspend(false);

        self.send_command(*stop_cmd.raw(), CommandContext::Empty)
            .map(|_| ())
    }
}

/// Turns the `bMaxPacketSize0` field of a device descriptor into a byte count.
/// SuperSpeed devices report the size as a power of two exponent, every other speed reports the size directly.
fn max_packet_size_0(port_speed: u8, reported: u8) -> Option<u32> {
    const SUPER_SPEED: u8 = 4;

    if port_speed == SUPER_SPEED {
        return (reported == 9).then_some(512);
    }

    matches!(reported, 8 | 16 | 32 | 64).then_some(reported as u32)
}

/// Applies a completion to a request and runs its callback with no driver lock held.
fn complete_request(
    request: Arc<IrqMutex<UsbTransferRequest>>,
    status: UsbTransferStatus,
    residual: usize,
) {
    let callback = {
        let mut req = request.lock();
        req.status = status;
        req.bytes_transferred = req.data_buffer_length.saturating_sub(residual);
        req.completion_callback
    };

    if let Some(callback) = callback {
        set_timeout(10, move || {
            callback(request);
        });
    }
}

//==================================================================================================
//  CONTROLLER REGISTRY
//==================================================================================================

static XHCI_CONTROLLERS: IrqMutex<Vec<Arc<XHCI>>> = IrqMutex::new(Vec::new());
static NEXT_XHCI_ID: AtomicUsize = AtomicUsize::new(0);

fn xhci_by_id(xhci_id: usize) -> Option<Arc<XHCI>> {
    XHCI_CONTROLLERS
        .lock()
        .iter()
        .find(|xhci| xhci.id == xhci_id)
        .cloned()
}

fn defer_with_xhci<F>(xhci_id: usize, delay_ms: u64, f: F)
where
    F: FnOnce(&Arc<XHCI>) + Send + 'static,
{
    set_timeout(delay_ms, move || {
        if let Some(xhci) = xhci_by_id(xhci_id) {
            f(&xhci);
        }
    });
}

//==================================================================================================
//  PORT SCHEDULING
//==================================================================================================
const USB2_DEBOUNCE_MS: u64 = 100;
const PORT_RESET_TIMEOUT_MS: u64 = 500;
const PORT_RESUME_TIMEOUT_MS: u64 = 20;
const PORT_RESET_MAX_ATTEMPTS: u8 = 3;
const PORT_RESUME_TO_REXIT_MS: u64 = 20;
const PORT_REXIT_POLL_MS: u64 = 2;

fn schedule_debounce(xhci: &XHCI, port: u8, generation: usize) {
    defer_with_xhci(xhci.id, USB2_DEBOUNCE_MS, move |xhci| {
        let mut ports = xhci.ports.lock();

        if !matches!(ports.state(port), PortState::Debounce { generation: g } if g == generation) {
            return;
        }

        let portsc = xhci.regs.portsc(port);

        if !portsc.ccs_read() {
            ports.set_state(port, PortState::Idle);
        } else if portsc.pls_read() == U3 {
            unsafe { xhci.resume_port(&mut ports, port) };
        } else {
            kprintln!(Debug, "[PORT {}] Debounce elapsed, resetting port.", port);
            ports.set_state(port, PortState::ResetInProgress { attempts: 1 });
            schedule_reset_timeout(xhci, port, 1);
            xhci.regs.portsc_issue_reset(port);
        }
    });
}

fn schedule_reset_timeout(xhci: &XHCI, port: u8, current_attempt: u8) {
    defer_with_xhci(xhci.id, PORT_RESET_TIMEOUT_MS, move |xhci| {
        let mut ports = xhci.ports.lock();

        //if a port completed the reset, it moved its state on
        let PortState::ResetInProgress { attempts } = ports.state(port) else {
            return;
        };

        if attempts != current_attempt {
            return;
        }

        let portsc = xhci.regs.portsc(port);

        if portsc.ccs_read() && attempts < PORT_RESET_MAX_ATTEMPTS {
            kprintln!(Warn, "Port {} reset timed out, retry {}", port, attempts + 1);
            ports.set_state(
                port,
                PortState::ResetInProgress {
                    attempts: attempts + 1,
                },
            );
            schedule_reset_timeout(xhci, port, attempts + 1);
            xhci.regs.portsc_issue_reset(port);
        } else {
            kprintln!(
                Warn,
                "Port {} reset timed out fatally. PORTSC={:#010x}",
                port,
                portsc.raw()
            );
            ports.set_state(port, PortState::Idle);
        }
    });
}

fn schedule_resume_to_rexit(xhci: &XHCI, port: u8) {
    defer_with_xhci(xhci.id, PORT_RESUME_TO_REXIT_MS, move |xhci| {
        let mut ports = xhci.ports.lock();

        if !matches!(
            ports.state(port),
            PortState::Resuming {
                phase: ResumePhase::Resume
            }
        ) {
            return;
        }

        xhci.regs.portsc_set_link_state(port, U0);
        kprintln!(Debug, "[PORT {}] Transitioning port to RExit state.", port);

        ports.set_state(
            port,
            PortState::Resuming {
                phase: ResumePhase::RExit,
            },
        );

        schedule_rexit_poll(xhci, port, timer_lapic_uptime_ms());
    });
}

fn schedule_rexit_poll(xhci: &XHCI, port: u8, rexit_start_ms: u64) {
    defer_with_xhci(xhci.id, PORT_REXIT_POLL_MS, move |xhci| {
        let mut ports = xhci.ports.lock();

        if !matches!(
            ports.state(port),
            PortState::Resuming {
                phase: ResumePhase::RExit
            }
        ) {
            kprintln!(
                Error,
                "[PORT {}] Invalid port state {:?} inside rexit poll scheduler.",
                port,
                ports.state(port)
            );
            return;
        }

        let portsc = xhci.regs.portsc(port);
        let now = timer_lapic_uptime_ms();

        if portsc.pls_read() == U0 {
            ports.set_state(port, PortState::ResetInProgress { attempts: 1 });
            schedule_reset_timeout(xhci, port, 1);
            xhci.regs.portsc_issue_reset(port);
        } else if now.wrapping_sub(rexit_start_ms) >= PORT_RESUME_TIMEOUT_MS {
            kprintln!(
                Warn,
                "Port {} stuck leaving resume (PLS={:?})",
                port,
                portsc.pls_read()
            );
            ports.set_state(port, PortState::Idle);
        } else {
            schedule_rexit_poll(xhci, port, rexit_start_ms);
        }
    });
}

fn xhci_init_port_states(xhci: &Arc<XHCI>) {
    let mut ports = xhci.ports.lock();

    for port in 1..=xhci.regs.max_ports as u8 {
        let portsc = xhci.regs.portsc(port);
        if !portsc.ccs_read() {
            continue;
        }

        let Some(port_info) = xhci.regs.port_info(port) else {
            continue;
        };

        if port_info.protocol == Usb2 {
            kprintln!(
                Info,
                "[XHCI STARTUP] Startup: device already present on port {} (PLS={:?})",
                port,
                portsc.pls_read()
            );

            xhci.regs.portsc_ack_changes(port, portsc);

            ports.set_state(port, PortState::Debounce { generation: 0 });
            schedule_debounce(xhci, port, 0);
        } else {
            // TODO: USB3
        }
    }
}

//==================================================================================================
//  DEVICE INITIALIZATION
//==================================================================================================

impl PciDeviceInitializer for XHCI {
    fn initialize(dev: PciDevice) -> Result<(), PciDeviceInitError> {
        let bar = PciBAR::get(&dev, 0);

        if bar.bar_type() == &BarType::Io {
            return Err(InvalidBarType);
        }

        dev.enable_pci_mmio_and_bus_mastering();

        unsafe {
            let iomap = bar.ioremap_checked();
            let cap_base = iomap.virt_addr;

            let cap_length = mmio_read::<u8>(cap_base, CAP_REG_CAPLENGTH as u64);
            let operational_base = cap_base.add(cap_length as u64);

            let runtime_offset = mmio_read::<u32>(cap_base, CAP_REG_RTSOFF as u64) as u64;
            let runtime_base =
                VirtAddr::new((cap_base.as_u64() + runtime_offset) & RUNTIME_BASE_ALIGNMENT_MASK);

            let hccparams1 = mmio_read::<u32>(cap_base, CAP_REG_HCCPARAMS1 as u64);
            let ext_cap_address = first_ext_cap_addr(cap_base, hccparams1);
            xhci_legacy_handoff(ext_cap_address);

            xhci_stop_controller(operational_base)?;
            xhci_reset_controller(operational_base)?;

            let hcsparams1 = mmio_read::<u32>(cap_base, CAP_REG_HCSPARAMS1 as u64);
            let hccparams1 = mmio_read::<u32>(cap_base, CAP_REG_HCCPARAMS1 as u64);

            //enable all slots
            let max_slots = hcsparams1 & MAX_SLOTS_MASK;
            let max_ports = ((hcsparams1 & MAX_PORTS_MASK) >> MAX_PORTS_SHIFT) as usize;

            let config_reg = mmio_read::<u32>(operational_base, OP_REG_CONFIG as u64);
            mmio_write::<u32>(
                operational_base,
                OP_REG_CONFIG as u64,
                config_reg | max_slots,
            );

            let context_size = if (hccparams1 & CONTEXT_SIZE_64_BYTE_FLAG) != 0 {
                CONTEXT_SIZE_64_BYTES
            } else {
                CONTEXT_SIZE_32_BYTES
            };

            let dcbaa_dma = dma_alloc_zeroed(size_of::<Dcbaa>(), PageSize::SIZE_4KB as usize)
                .expect("Failed to allocate dma for dcbaa.");
            kprintln!(Debug, "Allocated DMA for dcbaa at phys{:#011x}; virt: {:#011x} with align {}",
                dcbaa_dma.phys, dcbaa_dma.virt, dcbaa_dma.layout.align());

            let mut dcbaa = DcbaaOwner::new(dcbaa_dma);
            mmio_write::<u64>(
                operational_base,
                OP_REG_DCBAAP as u64,
                dcbaa.phys().as_u64(),
            );

            let scratchpad = xhci_alloc_scratchpad(cap_base, &mut dcbaa);

            let (command_ring_dma, command_ring_trbs) = TrbRing::dma_alloc(COMMAND_RING_TRBS)
                .expect("Failed to allocate DMA for command ring.");

            kprintln!(Debug, "Allocated DMA for command ring at phys{:#011x}; virt: {:#011x} with align {}",
                command_ring_dma.phys,
                command_ring_dma.virt,
                command_ring_dma.layout.align()
            );

            let command_ring = CommandRing::new(command_ring_trbs, command_ring_dma.phys);
            mmio_write::<u64>(
                operational_base,
                OP_REG_CRCR as u64,
                (command_ring_dma.phys.as_u64() & !COMMAND_RING_RESERVED_BITS)
                    | COMMAND_RING_CYCLE_STATE,
            );

            let interrupt_config = xhci_configure_interrupts(&dev)?;


            let primary = xhci_setup_interrupter(
                operational_base,
                runtime_base,
                PRIMARY_INTERRUPTER,
                XhciInterrupterKind::Primary,
                "primary",
            );

            //setup secondary interrupter only if msix is found
            let interrupters: Vec<Arc<XhciInterrupter>> = match interrupt_config {
                XhciInterruptConfig::Msix { .. } => Vec::from([
                    primary,
                    xhci_setup_interrupter(
                        operational_base,
                        runtime_base,
                        TRANSFER_INTERRUPTER,
                        XhciInterrupterKind::Transfer,
                        "transfer",
                    ),
                ]),
                XhciInterruptConfig::Msi { .. } => Vec::from([primary]),
            };

            let transfer_interrupter_target = if interrupters.len() > 1 {
                TRANSFER_INTERRUPTER as u16
            } else {
                PRIMARY_INTERRUPTER as u16
            };

            let ext_cap_address = first_ext_cap_addr(cap_base, hccparams1);

            //fix for intel panther point chipset - switch over usb2 ports to xhci
            if dev.vendor_id() == PciVendor::INTEL {
                dev.usb_intel_enable_xhci_ports();
            }
            let supported_protocols = parse_xhci_supported_protocols(ext_cap_address, max_ports);
            xhci_enable_usb3_port_power(operational_base, &supported_protocols);

            let doorbell_offset = mmio_read::<u32>(cap_base, CAP_REG_DBOFF as u64);

            let xhci_arc = Arc::new(XHCI {
                regs: XhciRegs {
                    cap_base,
                    operational_base,
                    runtime_base,
                    doorbell_offset,
                    context_size,
                    max_slots,
                    max_ports,
                    supported_protocols,
                },

                pci_device: dev,
                dcbaa: IrqMutex::new(dcbaa),
                commands: IrqMutex::new(CommandQueue::new(command_ring_dma, command_ring)),
                devices: IrqMutex::new(BTreeMap::new()),
                ports: IrqMutex::new(PortTable::new()),
                interrupters,
                interrupt_config,
                transfer_interrupter_target,
                _scratchpad: scratchpad,
                id: NEXT_XHCI_ID.fetch_add(1, Ordering::Relaxed),
            });

            for interrupter in &xhci_arc.interrupters {
                interrupter.set_controller(&xhci_arc);
            }

            XHCI_CONTROLLERS.lock().push(xhci_arc.clone());
            xhci_arc.register_interrupt_handlers()?;

            xhci_start_controller(operational_base)?;
            xhci_init_port_states(&xhci_arc);
        }
        Ok(())
    }
}


fn xhci_endpoint_address_to_dci(b_endpoint_address: u8) -> u8 {
    let endpoint_num = b_endpoint_address & 0x0F;
    if endpoint_num == 0 {
        return EP0_DCI;
    }
    let is_in = (b_endpoint_address & 0x80) != 0;
    let dci = (endpoint_num * 2) + if is_in { 1 } else { 0 };

    dci
}

impl UsbHostController for XHCI {
    fn submit_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str> {
        let (slot_id, dci, setup_packet, dma_buffer, buffer_length) = {
            let req = request.lock();

            let phys_dma_addr = match req.dma_buffer.as_ref() {
                None => None,
                Some(x) => Some(x.phys.as_u64())
            };

            (
                req.target_device.hardware_id,
                xhci_endpoint_address_to_dci(req.endpoint_address),
                req.setup_packet,
                phys_dma_addr,
                req.data_buffer_length,
            )
        };

        let dev = self
            .device(slot_id)
            .ok_or("Tried submitting a request to invalid hardware id device!")?;

        let interrupter_target = self.transfer_interrupter_target;

        let setup_packet = if dci == EP0_DCI {
            Some(
                setup_packet
                    .ok_or("Cannot issue request - ep0 specified and no setup packet provided.")?,
            )
        } else {
            None
        };

        {
            let issued = match setup_packet {
                Some(setup) => unsafe {

                    //for the SET_CONFIGURATION request, first we need to send a configure endpoint
                    //trb before we issue the request
                    if setup.b_request == 0x09 {
                        let port = {
                            dev.lock().port
                        };

                        {
                            self.ports.lock().state[port as usize] = PendingSetConfiguration;
                        }
                        kprintln!("Set port to pending set configuration.");

                        self.on_pending_set_configuration(request, &dev);
                        return Ok(());
                    }

                    let mut dev_lock = dev.lock();
                    dev_lock.ep0_issue_request(
                        interrupter_target,
                        setup.bm_request_type,
                        setup.b_request,
                        buffer_length,
                        setup.w_value,
                        setup.w_index,
                        dma_buffer,
                    )
                },
                None => unsafe {
                    let mut dev_lock = dev.lock();
                    dev_lock.issue_normal_transfer(
                        interrupter_target,
                        dci,
                        dma_buffer,
                        buffer_length
                    )
                },
            };

            let mut dev_lock = dev.lock();
            let trb_phys = issued.map_err(|_| "Failed to enqueue the transfer TRBs.")?;
            dev_lock.insert_pending(trb_phys, request.clone());
        }

        self.regs.ring_doorbell(slot_id, dci, 0);

        Ok(())
    }

    fn cancel_request(&self, request: Arc<IrqMutex<UsbTransferRequest>>) -> Result<(), &'static str> {
        //TODO: implement this with all the stuff inside command completion event
        Err("not implemented yet")
        // let req = request.lock();
        // let slot_id = req.target_device.hardware_id;
        // self.cancel_request_for_device(slot_id, enpoint_address_to_dci(req.endpoint_address));
        // self.ring_global_doorbell();
    }
}

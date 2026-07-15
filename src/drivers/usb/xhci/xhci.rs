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
use crate::drivers::apic::apic::{LAPIC, timer_lapic_uptime_ms};
use crate::drivers::pci::pci_bar::{BarType, PciBAR};
use crate::drivers::pci::pci_device::PciDeviceInitError::{
    InvalidBarType, XhciControllerNotReadyTimeout,
    XhciControllerResetTimeout, XhciControllerStartTimeout, XhciControllerStopTimeout,
    InsufficientMsixVectors, XhciMsiCapabilityNotFound
};
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_io::{PciVendor};
use crate::drivers::pci::*;
use crate::drivers::usb::xhci::xhci_endpoint_context::*;
use crate::drivers::usb::xhci::xhci_ext_cap::{
    XhciPortInfo, XhciPortProtocol, parse_xhci_supported_protocols,
};
use crate::drivers::usb::xhci::xhci_portsc::{PortLinkState, PortStatusControl};
use crate::drivers::usb::xhci::xhci_slot_context::*;
use crate::drivers::usb::xhci::xhci_trb::*;
use crate::drivers::usb::xhci::*;
use crate::interrupts::router::register_handler_with_context;
use crate::interrupts::vector::InterruptVector;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::mem::{size_of};
use core::ops::Add;
use core::ptr;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, Ordering};
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::idt::InterruptStackFrame;
use crate::drivers::pci::pci_msi::{MsiCapability, MsixCapability, MsixPBA};
use crate::drivers::usb::xhci::xhci_ext_cap::XhciPortProtocol::Usb2;
use crate::drivers::usb::xhci::xhci_input_context::InputContext;
use crate::drivers::usb::xhci::xhci_portsc::PortLinkState::{U0, U3};
use crate::drivers::usb::xhci::xhci_trb_ring::{CommandContext, CommandRing, EventRing, Ring, RingError, ShadowRing, TransferRing, TrbRing};
use crate::{kprintln};
use crate::memory::dir_mapping::physical_to_virtual;
use crate::memory::page_tables::PageSize;

const PCI_STATUS_REGISTER: u32 = 0x06;
const PCI_STATUS_CAPABILITIES_LIST: u16 = 1 << 4;
const PCI_CAPABILITY_POINTER_REGISTER: u32 = 0x34;
const PCI_CAPABILITY_ID_MSIX: u8 = 0x11;
const PCI_CAPABILITY_ID_MSI: u8 = 0x05;
const PCI_CAPABILITY_NEXT_POINTER_OFFSET: u32 = 0x01;
const PCI_MSIX_MESSAGE_CONTROL_OFFSET: u32 = 0x02;
const PCI_MSIX_TABLE_OFFSET: u32 = 0x04;
const PCI_MSIX_PBA_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_CONTROL_OFFSET: u32 = 0x02;
const PCI_MSI_MESSAGE_ADDRESS_LOW_OFFSET: u32 = 0x04;
const PCI_MSI_MESSAGE_ADDRESS_HIGH_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_DATA_32_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_DATA_64_OFFSET: u32 = 0x0C;
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
const LAPIC_MSI_ADDR: u64 = 0xFEE0_0000;
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

const MAX_XHCI_CONTROLLERS: usize = 4;
pub static XHCI_TICK_LIST: [AtomicPtr<XHCI>; MAX_XHCI_CONTROLLERS] =
    [const { AtomicPtr::new(null_mut()) }; MAX_XHCI_CONTROLLERS];

#[derive(Clone, Copy)]
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

unsafe fn stop_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
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

unsafe fn reset_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
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

            let legsup = mmio_read::<u32>(cap_addr, 0);
            let legctlsts = mmio_read::<u32>(cap_addr, XHCI_LEGACY_CTLSTS_OFFSET);
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

fn enable_usb3_port_power(operational_base: VirtAddr, supported_protocols: &[XhciPortInfo]) {
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

fn debug_print_supported_protocols(supported_protocols: &[XhciPortInfo]) {
    for port_info in supported_protocols {
        if port_info.protocol == XhciPortProtocol::Unknown {
            kprintln!(Debug, "  port {}: unknown", port_info.port_id);
            continue;
        }

        match port_info.raw_bps {
            Some(raw_bps) => {
                kprintln!(Debug,
                    "  port {}: {} {}.{} {:?} psiv {} {} slot_type {} proto {:#x}",
                    port_info.port_id,
                    port_info.protocol,
                    port_info.major,
                    port_info.minor,
                    port_info.speed,
                    port_info.psiv,
                    raw_bps,
                    port_info.slot_type,
                    port_info.protocol_defined
                );
            }
            None => {
                kprintln!(Debug,
                "  port {}: {} {}.{} {:?} psiv {} unknown slot_type {} proto {:#x}",
                port_info.port_id,
                port_info.protocol,
                port_info.major,
                port_info.minor,
                port_info.speed,
                port_info.psiv,
                port_info.slot_type,
                port_info.protocol_defined
            );
            }
        }
    }
}

fn debug_print_usb3_portsc(operational_base: VirtAddr, supported_protocols: &[XhciPortInfo]) {
    for port_info in supported_protocols {
        if port_info.protocol == Usb2 || port_info.port_id == 0 {
            continue;
        }

        let portsc = PortStatusControl::from_port(operational_base, port_info.port_id);
        // kprintln!(Debug,
        //     "USB3 port {} PORTSC raw={:#010x} pp={} ccs={} ped={} pls={:?} ps={} cas={} chg[csc={} pec={} wrc={} prc={} plc={} cec={}]",
        //     port_info.port_id,
        //     portsc.raw(),
        //     portsc.pp_read(),
        //     portsc.ccs_read(),
        //     portsc.ped_read(),
        //     portsc.pls_read(),
        //     portsc.ps_read(),
        //     portsc.cas_read(),
        //     portsc.csc_read(),
        //     portsc.pec_read(),
        //     portsc.wrc_read(),
        //     portsc.prc_read(),
        //     portsc.plc_read(),
        //     portsc.cec_read()
        // );
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
    rsvdz: u32,
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

struct XHCIDevice {
    input_context_dma: DmaAlloc,
    transfer_ring_dma: DmaAlloc,
    transfer_ring: TransferRing,
    device_context_dma: DmaAlloc,
}

impl XHCIDevice {
    fn new(input_context_dma: DmaAlloc,
           transfer_ring_dma: DmaAlloc,
           transfer_ring: TransferRing,
           device_context_dma: DmaAlloc) -> Self
    {
        XHCIDevice {
            input_context_dma,
            transfer_ring_dma,
            transfer_ring,
            device_context_dma,
        }
    }
}

pub struct XhciInterrupterState {
    controller: *mut XHCI,
    runtime_base: VirtAddr,
    interrupter_offset: u64,
    event_ring: VirtAddr,
    kind: XhciInterrupterKind,
    name: &'static str,
}

impl XhciInterrupterState {
    pub const fn new(
        runtime_base: VirtAddr,
        interrupter_offset: u64,
        event_ring: VirtAddr,
        kind: XhciInterrupterKind,
        name: &'static str,
    ) -> Self {
        Self {
            controller: null_mut(),
            runtime_base,
            interrupter_offset,
            event_ring,
            kind,
            name,
        }
    }

    pub fn set_controller(&mut self, xhci: *mut XHCI) {
        self.controller = xhci;
    }

    fn ack(&self, dequeue_phys: u64) {
        unsafe {
            mmio_write::<u64>(
                self.runtime_base,
                RT_ERDP as u64 + self.interrupter_offset,
                (dequeue_phys & ERDP_PTR_MASK) | ERDP_EHB,
            );
        }
    }

    fn debug_print_first_event_trb(&self) {
        if self.event_ring.as_u64() == 0 {
            return;
        }

        unsafe {
            let trb_ptr = self.event_ring.as_u64() as *const u8;
            let parameter = ptr::read_volatile(trb_ptr.add(0) as *const u64);
            let status = ptr::read_volatile(trb_ptr.add(8) as *const u32);
            let control = ptr::read_volatile(trb_ptr.add(12) as *const u32);
            let trb_type = (control >> 10) & 0x3f;
            let cycle = control & 1;

            kprintln!(Debug,
                "xHCI {} event TRB: type={} cycle={} param={:#018x} status={:#010x} control={:#010x} runtime_base={:#011x}",
                self.name,
                trb_type,
                cycle,
                parameter,
                status,
                control,
                self.runtime_base.as_u64()
            );
        }
    }

    unsafe fn handle_port_reset(&self, xhci: &mut XHCI, port: u8) {
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);

        if portsc.ped_read() && !portsc.pr_read() && portsc.pls_read() == U0 {
            kprintln!(Debug, "[PORT {}] Succesfully reset port.", port); //TODO: usb3
            xhci.port_state[port as usize] = PortState::Enabled;
        } else {
            if let PortState::ResetInProgress { attempts, .. } = xhci.port_state[port as usize] {
                if attempts < PORT_RESET_MAX_ATTEMPTS && portsc.ccs_read() {
                    kprintln!(Warn, "[PORT {}] Port reset was incomplete, retry {}.", port, attempts + 1);
                    portsc_issue_reset(xhci, port);
                    xhci.port_state[port as usize] = PortState::ResetInProgress {
                        started_ms: timer_lapic_uptime_ms(),
                        attempts: attempts + 1,
                    };
                    return;
                }
            }
            kprintln!(Warn, "[PORT {}] Port reset timed out, PORTSC={:#010x} (PED={} PLS={:?} PRC={} CCS={})",
                port, portsc.raw(), portsc.ped_read(), portsc.pls_read(),
                portsc.prc_read(), portsc.ccs_read());
            xhci.port_state[port as usize] = PortState::Idle;
            return;
        }

        //we did reset the slot, so now we're in enabled state
        //TODO: usb3 logic (but it's probably the same, prioritizing usb2 for keyboard support)
        kprintln!(Debug, "[PORT {}] Sending enable slot command for port.", port);

        let index = xhci.command_ring.get_mut().enqueue_index();
        let slot_type = 0; //99,9999999999% cases its just zero
        let trb = EnableSlotCommandTrb::new_command(slot_type);
        xhci.send_command(*trb.raw()).expect("Sending enable slot command for XHCI failed.");

        xhci.shadow_ring.save_context(index, CommandContext::EnableSlot {
            port_id: port,
        });

        let slot = &(*xhci.command_ring.get_mut().trbs())[index] as *const Trb as *const u32;
        kprintln!(Debug, "CMD TRB[{}] @phys {:#x}: {:#010x} {:#010x} {:#010x} {:#010x}",
            index,
            xhci.command_ring_dma.phys.as_u64() + index as u64 * 16,
            ptr::read_volatile(slot), ptr::read_volatile(slot.add(1)),
            ptr::read_volatile(slot.add(2)), ptr::read_volatile(slot.add(3)));

        let crcr = mmio_read::<u64>(xhci.operational_base, OP_REG_CRCR as u64);
        kprintln!(Debug, "CRCR: CRR={}", (crcr >> 3) & 1);

        xhci.ring_global_doorbell();
    }

    unsafe fn handle_port_status_change(&self, xhci: &mut XHCI, trb: PortStatusChangeEventTrb) {
        let port = trb.port_id();

        if !xhci.is_valid_port(port) {
            kprintln!(Info,"[INVALID PORT {}] Ignoring port status change for this invalid port.", port);
            return;
        }
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);

        kprintln!(Debug,
            "[PORT {}] PSC event for port: PORTSC={:#010x} [CSC={} PEC={} PRC={} PLC={} WRC={} OCC={}] CCS={} PED={} PLS={:?} state={:?}",
            port, portsc.raw(),
            portsc.csc_read(), portsc.pec_read(), portsc.prc_read(),
            portsc.plc_read(), portsc.wrc_read(), portsc.occ_read(),
            portsc.ccs_read(), portsc.ped_read(), portsc.pls_read(),
            xhci.port_state[port as usize]
        );

        portsc_ack_changes(xhci, port, portsc);

        kprintln!(Debug, "[PORT {}] Cleared port status change event.", port);

        let idx = port as usize;
        if portsc.prc_read() {
            if matches!(xhci.port_state[idx], PortState::ResetInProgress { .. }) {
                self.handle_port_reset(xhci, port);
            } else {
                kprintln!(Warn, "[PORT {}] Unexpected PRC in state {:?}.", port, xhci.port_state[idx]);
            }
        }

        let csc = portsc.csc_read();
        let ccs = portsc.ccs_read();

        if portsc.csc_read() {
            if matches!(xhci.port_state[idx], PortState::Enabled) {
                kprintln!(Warn, "[PORT {}] Device on addressed port disconnected/bounced.", port);

            }
            if portsc.ccs_read() {
                xhci.port_state[idx] = PortState::Debounce { stable_since_ms: timer_lapic_uptime_ms() };

                if let Some(port_info) = xhci.port_info(port) {
                    kprintln!(Info,"[PORT {}] Attach detected with {} protocol.", port, port_info.protocol);
                } else {
                    kprintln!(Info,"[PORT {}] Attach detected with unknown protocol.", port);
                }
            } else {
                if let Some(port_info) = xhci.port_info(port) {
                    kprintln!(Info,"[PORT {}] Detach detected with {} protocol.", port, port_info.protocol);
                } else {
                    kprintln!(Info,"[PORT {}] Detach detected with unknown protocol.", port);
                }

                xhci.port_state[idx] = PortState::Idle;
            }
        }
    }

    unsafe fn handle_enable_slot_completion(&self, xhci: &mut XHCI, trb: CommandCompletionEventTrb, command_context: CommandContext) {
        let slot_id = trb.slot_id();
        let port = match command_context {
            CommandContext::EnableSlot { port_id } => { port_id },
            _ => { panic!("[SLOT {}] Invalid command context type! Expected enable slot, found {:?}.", slot_id, command_context) }
        };

        if trb.completion_code() == TrbCompletionCode::SUCCESS && slot_id != 0 {
            kprintln!(Info, "[SLOT {}][PORT {}] Successfully assigned slot {}.", trb.slot_id(), port, trb.slot_id());
        } else {
            //TODO: handle these so that the slot allocation doesnt go to waste
            kprintln!(Warn, "[SLOT {}][PORT {}] Handling enable slot command resulted in non successful exit code {}.", slot_id, port, trb.completion_code());
            return;
        }

        kprintln!(Debug, "[SLOT {}][PORT {}] Began allocating data structures for device.", slot_id, port);
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);
        let port_speed = portsc.ps_read();

        //now that we have a slot, we can initialize all the required data structures
        let input_context_dma = dma_alloc_zeroed(size_of::<InputContext>(), 4096). //TODO: remove, can cause deadlocks
            expect("Failed to dma alloc for input context!");
        let input_context = &mut *(input_context_dma.virt.as_mut_ptr::<InputContext>());
        let ic_control = input_context.control();

        ic_control.add_context(0);
        ic_control.add_context(1);

        let input_slot_context =
            input_context.slot::<SlotContext>(xhci.context_size);

        input_slot_context.set_root_hub_port(port);
        input_slot_context.set_route_string(0); // here we just handle devices directly connected to root hub, so that's just zero
        input_slot_context.set_context_entries(1);
        input_slot_context.set_speed(port_speed);

        let max_packet_size = match port_speed {
            1 => 8,
            2 => 8,
            3 => 64,
            4 => 512,
            _ => 8,   //fallback
        };

        // input slot context initialized, now the transfer ring
        let alloc_transfer_ring = TrbRing::dma_alloc(TRANSFER_RING_TRBS); //TODO: remove, can cause deadlocks
        if alloc_transfer_ring.is_none() {
            panic!("[SLOT {}][PORT {}] Failed to allocate transfer ring for device.", slot_id, port);
        }

        let (transfer_ring_dma, transfer_trbs) = alloc_transfer_ring.unwrap();
        let transfer_ring = TransferRing::new(transfer_trbs, transfer_ring_dma.phys);

        // endpoint context
        //ici=2 is the index of ep0
        let ep_0 = input_context.endpoint::<EndpointContext>(2, xhci.context_size);
        ep_0.set_ep_type(EndpointContext::EP_TYPE_CONTROL);
        ep_0.set_max_packet_size(max_packet_size);
        ep_0.set_max_burst(0);
        ep_0.set_tr_dequeue_ptr(transfer_ring_dma.phys.as_u64());
        ep_0.set_dequeue_cycle_state(true);
        ep_0.set_interval(0);
        ep_0.set_max_pstreams(0);
        ep_0.set_mult(0);
        ep_0.set_error_count(3);

        // output device context
        let device_context_dma = dma_alloc_zeroed(size_of::<DeviceContext>(), 4096). //TODO: remove, can cause deadlocks
            expect("Failed to allocate dma for device context.");
        let device_context = &*device_context_dma.virt.as_mut_ptr::<DeviceContext>();

        xhci.dcbaa.set_context(slot_id as usize, device_context_dma.phys.as_u64());

        let enqueue_index = xhci.command_ring.get_mut().enqueue_index();
        xhci.shadow_ring.save_context(enqueue_index,
            CommandContext::AddressDevice { slot_id }
        );

        kprintln!(Debug, "[SLOT {}][PORT {}] Succesfully allocated required data structures for device.", slot_id, port);

        let mut address_command_trb = AddressDeviceCommandTrb::new();
        address_command_trb.set_slot_id(slot_id);
        address_command_trb.set_input_context_pointer(input_context_dma.phys.as_u64());
        address_command_trb.set_bsr(false);

        let xhci_device = XHCIDevice::new(
            input_context_dma,
            transfer_ring_dma,
            transfer_ring,
            device_context_dma
        );

        xhci.devices.get_mut()[slot_id as usize] = Some(xhci_device);

        xhci.send_command(*address_command_trb.raw()).
            expect("Failed to send address device command!");
        xhci.ring_global_doorbell();
    }

    unsafe fn handle_address_device_completion(&self, xhci: &mut XHCI, trb: CommandCompletionEventTrb, command_context: CommandContext) {
        let command_trb_addr = physical_to_virtual(PhysAddr::new(trb.command_trb_pointer()));
        let command_trb = &*(command_trb_addr.as_mut_ptr::<AddressDeviceCommandTrb>());
        let slot_id = match command_context {
            CommandContext::AddressDevice { slot_id } => { slot_id }
            a => { panic!("Invalid context type! Expected AddressDevice found {:?}.", a) }
        };

        if slot_id == 0 {
            panic!("[SLOT 0] 0 is not a valid slot ID inside command context!");
        }

        if trb.completion_code() == TrbCompletionCode::CONTEXT_STATE_ERROR {
            kprintln!(Error, "[SLOT {}] Not in enabled state!", trb.slot_id());
            return; //TODO
        } else if trb.completion_code() == TrbCompletionCode::USB_TRANSACTION_ERROR {
            kprintln!(Error, "[SLOT {}] SET_ADDRESS request was not successful.", trb.slot_id());
            return;
        } else if trb.completion_code() == TrbCompletionCode::SLOT_NOT_ENABLED_ERROR {
            kprintln!(Error, "[SLOT {}] Was not enabled by enable slot command.", trb.slot_id());
            return;
        } else if trb.completion_code() != TrbCompletionCode::SUCCESS {
            kprintln!(Error, "[SLOT {}] Address Device Command FAILED with code: {:?}", slot_id, trb.completion_code());
            return;
        }

        let input_ptr = physical_to_virtual(PhysAddr::new(command_trb.input_context_pointer()));

        let bsr = command_trb.bsr();
        let input_context = &*(input_ptr.as_ptr::<InputContext>());

        let output_device_context = &*(xhci.dcbaa.get_context_virt(slot_id as usize).as_ptr::<DeviceContext>());
        let input_slot_context = input_context.slot_ref::<SlotContext>(xhci.context_size);
        let output_slot_context = output_device_context.slot();

        let input_ep0 = input_context.endpoint_ref::<EndpointContext>(2, xhci.context_size);
        let output_ep0 = output_device_context.endpoint(1, xhci.context_size);

        //stinky packed struct references!
        let out_dword0 = output_slot_context.dword0;
        let out_dword1 = output_slot_context.dword1;
        let in_dword0 = input_slot_context.dword0;
        let in_dword1 = input_slot_context.dword1;

        let out_ep0_dword0 = output_ep0.dword0;
        let out_ep0_dword1 = output_ep0.dword1;
        let out_ep0_dword2 = output_ep0.dword2;
        let out_ep0_dword3 = output_ep0.dword3;

        let in_ep0_dword0 = input_ep0.dword0;
        let in_ep0_dword1 = input_ep0.dword1;
        let in_ep0_dword2 = input_ep0.dword2;
        let in_ep0_dword3 = input_ep0.dword3;

        //TODO: replace these stupid asserts with some creative error messages

        //compare output and input contexts
        assert_eq!(out_dword0 & 0xF800_0000, in_dword0 & 0xF800_0000);
        assert_eq!(out_dword1 & 0x00FF_0000, in_dword1 & 0x00FF_0000);

        //compare input.output ep0 contexts
        assert_eq!(out_ep0_dword1, in_ep0_dword1);
        assert_eq!(out_ep0_dword2, in_ep0_dword2);
        assert_eq!(out_ep0_dword3, in_ep0_dword3);

        assert_eq!(output_ep0.get_ep_state(), EndpointState::RUNNING);

        if bsr == false {
            assert_eq!(output_slot_context.get_slot_state(), SlotState::ADDRESSED);
            assert!(output_slot_context.get_usb_address() > 0);
            kprintln!(Info, "[SLOT {}] SET_ADDRESS request completed successfully.", trb.slot_id());
        } else {
            assert_eq!(output_slot_context.get_slot_state(), SlotState::DEFAULT);
            assert_eq!(output_slot_context.get_usb_address(), 0);
        }

        kprintln!(Info, "[SLOT {}] Successfully handled address device command.", trb.slot_id());
    }

    unsafe fn handle_command_completion(&self, xhci: &mut XHCI, trb: CommandCompletionEventTrb) {
        let command_trb_pointer = PhysAddr::new(trb.command_trb_pointer());
        let virt = physical_to_virtual(command_trb_pointer);
        let command_trb = &*(virt.as_u64() as *const Trb);
        let context = xhci.shadow_ring.take_context_by_phys_addr(command_trb_pointer);

        kprintln!(Debug,
            "[SLOT {}] Command completion event for port: [COMMAND_TYPE={}, COMPLETION_CODE={}]",
            trb.slot_id(), command_trb.trb_type(), trb.completion_code()
        );

        if !command_trb.is_command_trb() {
            panic!("Invalid trb type on command ring! This is probably caused by reading some garbage data.");
        }

        match command_trb.trb_type() {
            Trb::TRB_ENABLE_SLOT_COMMAND => {
                self.handle_enable_slot_completion(xhci, trb, context);
            },
            Trb::TRB_ADDRESS_DEVICE => {
                self.handle_address_device_completion(xhci, trb, context);
            },
            0_u8..=8_u8 | 10_u8 | 12_u8..=u8::MAX => todo!()
        }
    }

    unsafe fn handle(&self) {
        let xhci1 = self.controller;

        if xhci1.is_null() {
            panic!("Received IRQ for XHCI but XHCI is null!");
        }

        let xhci = &mut *xhci1;
        let event_ring = match self.kind {
            XhciInterrupterKind::Primary => unsafe { &mut *xhci.event_ring_primary.get() },
            XhciInterrupterKind::Transfer => unsafe { &mut *xhci.event_ring_secondary.get() },
        };

        let usbcmd = ptr::read_volatile(xhci.operational_base.as_ptr::<u32>());          // offset 0x00
        let usbsts = ptr::read_volatile(xhci.operational_base.add(0x04).as_ptr::<u32>());
        kprintln!(Debug, "USBCMD={:#x} (R/S={}) USBSTS={:#x} (HCH={} HSE={} CNR={})",
            usbcmd, usbcmd & 1,
            usbsts, usbsts & 1, (usbsts >> 2) & 1, (usbsts >> 11) & 1);

        while let Ok(trb) = event_ring.dequeue() {
            let trb_type = trb.trb_type();

            if trb_type == Trb::TRB_PORT_STATUS_CHANGE_EVENT {
                let event_change = trb
                    .try_as_port_status_change_event()
                    .expect("Cannot parse change event TRB!");
                self.handle_port_status_change(xhci, event_change);
            } else if trb_type == Trb::TRB_COMMAND_COMPLETION_EVENT {
                let command_completion = trb
                    .try_as_command_completion_event()
                    .expect("Cannot parse completion event TRB!");
                self.handle_command_completion(xhci, command_completion);
            } else {
                kprintln!(Debug, "Trb type {} received.", trb_type);
            }
        }

        let dequeue_phys = event_ring.get_dequeue_phys().unwrap(); //always returns some
        self.ack(dequeue_phys.as_u64());
    }
}

fn xhci_irq_handler(_: InterruptVector, _: InterruptStackFrame, context: usize) {
    if context == 0 {
        kprintln!(Warn,"xHCI IRQ without interrupter context");
        unsafe {
            if let Some(lapic) = LAPIC.get() {
                lapic.eoi();
            }
        }
        return;
    }

    unsafe {
        let interrupter = &*(context as *const XhciInterrupterState);
        interrupter.handle();
        LAPIC.get().unwrap().eoi()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ResumePhase {
    Resume,
    RExit,
}

#[derive(Copy, Clone, Debug)]
enum PortState {
    Idle,
    Debounce { stable_since_ms: u64 },
    ResetInProgress { started_ms: u64, attempts: u8 },
    Enabled,
    Resuming { started_ms: u64, phase: ResumePhase },
}

pub struct XHCI {
    pub pci_device: PciDevice,
    operational_base: VirtAddr,
    cap_base: VirtAddr,
    slots: u32,
    context_size: u32,
    dcbaa_dma: DmaAlloc,
    dcbaa: &'static mut Dcbaa,
    command_ring_dma: DmaAlloc,
    command_ring: UnsafeCell<CommandRing>,
    event_ring_primary_dma: DmaAlloc,
    event_ring_primary: UnsafeCell<EventRing>,
    event_ring_secondary_dma: DmaAlloc,
    event_ring_secondary: UnsafeCell<EventRing>,
    erst_primary_dma: DmaAlloc,
    erst_primary: &'static mut ERST,
    erst_secondary_dma: DmaAlloc,
    erst_secondary: &'static mut ERST,
    primary_interrupter: XhciInterrupterState,
    transfer_interrupter: XhciInterrupterState,
    interrupt_config: XhciInterruptConfig,
    supported_protocols: Vec<XhciPortInfo>,
    doorbell_offset: u32,
    shadow_ring: ShadowRing<COMMAND_RING_TRBS>,
    devices: UnsafeCell<[Option<XHCIDevice>; 256]>,
    port_state: [PortState; 256],
    max_ports: usize,
    scratchpad_array_dma: Option<DmaAlloc>,
    scratchpad_entry_dma: Option<Vec<DmaAlloc>>
}

impl XHCI {
    fn is_valid_port(&self, port: u8) -> bool {
        port != 0 && (port as usize) <= self.supported_protocols.len()
    }

    fn port_info(&self, port: u8) -> Option<XhciPortInfo> {
        if !self.is_valid_port(port) {
            return None;
        }

        self.supported_protocols.get((port - 1) as usize).copied()
    }

    #[inline(always)]
    fn send_command(&mut self, trb: Trb) -> Result<(), RingError> {
        self.command_ring.get_mut().enqueue(trb)
    }

    #[inline(always)]
    fn ring_global_doorbell(&self, ) {
        unsafe {
            mmio_write::<u32>(self.cap_base, self.doorbell_offset as u64, 0);
        }
    }

    fn new(
        pci_device: PciDevice,
        operational_base: VirtAddr,
        cap_base: VirtAddr,
        slots: u32,
        context_size: u32,
        dcbaa_dma: DmaAlloc,
        dcbaa: &'static mut Dcbaa,
        command_ring_dma: DmaAlloc,
        command_ring: CommandRing,
        event_ring_primary_dma: DmaAlloc,
        event_ring_primary: EventRing,
        event_ring_secondary_dma: DmaAlloc,
        event_ring_secondary: EventRing,
        erst_primary_dma: DmaAlloc,
        erst_primary: &'static mut ERST,
        erst_secondary_dma: DmaAlloc,
        erst_secondary: &'static mut ERST,
        primary_interrupter: XhciInterrupterState,
        transfer_interrupter: XhciInterrupterState,
        interrupt_config: XhciInterruptConfig,
        supported_protocols: Vec<XhciPortInfo>,
        doorbel_offset: u32,
        shadow_ring: ShadowRing<COMMAND_RING_TRBS>,
        max_ports: usize,
        scratchpad_array_dma: Option<DmaAlloc>,
        scratchpad_entry_dma: Option<Vec<DmaAlloc>>
    ) -> Self {
        XHCI {
            pci_device,
            operational_base,
            cap_base,
            slots,
            context_size,
            dcbaa_dma,
            dcbaa,
            command_ring_dma,
            command_ring: UnsafeCell::new(command_ring),
            event_ring_primary_dma,
            event_ring_primary: UnsafeCell::new(event_ring_primary),
            event_ring_secondary_dma,
            event_ring_secondary: UnsafeCell::new(event_ring_secondary),
            erst_primary_dma,
            erst_primary,
            erst_secondary_dma,
            erst_secondary,
            primary_interrupter,
            transfer_interrupter,
            interrupt_config,
            supported_protocols,
            doorbell_offset: doorbel_offset,
            shadow_ring,
            devices: UnsafeCell::new([const { None }; 256]),
            port_state: [PortState::Debounce { stable_since_ms: 0 }; 256],
            max_ports,
            scratchpad_array_dma,
            scratchpad_entry_dma
        }
    }

    fn register_interrupt_handlers(&self) -> Result<(), PciDeviceInitError> {
        match &self.interrupt_config {
            XhciInterruptConfig::Msix {
                command_vector,
                transfer_vector,
                ..
            } => {
                if !register_handler_with_context(
                    *command_vector,
                    xhci_irq_handler,
                    &self.primary_interrupter as *const _ as usize,
                ) {
                    return Err(InsufficientMsixVectors);
                }

                if !register_handler_with_context(
                    *transfer_vector,
                    xhci_irq_handler,
                    &self.transfer_interrupter as *const _ as usize,
                ) {
                    return Err(InsufficientMsixVectors);
                }
            }
            XhciInterruptConfig::Msi { vector, .. } => {
                if !register_handler_with_context(
                    *vector,
                    xhci_irq_handler,
                    &self.primary_interrupter as *const _ as usize,
                ) {
                    return Err(InsufficientMsixVectors);
                }
            }
        }

        Ok(())
    }

    unsafe fn resume_port(&mut self, port: u8, now: u64) {
        let portsc = PortStatusControl::from_port(self.operational_base, port);
        let mut w = PortStatusControl::write_from_raw(portsc.raw());
        w.pls_write(PortLinkState::Resume);
        w.write_to_port(self.operational_base, port);
        kprintln!("Transitioning port {} to Resume state.", port);
        self.port_state[port as usize] = PortState::Resuming {
            started_ms: now,
            phase: ResumePhase::Resume,
        };
    }

    fn bind_interrupters_to_controller(&mut self) {
        let xhci = self as *mut XHCI;
        self.primary_interrupter.set_controller(xhci);
        self.transfer_interrupter.set_controller(xhci);
    }
}

const USB2_DEBOUNCE_MS: u64 = 100;
const PORT_RESET_TIMEOUT_MS: u64 = 500;
const PORT_RESUME_TIMEOUT_MS: u64 = 20;
const PORT_RESET_MAX_ATTEMPTS: u8 = 3;

fn xhci_register_for_ticks(xhci: *mut XHCI) {
    for slot in XHCI_TICK_LIST.iter() {
        if slot.compare_exchange(null_mut(), xhci,
                                 Ordering::AcqRel, Ordering::Acquire).is_ok() {
            return;
        }
    }
    panic!("Too many xHCI controllers");
}

pub unsafe fn xhci_timer_tick(xhci: &mut XHCI) {
    let now = timer_lapic_uptime_ms();

    // let usbcmd = ptr::read_volatile(xhci.operational_base.as_ptr::<u32>());          // offset 0x00
    // let usbsts = ptr::read_volatile(xhci.operational_base.add(0x04).as_ptr::<u32>());

    // let hch = usbsts & 1;
    // let hse = (usbsts >> 2) & 1;
    // let cnr = (usbsts >> 11) & 1;
    //
    // if hch == 1 || hse == 1 || cnr == 1 {
    //     panic!("XHCI critical error! All these USBSTS flags should be 0!\nUSBCMD={:#x} (R/S={}) USBSTS={:#x} (HCH={} HSE={} CNR={})",
    //             usbcmd, usbcmd & 1,
    //             usbsts, usbsts & 1, (usbsts >> 2) & 1, (usbsts >> 11) & 1);
    // }

    // let crcr = mmio_read::<u64>(xhci.operational_base, OP_REG_CRCR as u64);
    // let crr = (crcr >> 3) & 1;
    // if crr != 1 && (hch == 1 || hse == 1 || cnr == 1) {
    //     panic!("CRCR: CRR={} is not 1!", crr);
    // }


    for port in 1..=xhci.max_ports {
        let idx = port;
        // kprint!("Port {} is in state {:?}, ", port, xhci.port_state[idx]);
        match xhci.port_state[idx] {
            PortState::Idle  => {
                let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                // kprint!("pls={:?}\n", portsc.pls_read());
            },

            PortState::Enabled  => {
                let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                // kprint!("pls={:?}\n", portsc.pls_read());
            },

            PortState::Debounce { stable_since_ms }
            if now.wrapping_sub(stable_since_ms) >= USB2_DEBOUNCE_MS =>
                {
                    let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                    // kprint!("pls={:?}\n", portsc.pls_read());
                    if !portsc.ccs_read() {
                        xhci.port_state[idx] = PortState::Idle;
                    } else if portsc.pls_read() == U3 {
                        xhci.resume_port(port as u8, now);
                    } else {
                        portsc_issue_reset(xhci, port as u8);
                        xhci.port_state[idx] = PortState::ResetInProgress { started_ms: now, attempts: 1 };
                    }
                }

            PortState::ResetInProgress { started_ms, attempts }
            if now.wrapping_sub(started_ms) >= PORT_RESET_TIMEOUT_MS =>
                {
                    let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                    // kprint!("pls={:?}\n", portsc.pls_read());
                    if portsc.ccs_read() && attempts < PORT_RESET_MAX_ATTEMPTS {
                        kprintln!(Warn, "Port {} reset timed out, retry {}", port, attempts + 1);
                        portsc_issue_reset(xhci, port as u8);
                        xhci.port_state[idx] =
                            PortState::ResetInProgress { started_ms: now, attempts: attempts + 1 };
                    } else {
                        kprintln!(Warn, "Port {} reset timed out, PORTSC={:#010x} (PED={} PLS={:?} PRC={} CCS={})",
                            port, portsc.raw(), portsc.ped_read(), portsc.pls_read(),
                            portsc.prc_read(), portsc.ccs_read());
                        xhci.port_state[idx] = PortState::Idle;
                    }
                }

            PortState::Resuming { started_ms, phase } => match phase {
                ResumePhase::Resume if now.wrapping_sub(started_ms) >= 20 => {
                    let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                    let mut w = PortStatusControl::write_from_raw(portsc.raw());
                    // kprint!("pls={:?}\n", w.pls_read());
                    w.pls_write(U0);
                    w.write_to_port(xhci.operational_base, port as u8);
                    kprintln!("Transitioning port {} to RExit state.", port);
                    xhci.port_state[idx] = PortState::Resuming { started_ms: now, phase: ResumePhase::RExit };
                }

                ResumePhase::RExit => {
                    let portsc = PortStatusControl::from_port(xhci.operational_base, port as u8);
                    // kprint!("pls={:?}\n", portsc.pls_read());
                    if portsc.pls_read() == U0 {
                        portsc_issue_reset(xhci, port as u8);
                        xhci.port_state[idx] = PortState::ResetInProgress { started_ms: now, attempts: 1 };
                    } else if now.wrapping_sub(started_ms) >= PORT_RESUME_TIMEOUT_MS {
                        kprintln!(Warn, "Port {} stuck leaving resume (PLS={:?})", port, portsc.pls_read());
                        xhci.port_state[idx] = PortState::Idle;
                    }
                }
                _ => {}
            },
            PortState::Debounce { .. } | PortState::ResetInProgress { .. } => {
                kprintln!("");
            }
        }
    }
}

unsafe fn portsc_ack_changes(xhci: &XHCI, port: u8, snapshot: PortStatusControl) {
    PortStatusControl::ack_changes_of(snapshot)
        .write_to_port(xhci.operational_base, port);
}

unsafe fn portsc_issue_reset(xhci: &XHCI, port: u8) {
    let portsc = PortStatusControl::from_port(xhci.operational_base, port);
    let mut cmd = PortStatusControl::write_from_raw(portsc.raw());
    cmd.pr_write();
    cmd.write_to_port(xhci.operational_base, port);
}

fn alloc_dma_erst() -> Option<(DmaAlloc, &'static mut ERST)> {
    let alloc = dma_alloc_zeroed(size_of::<ERST>(), PageSize::SIZE_4KB as usize)?;
    let erst = unsafe { alloc.as_mut::<ERST>() };
    Some((alloc, erst))
}

fn xhci_init_port_states(xhci: &mut XHCI) {
    let now = timer_lapic_uptime_ms();
    for port in 1..=xhci.max_ports as u8 {
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);
        if !portsc.ccs_read() {
            continue;
        }
        let Some(port_info) = xhci.port_info(port) else { continue };
        if port_info.protocol == Usb2 {
            kprintln!(Info, "Startup: device already present on port {} (PLS={:?})",
                port, portsc.pls_read());

            xhci.port_state[port as usize] = PortState::Debounce { stable_since_ms: now };
        } else {
            // TODO: USB3
        }
    }
}

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

            stop_controller(operational_base)?;
            reset_controller(operational_base)?;

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
            kprintln!(Debug, "Allocated DMA for dcbaa at phys{:#011x}; virt: {:#011x} with align {}", dcbaa_dma.phys, dcbaa_dma.virt, dcbaa_dma.layout.align());
            let dcbaa = dcbaa_dma.as_mut::<Dcbaa>();
            mmio_write::<u64>(
                operational_base,
                OP_REG_DCBAAP as u64,
                dcbaa_dma.phys.as_u64(),
            );

            let hcsparams2 = ptr::read_volatile(cap_base.add(0x08).as_ptr::<u32>());
            let hi = (hcsparams2 >> 21) & 0x1F;
            let lo = (hcsparams2 >> 27) & 0x1F;
            let count = ((hi << 5) | lo) as usize;
            kprintln!(Debug, "Scratchpad buffers required: {}", count);

            let mut scratchpad_array_dma: Option<DmaAlloc> = None;
            let mut scratchpad_entries_dma: Option<Vec<DmaAlloc>> = None;

            if count > 0 {
                let array = dma_alloc_zeroed(count * 8, 4096).expect("scratchpad array");
                let entries = array.virt.as_mut_ptr::<u64>();
                scratchpad_entries_dma = Some(Vec::with_capacity(count));
                for i in 0..count {
                    let buf = dma_alloc_zeroed(4096, 4096).expect("scratchpad buffer");
                    ptr::write_volatile(entries.add(i), buf.phys.as_u64());
                    scratchpad_entries_dma.as_mut().unwrap().push(buf);
                }
                dcbaa.set_context(0, array.phys.as_u64()); // DCBAA[0] = scratchpad array
                scratchpad_array_dma = Some(array);
            }


            let (command_ring_alloc, command_ring_primary) = TrbRing::dma_alloc(COMMAND_RING_TRBS).
                expect("Failed to allocate DMA for command ring.");

            kprintln!(Debug, "Allocated DMA for command ring at phys{:#011x}; virt: {:#011x} with align {}",
                command_ring_alloc.phys,
                command_ring_alloc.virt,
                command_ring_alloc.layout.align()
            );

            let command_ring = CommandRing::new(command_ring_primary, command_ring_alloc.phys);
            mmio_write::<u64>(
                operational_base,
                OP_REG_CRCR as u64,
                (command_ring_alloc.phys.as_u64() & !COMMAND_RING_RESERVED_BITS)
                    | COMMAND_RING_CYCLE_STATE,
            );

            let interrupt_config = xhci_configure_interrupts(&dev)?;

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
            let primary_interrupter_offset = runtime_interrupter_offset(PRIMARY_INTERRUPTER);
            let transfer_interrupter_offset = runtime_interrupter_offset(TRANSFER_INTERRUPTER);

            /*
            The Event Ring Segment Table Base Address Register identifies the start address
            of the Event Ring Segment Table (ERST). Refer to section 6.5 for the definition of
            an ERST entry.
             */
            //allocate event rings for port data and transfer events
            let (event_ring_primary_dma, event_ring_primary) = TrbRing::dma_alloc(EVENT_RING_TRBS).
                expect("Failed to allocate dma for event ring (primary).");
            let (event_ring_secondary_dma, event_ring_secondary) =
                TrbRing::dma_alloc(EVENT_RING_TRBS).
                    expect("Failed to allocate dma for event ring (secondary).");

            kprintln!(Debug, "Allocated DMA for event ring 1 at phys{:#011x}; virt: {:#011x} with align {}",
                event_ring_primary_dma.phys,
                event_ring_primary_dma.virt,
                event_ring_primary_dma.layout.align()
            );

            kprintln!(Debug, "Allocated DMA for event ring 1 at phys{:#011x}; virt: {:#011x} with align {}",
                event_ring_secondary_dma.phys,
                event_ring_secondary_dma.virt,
                event_ring_secondary_dma.layout.align()
            );

            //allocate and initialize erst's
            let (erst_primary_dma, erst_primary) = alloc_dma_erst().
                expect("Failed to allocate dma for erst (primary).");
            let (erst_secondary_dma, erst_secondary) = alloc_dma_erst().
                expect("Failed to allocate dma for erst (secondary).");

            kprintln!(Debug, "Allocated DMA for erst 1 at phys{:#011x}; virt: {:#011x} with align {}",
                erst_primary_dma.phys,
                erst_primary_dma.virt,
                erst_primary_dma.layout.align()
            );

            kprintln!(Debug, "Allocated DMA for erst 2 at phys{:#011x}; virt: {:#011x} with align {}",
                erst_secondary_dma.phys,
                erst_secondary_dma.virt,
                erst_secondary_dma.layout.align()
            );

            *erst_primary = ERST::new(event_ring_primary_dma.phys.as_u64(), EVENT_RING_TRBS as u32);
            *erst_secondary = ERST::new(
                event_ring_secondary_dma.phys.as_u64(),
                EVENT_RING_TRBS as u32,
            );

            mmio_write::<u32>(
                runtime_base,
                RT_ERSTSZ as u64 + primary_interrupter_offset,
                SINGLE_ERST_SEGMENT,
            );
            mmio_write::<u32>(
                runtime_base,
                RT_ERSTSZ as u64 + transfer_interrupter_offset,
                SINGLE_ERST_SEGMENT,
            );
            mmio_write::<u64>(
                runtime_base,
                RT_ERSTBA as u64 + primary_interrupter_offset,
                erst_primary_dma.phys.as_u64(),
            );
            mmio_write::<u64>(
                runtime_base,
                RT_ERSTBA as u64 + transfer_interrupter_offset,
                erst_secondary_dma.phys.as_u64(),
            );
            mmio_write::<u64>(
                runtime_base,
                RT_ERDP as u64 + primary_interrupter_offset,
                event_ring_primary_dma.phys.as_u64() & !ERDP_RESERVED_BITS,
            );
            mmio_write::<u64>(
                runtime_base,
                RT_ERDP as u64 + transfer_interrupter_offset,
                event_ring_secondary_dma.phys.as_u64() & !ERDP_RESERVED_BITS,
            );

            let primary_iman =
                mmio_read::<u32>(runtime_base, RT_IMAN as u64 + primary_interrupter_offset);
            mmio_write::<u32>(
                runtime_base,
                RT_IMAN as u64 + primary_interrupter_offset,
                primary_iman | INTERRUPTER_MANAGEMENT_ENABLE,
            );
            let transfer_iman =
                mmio_read::<u32>(runtime_base, RT_IMAN as u64 + transfer_interrupter_offset);
            mmio_write::<u32>(
                runtime_base,
                RT_IMAN as u64 + transfer_interrupter_offset,
                transfer_iman | INTERRUPTER_MANAGEMENT_ENABLE,
            );

            let primary_interrupter = XhciInterrupterState::new(
                runtime_base,
                primary_interrupter_offset,
                event_ring_primary_dma.virt,
                XhciInterrupterKind::Primary,
                "primary",
            );
            let transfer_interrupter = XhciInterrupterState::new(
                runtime_base,
                transfer_interrupter_offset,
                event_ring_secondary_dma.virt,
                XhciInterrupterKind::Transfer,
                "transfer",
            );
            let event_ring_primary =
                EventRing::new(event_ring_primary, event_ring_primary_dma.phys);
            let event_ring_secondary =
                EventRing::new(event_ring_secondary, event_ring_secondary_dma.phys);

            let ext_cap_address = first_ext_cap_addr(cap_base, hccparams1);

            //fix for intel panther point chipset - switch over usb2 ports to xhci
            if dev.vendor_id() == PciVendor::INTEL {
                dev.usb_intel_enable_xhci_ports();
            }
            let supported_protocols = parse_xhci_supported_protocols(ext_cap_address, max_ports);
            // debug_print_supported_protocols(&supported_protocols);
            enable_usb3_port_power(operational_base, &supported_protocols);
            // debug_print_usb3_portsc(operational_base, &supported_protocols);

            let doorbell_offset_bytes = mmio_read::<u32>(cap_base, CAP_REG_DBOFF as u64);
            let shadow_ring: ShadowRing<COMMAND_RING_TRBS> = ShadowRing::new(command_ring_alloc.phys);

            let xhci_struct = XHCI::new(
                dev,
                operational_base,
                cap_base,
                max_slots,
                context_size,
                dcbaa_dma,
                dcbaa,
                command_ring_alloc,
                command_ring,
                event_ring_primary_dma,
                event_ring_primary,
                event_ring_secondary_dma,
                event_ring_secondary,
                erst_primary_dma,
                erst_primary,
                erst_secondary_dma,
                erst_secondary,
                primary_interrupter,
                transfer_interrupter,
                interrupt_config,
                supported_protocols,
                doorbell_offset_bytes,
                shadow_ring,
                max_ports,
                scratchpad_array_dma,
                scratchpad_entries_dma
            );

            let xhci_controller = Box::leak(Box::new(xhci_struct));
            xhci_controller.bind_interrupters_to_controller();
            xhci_controller.register_interrupt_handlers()?;

            xhci_register_for_ticks(xhci_controller);
            xhci_start_controller(operational_base)?;
            xhci_init_port_states(xhci_controller);

            let usbcmd = ptr::read_volatile(xhci_controller.operational_base.as_ptr::<u32>());          // offset 0x00
            let usbsts = ptr::read_volatile(xhci_controller.operational_base.add(0x04).as_ptr::<u32>());
            kprintln!(Debug, "USBCMD={:#x} (R/S={}) USBSTS={:#x} (HCH={} HSE={} CNR={})",
                usbcmd, usbcmd & 1,
                usbsts, usbsts & 1, (usbsts >> 2) & 1, (usbsts >> 11) & 1);
        }

        Ok(())
    }
}

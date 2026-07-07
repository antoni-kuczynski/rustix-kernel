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
    InvalidBarType, XhciCommandRingInitFailure, XhciControllerNotReadyTimeout,
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
use crate::drivers::usb::xhci::xhci_portsc::PortStatusControl;
use crate::drivers::usb::xhci::xhci_slot_context::*;
use crate::drivers::usb::xhci::xhci_trb::*;
use crate::drivers::usb::xhci::*;
use crate::interrupts::router::register_handler_with_context;
use crate::interrupts::vector::InterruptVector;
use crate::memory::dma::{dma_alloc_zeroed, DmaAlloc};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::mem::{align_of, size_of};
use core::ops::Add;
use core::ptr;
use core::ptr::null_mut;
use x86_64::VirtAddr;
use x86_64::structures::idt::InterruptStackFrame;
use crate::drivers::pci::pci_msi::{MsiCapability, MsixCapability, MsixPBA};
use crate::drivers::usb::xhci::xhci_ext_cap::XhciPortProtocol::Usb2;
use crate::drivers::usb::xhci::xhci_portsc::PortLinkState::U0;
use crate::drivers::usb::xhci::xhci_trb_ring::{CommandRing, EventRing, Ring, RingError, TrbRing};
use crate::kprintln;

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

unsafe fn start_controller(operational_base: VirtAddr) -> Result<(), PciDeviceInitError> {
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

pub struct XHCI {
    pub pci_device: PciDevice,
    operational_base: VirtAddr,
    slots: u32,
    context_size: u32,
    dcbaa_dma: DmaAlloc,
    dcbaa: &'static mut Dcbaa,
    command_ring_dma: DmaAlloc,
    command_ring: UnsafeCell<CommandRing<'static>>,
    event_ring_primary_dma: DmaAlloc,
    event_ring_primary: UnsafeCell<EventRing<'static>>,
    event_ring_secondary_dma: DmaAlloc,
    event_ring_secondary: UnsafeCell<EventRing<'static>>,
    erst_primary_dma: DmaAlloc,
    erst_primary: &'static mut ERST,
    erst_secondary_dma: DmaAlloc,
    erst_secondary: &'static mut ERST,
    primary_interrupter: XhciInterrupterState,
    transfer_interrupter: XhciInterrupterState,
    interrupt_config: XhciInterruptConfig,
    supported_protocols: Vec<XhciPortInfo>,
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

    unsafe fn handle_device_attach(&self, xhci: &XHCI, port: u8) {
        let Some(port_info) = xhci.port_info(port) else {
            kprintln!(Debug, "Attach detected at port {} with unknown protocol", port);
            return;
        };

        kprintln!(Info,
            "Attach detected at port {} and protocol {:?}",
            port,
            port_info.protocol
        );

        let fresh_portsc = PortStatusControl::from_port(xhci.operational_base, port);

        if port_info.protocol == Usb2 {
            let mut clean_cmd = PortStatusControl::write_from_raw(fresh_portsc.raw());

            clean_cmd.change_all_write();
            clean_cmd.pr_write();

            clean_cmd.write_to_port(xhci.operational_base, port);
            kprintln!(Debug, "Issued Port Reset on port {}", port);

        } else if port_info.protocol == XhciPortProtocol::Usb3 {
            //TODO: usb3 transition to enabled state
        }
    }

    unsafe fn handle_device_detach(&self, xhci: &XHCI, port: u8, portsc: PortStatusControl) {
        let Some(port_info) = xhci.port_info(port) else {
            kprintln!(Info,"Detach detected at port {} with unknown protocol", port);
            return;
        };
        // vgaprintln!(
        //     "Detach detected at port {} and protocol {}",
        //     port,
        //     port_info.protocol
        // );
    }

    unsafe fn handle_port_reset(&self, xhci: &mut XHCI, port: u8) {
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);

        if portsc.ped_read() && !portsc.pr_read() && portsc.pls_read() == U0 {
            kprintln!(Debug, "Succesfully reset port {}", port); //TODO: usb3
        } else {
            kprintln!(Warn, "Failed to reset port {}", port);
        }

        //we did reset the slot, so now we're in enabled state
        //TODO: usb3 logic (but it's probably the same, prioritizing usb2 for keyboard support)


        let mut trb = Trb::new();
        trb.set_trb_type(Trb::TRB_ENABLE_SLOT);
        xhci.send_command(trb).expect("Sending enable slot command for XHCI failed.");


    }

    unsafe fn handle_port_status_change(&self, xhci: &mut XHCI, trb: PortStatusChangeEventTrb) {
        let port = trb.read_port_id();

        if !xhci.is_valid_port(port) {
            kprintln!(Info,"Ignoring port status change for invalid port {}", port);
            return;
        }
        let portsc = PortStatusControl::from_port(xhci.operational_base, port);

        let mut ack = PortStatusControl::write_from_raw(portsc.raw());
        ack.change_all_write();
        ack.write_to_port(xhci.operational_base, port);
        kprintln!(Debug, "Cleared port status change event.");

        if portsc.prc_read() == true {
            self.handle_port_reset(xhci, port);
            return;
        }

        let csc = portsc.csc_read();
        let ccs = portsc.ccs_read();

        if csc && ccs {
            self.handle_device_attach(xhci, port);
        } else if csc && !ccs {
            self.handle_device_detach(xhci, port, portsc);
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

        debug_print_usb3_portsc(xhci.operational_base, &xhci.supported_protocols);

        while let Ok(trb) = event_ring.dequeue() {
            let trb_type = trb.trb_type();
            // vgaprintln!(
            //     "xHCI {} event TRB: type={} cycle={} param={:#018x} status={:#010x} control={:#010x}",
            //     self.name,
            //     trb_type,
            //     trb.cycle(),
            //     trb.parameter(),
            //     trb.status(),
            //     trb.control()
            // );

            if trb_type == Trb::TRB_PORT_STATUS_CHANGE_EVENT {
                let event_change = trb
                    .try_as_port_status_change_event()
                    .expect("Cannot parse change event TRB!");
                self.handle_port_status_change(xhci, event_change);
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

    fn send_command(&mut self, trb: Trb) -> Result<(), RingError> {
        self.command_ring.get_mut().enqueue(trb)
    }

    fn new(
        pci_device: PciDevice,
        operational_base: VirtAddr,
        slots: u32,
        context_size: u32,
        dcbaa_dma: DmaAlloc,
        dcbaa: &'static mut Dcbaa,
        command_ring_dma: DmaAlloc,
        command_ring: CommandRing<'static>,
        event_ring_primary_dma: DmaAlloc,
        event_ring_primary: EventRing<'static>,
        event_ring_secondary_dma: DmaAlloc,
        event_ring_secondary: EventRing<'static>,
        erst_primary_dma: DmaAlloc,
        erst_primary: &'static mut ERST,
        erst_secondary_dma: DmaAlloc,
        erst_secondary: &'static mut ERST,
        primary_interrupter: XhciInterrupterState,
        transfer_interrupter: XhciInterrupterState,
        interrupt_config: XhciInterruptConfig,
        supported_protocols: Vec<XhciPortInfo>,
    ) -> Self {
        XHCI {
            pci_device,
            operational_base,
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

    fn bind_interrupters_to_controller(&mut self) {
        let xhci = self as *mut XHCI;
        self.primary_interrupter.set_controller(xhci);
        self.transfer_interrupter.set_controller(xhci);
    }
}

fn alloc_dma_erst() -> Option<(DmaAlloc, &'static mut ERST)> {
    let alloc = dma_alloc_zeroed(size_of::<ERST>(), 64)?;
    let erst = unsafe { alloc.as_mut::<ERST>() };
    Some((alloc, erst))
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
            let base = iomap.virt_addr;

            let cap_length = mmio_read::<u8>(base, CAP_REG_CAPLENGTH as u64);
            let operational_base = base.add(cap_length as u64);

            let runtime_offset = mmio_read::<u32>(base, CAP_REG_RTSOFF as u64) as u64;
            let runtime_base =
                VirtAddr::new((base.as_u64() + runtime_offset) & RUNTIME_BASE_ALIGNMENT_MASK);

            let hccparams1 = mmio_read::<u32>(base, CAP_REG_HCCPARAMS1 as u64);
            let ext_cap_address = first_ext_cap_addr(base, hccparams1);
            xhci_legacy_handoff(ext_cap_address);

            stop_controller(operational_base)?;
            reset_controller(operational_base)?;

            let hcsparams1 = mmio_read::<u32>(base, CAP_REG_HCSPARAMS1 as u64);
            let hccparams1 = mmio_read::<u32>(base, CAP_REG_HCCPARAMS1 as u64);

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

            let dcbaa_dma = dma_alloc_zeroed(size_of::<Dcbaa>(), align_of::<Dcbaa>())
                .expect("Failed to allocate dma for dcbaa.");
            let dcbaa = dcbaa_dma.as_mut::<Dcbaa>();
            mmio_write::<u64>(
                operational_base,
                OP_REG_DCBAAP as u64,
                dcbaa_dma.phys.as_u64(),
            );

            let a = TrbRing::dma_alloc(COMMAND_RING_TRBS);
            let b = a.unwrap();


            let (command_ring_alloc, command_ring_primary) = TrbRing::dma_alloc(EVENT_RING_TRBS).
                expect("Failed to allocate DMA for command ring.");
            
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

            //allocate and initialize erst's
            let (erst_primary_dma, erst_primary) = alloc_dma_erst().
                expect("Failed to allocate dma for erst (primary).");
            let (erst_secondary_dma, erst_secondary) = alloc_dma_erst().
                expect("Failed to allocate dma for erst (secondary).");

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

            let ext_cap_address = first_ext_cap_addr(base, hccparams1);

            //fix for intel panther point chipset - switch over usb2 ports to xhci
            if dev.vendor_id() == PciVendor::INTEL {
                dev.usb_intel_enable_xhci_ports();
            }
            let supported_protocols = parse_xhci_supported_protocols(ext_cap_address, max_ports);
            // debug_print_supported_protocols(&supported_protocols);
            enable_usb3_port_power(operational_base, &supported_protocols);
            debug_print_usb3_portsc(operational_base, &supported_protocols);

            let xhci_struct = XHCI::new(
                dev,
                operational_base,
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
            );

            let xhci_controller = Box::leak(Box::new(xhci_struct));
            xhci_controller.bind_interrupters_to_controller();
            xhci_controller.register_interrupt_handlers()?;

            start_controller(operational_base)?;
        }

        Ok(())
    }
}

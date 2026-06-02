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
    XhciInsufficientMsixVectors, XhciMsiCapabilityNotFound, XhciMsixPbaBarInvalid,
    XhciMsixTableBarInvalid,
};
use crate::drivers::pci::pci_device::{PciDeviceHeader, PciDeviceInitError, PciDeviceInitializer};
use crate::drivers::pci::pci_io::{pci_read8, pci_read16, pci_read32, pci_write16, pci_write32};
use crate::drivers::pci::*;
use crate::drivers::usb::xhci::xhci_endpoint_context::*;
use crate::drivers::usb::xhci::xhci_msix::*;
use crate::drivers::usb::xhci::xhci_portsc::PortStatusControl;
use crate::drivers::usb::xhci::xhci_slot_context::*;
use crate::drivers::usb::xhci::xhci_trb::*;
use crate::drivers::usb::xhci::*;
use crate::interrupts::router::register_handler_with_context;
use crate::interrupts::vector::InterruptVector;
use crate::interrupts::vector::allocate_vectors;
use crate::memory::dma::DmaAlloc;
use crate::vgaprintln;
use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::mem::{align_of, size_of};
use core::ops::Add;
use core::ptr;
use x86_64::VirtAddr;
use x86_64::structures::idt::InterruptStackFrame;

const PCI_COMMAND_REGISTER: u32 = 0x04;
const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
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

#[derive(Clone, Copy)]
pub(crate) enum XhciInterrupterKind {
    Primary,
    Transfer,
}

fn runtime_interrupter_offset(interrupter: u8) -> u64 {
    interrupter as u64 * INTERRUPTER_REGISTER_STRIDE
}

fn msi_message_address() -> u64 {
    let lapic_id = unsafe { LAPIC.get().map(|lapic| lapic.id()).unwrap_or(0) as u64 };
    LAPIC_MSI_ADDR | (lapic_id << 12)
}

fn enable_pci_mmio_and_bus_mastering(pci_device: &PciDeviceHeader) {
    let command = pci_read16(pci_device.base_id(), PCI_COMMAND_REGISTER);
    pci_write16(
        pci_device.base_id(),
        PCI_COMMAND_REGISTER,
        command | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER,
    );
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

fn find_pci_capability(pci_device: &PciDeviceHeader, capability_id: u8) -> Option<u8> {
    let status = pci_read16(pci_device.base_id(), PCI_STATUS_REGISTER);
    if status & PCI_STATUS_CAPABILITIES_LIST == 0 {
        return None;
    }

    let mut cap_ptr = pci_read8(pci_device.base_id(), PCI_CAPABILITY_POINTER_REGISTER);
    while cap_ptr != 0 {
        let cap_id = pci_read8(pci_device.base_id(), cap_ptr as u32);
        if cap_id == capability_id {
            return Some(cap_ptr);
        }

        cap_ptr = pci_read8(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET,
        );
    }

    None
}

fn configure_msix(
    pci_device: &PciDeviceHeader,
    cap_ptr: u8,
) -> Result<XhciInterruptConfig, PciDeviceInitError> {
    let [command_vector, transfer_vector] =
        allocate_vectors::<2>().ok_or(XhciInsufficientMsixVectors)?;

    let mut msix_capability = MsixCapability {
        cap_id: pci_read8(pci_device.base_id(), cap_ptr as u32),
        next: pci_read8(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET,
        ),
        message_control: pci_read16(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
        ),
        table: pci_read32(pci_device.base_id(), cap_ptr as u32 + PCI_MSIX_TABLE_OFFSET),
        pba: pci_read32(pci_device.base_id(), cap_ptr as u32 + PCI_MSIX_PBA_OFFSET),
    };

    msix_capability.mask_all();
    pci_write16(
        pci_device.base_id(),
        cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
        msix_capability.message_control,
    );

    if msix_capability.table_size() < 2 {
        return Err(XhciInsufficientMsixVectors);
    }

    let table_bar = PciBAR::from_bir(pci_device, msix_capability.table_bir())
        .map_err(|_| XhciMsixTableBarInvalid)?;
    let table_iomap = table_bar.ioremap_checked();
    let table_mmio = table_iomap
        .virt_addr
        .add(msix_capability.table_offset() as u64);

    let msix_table_ptr = table_mmio.as_mut_ptr::<MsixTableEntry>();
    let _msix_table_view = MsiXTableView::new(msix_table_ptr);

    let mut entry0 = unsafe { ptr::read_unaligned(msix_table_ptr) };
    let mut entry1 = unsafe { ptr::read_unaligned(msix_table_ptr.wrapping_add(1)) };

    let message_address = msi_message_address();

    entry0.msg_addr_low = message_address as u32;
    entry0.msg_addr_high = (message_address >> 32) as u32;
    entry0.msg_data = command_vector.as_u8() as u32;
    entry0.vector_ctrl = 0;

    entry1.msg_addr_low = message_address as u32;
    entry1.msg_addr_high = (message_address >> 32) as u32;
    entry1.msg_data = transfer_vector.as_u8() as u32;
    entry1.vector_ctrl = 0;

    unsafe {
        ptr::write_unaligned(msix_table_ptr, entry0);
        ptr::write_unaligned(msix_table_ptr.wrapping_add(1), entry1);
    }

    let pba_bar = PciBAR::from_bir(pci_device, msix_capability.pba_bir())
        .map_err(|_| XhciMsixPbaBarInvalid)?;
    let pba_iomap = pba_bar.ioremap_checked();
    let msix_pba = MsixPBA::new(
        pba_iomap.virt_addr.as_mut_ptr::<u8>(),
        msix_capability.pba_offset(),
        2,
    );

    msix_capability.unmask_all();
    msix_capability.enable();
    pci_write16(
        pci_device.base_id(),
        cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
        msix_capability.message_control,
    );

    Ok(XhciInterruptConfig::Msix {
        capability: msix_capability,
        pba: msix_pba,
        command_vector,
        transfer_vector,
    })
}

fn configure_msi(
    pci_device: &PciDeviceHeader,
    cap_ptr: u8,
) -> Result<XhciInterruptConfig, PciDeviceInitError> {
    let [vector] = allocate_vectors::<1>().ok_or(XhciInsufficientMsixVectors)?;

    let message_address = msi_message_address();

    let mut msi_capability = MsiCapability {
        cap_id: pci_read8(pci_device.base_id(), cap_ptr as u32),
        next: pci_read8(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET,
        ),
        message_control: pci_read16(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_MSI_MESSAGE_CONTROL_OFFSET,
        ),
        message_address_low: message_address as u32,
        message_address_high: (message_address >> 32) as u32,
        message_data: vector.as_u8() as u16,
    };

    pci_write32(
        pci_device.base_id(),
        cap_ptr as u32 + PCI_MSI_MESSAGE_ADDRESS_LOW_OFFSET,
        msi_capability.message_address_low,
    );

    let message_data_offset = if msi_capability.is_64_bit_capable() {
        pci_write32(
            pci_device.base_id(),
            cap_ptr as u32 + PCI_MSI_MESSAGE_ADDRESS_HIGH_OFFSET,
            msi_capability.message_address_high,
        );
        PCI_MSI_MESSAGE_DATA_64_OFFSET
    } else {
        PCI_MSI_MESSAGE_DATA_32_OFFSET
    };

    pci_write16(
        pci_device.base_id(),
        cap_ptr as u32 + message_data_offset,
        msi_capability.message_data,
    );

    msi_capability.enable_single_vector();
    pci_write16(
        pci_device.base_id(),
        cap_ptr as u32 + PCI_MSI_MESSAGE_CONTROL_OFFSET,
        msi_capability.message_control,
    );

    Ok(XhciInterruptConfig::Msi {
        capability: msi_capability,
        vector,
    })
}

fn configure_interrupts(
    pci_device: &PciDeviceHeader,
) -> Result<XhciInterruptConfig, PciDeviceInitError> {
    if let Some(msix_cap_ptr) = find_pci_capability(pci_device, PCI_CAPABILITY_ID_MSIX) {
        return configure_msix(pci_device, msix_cap_ptr);
    }

    let msi_cap_ptr =
        find_pci_capability(pci_device, PCI_CAPABILITY_ID_MSI).ok_or(XhciMsiCapabilityNotFound)?;
    configure_msi(pci_device, msi_cap_ptr)
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

pub struct XHCI<'a> {
    pci_device: &'a PciDeviceHeader,
    operational_base: VirtAddr,
    slots: u32,
    context_size: u32,
    dcbaa_dma: DmaAlloc,
    dcbaa: &'a mut Dcbaa,
    command_ring_dma: DmaAlloc,
    command_ring: TrbRing,
    event_ring_primary_dma: DmaAlloc,
    event_ring_primary: UnsafeCell<EventRing>,
    event_ring_secondary_dma: DmaAlloc,
    event_ring_secondary: UnsafeCell<EventRing>,
    erst_primary_dma: DmaAlloc,
    erst_primary: &'a mut ERST,
    erst_secondary_dma: DmaAlloc,
    erst_secondary: &'a mut ERST,
    primary_interrupter: XhciInterrupterState,
    transfer_interrupter: XhciInterrupterState,
    interrupt_config: XhciInterruptConfig,
}

#[derive(Clone, Copy)]
pub struct XhciInterrupterState {
    controller: Option<&'static XHCI<'static>>,
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
            controller: None,
            runtime_base,
            interrupter_offset,
            event_ring,
            kind,
            name,
        }
    }

    pub fn set_controller(&mut self, controller: &'static XHCI<'static>) {
        self.controller = Some(controller);
    }

    pub const fn controller(&self) -> Option<&'static XHCI<'static>> {
        self.controller
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
            let parameter = core::ptr::read_volatile(trb_ptr.add(0) as *const u64);
            let status = core::ptr::read_volatile(trb_ptr.add(8) as *const u32);
            let control = core::ptr::read_volatile(trb_ptr.add(12) as *const u32);
            let trb_type = (control >> 10) & 0x3f;
            let cycle = control & 1;

            vgaprintln!(
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

    unsafe fn handle_port_status_change(&self, trb: PortStatusChangeEventTrb, controller: &XHCI) {
        let port = trb.read_port_id();
        let portsc = PortStatusControl::from_port(controller.operational_base, port);
        let csc = portsc.csc_read();
        let ccs = portsc.ccs_read();

        if csc && ccs {
            vgaprintln!("Device attached at port {}", port);
        } else if csc && !ccs {
            vgaprintln!("Device disconnected from port {}", port);
        }

        let mut clear = PortStatusControl::write_from_raw(portsc.raw());
        clear.change_all_write();
        clear.write_to_port(controller.operational_base, port);
    }

    unsafe fn handle(&self) {
        let controller = self
            .controller
            .expect("controller not initialized for interrupter");
        let event_ring = match self.kind {
            XhciInterrupterKind::Primary => unsafe { &mut *controller.event_ring_primary.get() },
            XhciInterrupterKind::Transfer => unsafe { &mut *controller.event_ring_secondary.get() },
        };

        while let Ok(trb) = event_ring.dequeue() {
            let trb_type = trb.trb_type();

            if trb_type == Trb::TRB_PORT_STATUS_CHANGE_EVENT {
                let event_change = trb
                    .try_as_port_status_change_event()
                    .expect("Cannot parse change event TRB!");
                self.handle_port_status_change(event_change, controller);
            }

            // let cycle = trb.cycle();
            // let parameter = trb.parameter();
            // let status = trb.status();
            // let control = trb.control();
            //
            // vgaprintln!(
            //     "xHCI {} event TRB: type={} cycle={} param={:#018x} status={:#010x} control={:#010x}",
            //     self.name,
            //     trb_type,
            //     cycle,
            //     parameter,
            //     status,
            //     control
            // );
        }

        self.ack(event_ring.dequeue_phys().as_u64());
    }
}

fn xhci_irq_handler(_: InterruptVector, _: InterruptStackFrame, context: usize) {
    if context == 0 {
        vgaprintln!("xHCI IRQ without interrupter context");
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

impl<'a> XHCI<'a> {
    fn new(
        pci_device: &'a PciDeviceHeader,
        operational_base: VirtAddr,
        slots: u32,
        context_size: u32,
        dcbaa_dma: DmaAlloc,
        dcbaa: &'a mut Dcbaa,
        command_ring_dma: DmaAlloc,
        command_ring: TrbRing,
        event_ring_primary_dma: DmaAlloc,
        event_ring_primary: EventRing,
        event_ring_secondary_dma: DmaAlloc,
        event_ring_secondary: EventRing,
        erst_primary_dma: DmaAlloc,
        erst_primary: &'a mut ERST,
        erst_secondary_dma: DmaAlloc,
        erst_secondary: &'a mut ERST,
        primary_interrupter: XhciInterrupterState,
        transfer_interrupter: XhciInterrupterState,
        interrupt_config: XhciInterruptConfig,
    ) -> Self {
        XHCI {
            pci_device,
            operational_base,
            slots,
            context_size,
            dcbaa_dma,
            dcbaa,
            command_ring_dma,
            command_ring,
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
                    return Err(XhciInsufficientMsixVectors);
                }

                if !register_handler_with_context(
                    *transfer_vector,
                    xhci_irq_handler,
                    &self.transfer_interrupter as *const _ as usize,
                ) {
                    return Err(XhciInsufficientMsixVectors);
                }
            }
            XhciInterruptConfig::Msi { vector, .. } => {
                if !register_handler_with_context(
                    *vector,
                    xhci_irq_handler,
                    &self.primary_interrupter as *const _ as usize,
                ) {
                    return Err(XhciInsufficientMsixVectors);
                }
            }
        }

        Ok(())
    }

    fn bind_interrupters_to_controller(&mut self) {
        let controller = unsafe { &*(self as *const XHCI<'a> as *const XHCI<'static>) };
        self.primary_interrupter.set_controller(controller);
        self.transfer_interrupter.set_controller(controller);
    }
}

fn alloc_dma_erst() -> Result<(DmaAlloc, &'static mut ERST), PciDeviceInitError> {
    let alloc = dma_alloc_zeroed(size_of::<ERST>(), 64)?;
    let erst = unsafe { dma_as_mut::<ERST>(&alloc) };
    Ok((alloc, erst))
}

impl PciDeviceInitializer for XHCI<'_> {
    fn initialize(pci_device: &PciDeviceHeader) -> Result<(), PciDeviceInitError> {
        let bar = PciBAR::get(pci_device, 0);

        if bar.bar_type() == &BarType::Io {
            return Err(InvalidBarType);
        }

        enable_pci_mmio_and_bus_mastering(pci_device);

        unsafe {
            let iomap = bar.ioremap_checked();
            let base = iomap.virt_addr;

            let cap_length = mmio_read::<u8>(base, CAP_REG_CAPLENGTH as u64);
            let operational_base = base.add(cap_length as u64);

            let runtime_offset = mmio_read::<u32>(base, CAP_REG_RTSOFF as u64) as u64;
            let runtime_base =
                VirtAddr::new((base.as_u64() + runtime_offset) & RUNTIME_BASE_ALIGNMENT_MASK);

            stop_controller(operational_base)?;
            reset_controller(operational_base)?;

            let hcsparams1 = mmio_read::<u32>(base, CAP_REG_HCSPARAMS1 as u64);
            let hccparams1 = mmio_read::<u32>(base, CAP_REG_HCCPARAMS1 as u64);

            //enable all slots
            let max_slots = hcsparams1 & MAX_SLOTS_MASK;
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

            let dcbaa_dma = dma_alloc_zeroed(size_of::<Dcbaa>(), align_of::<Dcbaa>())?;
            let dcbaa = dma_as_mut::<Dcbaa>(&dcbaa_dma);
            mmio_write::<u64>(
                operational_base,
                OP_REG_DCBAAP as u64,
                dcbaa_dma.phys.as_u64(),
            );

            let (command_ring_dma, trb_arr) = alloc_dma_trb_ring(COMMAND_RING_TRBS)?;
            let command_ring = TrbRing::new(trb_arr, command_ring_dma.phys)
                .map_err(|_| XhciCommandRingInitFailure)?;
            mmio_write::<u64>(
                operational_base,
                OP_REG_CRCR as u64,
                (command_ring_dma.phys.as_u64() & !COMMAND_RING_RESERVED_BITS)
                    | COMMAND_RING_CYCLE_STATE,
            );

            let interrupt_config = configure_interrupts(pci_device)?;

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
            let (event_ring_primary_dma, event_ring_primary) = alloc_dma_trb_ring(EVENT_RING_TRBS)?;
            let (event_ring_secondary_dma, event_ring_secondary) =
                alloc_dma_trb_ring(EVENT_RING_TRBS)?;

            //allocate and initialize erst's
            let (erst_primary_dma, erst_primary) = alloc_dma_erst()?;
            let (erst_secondary_dma, erst_secondary) = alloc_dma_erst()?;

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

            let xhci_controller = Box::leak(Box::new(XHCI::new(
                pci_device,
                operational_base,
                max_slots,
                context_size,
                dcbaa_dma,
                dcbaa,
                command_ring_dma,
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
            )));
            xhci_controller.bind_interrupters_to_controller();
            xhci_controller.register_interrupt_handlers()?;

            start_controller(operational_base)?;
        }

        Ok(())
    }
}

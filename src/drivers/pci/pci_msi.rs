/*
 * Created by Antoni Kuczyński
 * 07/07/2026
 */
use alloc::vec::Vec;
use core::ops::Add;
use core::ptr;
use crate::drivers::apic::apic::LAPIC;
use crate::drivers::pci::pci_bar::PciBAR;
use crate::drivers::pci::pci_device::{PciDevice, PciDeviceInitError};
use crate::drivers::pci::pci_device::PciDeviceInitError::{InsufficientMsixVectors, InvalidBarType, MsixPbaBarInvalid};
use crate::interrupts::vector::{allocate_vectors, InterruptVector};
use crate::kprintln;


const PCI_CAPABILITY_NEXT_POINTER_OFFSET: u32 = 0x01;
const PCI_MSIX_MESSAGE_CONTROL_OFFSET: u32 = 0x02;
const PCI_MSIX_TABLE_OFFSET: u32 = 0x04;
const PCI_MSIX_PBA_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_CONTROL_OFFSET: u32 = 0x02;
const PCI_MSI_MESSAGE_ADDRESS_LOW_OFFSET: u32 = 0x04;
const PCI_MSI_MESSAGE_ADDRESS_HIGH_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_DATA_32_OFFSET: u32 = 0x08;
const PCI_MSI_MESSAGE_DATA_64_OFFSET: u32 = 0x0C;
const LAPIC_MSI_ADDR: u64 = 0xFEE0_0000;

//=======================================================
//      MSI-X CONFIGURATION CAPABILITY STRUCTURE
//=======================================================
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MsixCapability {
    pub cap_id: u8, //0x11
    pub next: u8,   //next capability pointer
    pub message_control: u16,
    pub table: u32,
    pub pba: u32,
}

impl MsixCapability {
    pub unsafe fn new(ptr: *const u8) -> Self {
        unsafe { ptr::read_unaligned(ptr as *const MsixCapability) }
    }

    /// Number of MSI-X vectors supported
    pub fn table_size(&self) -> u16 {
        (self.message_control & 0x07FF) + 1
    }

    /// Is MSI-X enabled?
    pub fn enabled(&self) -> bool {
        (self.message_control & (1 << 15)) != 0
    }

    /// Enable MSI-X
    pub fn enable(&mut self) {
        self.message_control |= 1 << 15;
    }

    /// Mask all MSI-X vectors
    pub fn mask_all(&mut self) {
        self.message_control |= 1 << 14;
    }

    /// Unmask all MSI-X vectors
    pub fn unmask_all(&mut self) {
        self.message_control &= !(1 << 14);
    }

    /// BAR index of the MSI-X table
    pub fn table_bir(&self) -> u8 {
        (self.table & 0x7) as u8
    }

    /// Offset of MSI-X table in BAR
    pub fn table_offset(&self) -> u32 {
        self.table & !0x7
    }

    /// BAR index of the PBA
    pub fn pba_bir(&self) -> u8 {
        (self.pba & 0x7) as u8
    }

    /// Offset of the PBA in BAR
    pub fn pba_offset(&self) -> u32 {
        self.pba & !0x7
    }

    pub fn write_back(&self, dev: &PciDevice, cap_ptr: u8) {
        // Message Control is at offset +2
        dev.pci_write16((cap_ptr as u32) + 0x02, self.message_control);
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

        kprintln!(Debug, "MSI-X Capability:");
        kprintln!(Debug, "  Cap ID          : 0x{:02X}", self.cap_id);
        kprintln!(Debug, "  Next Pointer    : 0x{:02X}", self.next);
        kprintln!(Debug, "  Message Control : 0x{:04X}", msg_control);
        kprintln!(Debug, "    Table Size    : {}", table_size);
        kprintln!(Debug, "    Function Mask : {}", function_mask);
        kprintln!(Debug, "    MSI-X Enabled : {}", msix_enabled);
        kprintln!(Debug, "  Table:");
        kprintln!(Debug, "    BIR           : {}", table_bir);
        kprintln!(Debug, "    Offset        : 0x{:08X}", table_offset);
        kprintln!(Debug, "  PBA:");
        kprintln!(Debug, "    BIR           : {}", pba_bir);
        kprintln!(Debug, "    Offset        : 0x{:08X}", pba_offset);
    }
}

//=======================================================
//      MSI-X TABLE ENTRY
//=======================================================
#[repr(C)]
pub struct MsixTableEntry {
    msg_addr_low: u32,
    msg_addr_high: u32,
    msg_data: u32,
    vector_ctrl: u32,
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
    pub fn new(base: *mut MsixTableEntry) -> Self {
        Self { base }
    }

    unsafe fn entry(&self, vector: u16) -> &mut MsixTableEntry {
        unsafe { &mut *self.base.add(vector as usize) }
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
            let e = unsafe { self.entry(i as u16) };

            let addr = ((e.msg_addr_high as u64) << 32) | (e.msg_addr_low as u64);
            let masked = if e.is_masked() { "yes" } else { "no" };

            kprintln!(Debug,
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
pub struct MsixPBA {
    base: *const u32,
    vectors: usize,
}

impl MsixPBA {
    pub fn new(pba_bar_base: *mut u8, pba_offset: u32, vectors: usize) -> Self {
        let base = unsafe { pba_bar_base.add(pba_offset as usize) as *const u32 };

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
//      MSI CONFIGURATION CAPABILITY STRUCTURE
//=======================================================
#[derive(Debug, Clone, Copy)]
pub struct MsiCapability {
    pub cap_id: u8, //0x05
    pub next: u8,
    pub message_control: u16,
    pub message_address_low: u32,
    pub message_address_high: u32,
    pub message_data: u16,
}

impl MsiCapability {
    pub fn is_64_bit_capable(&self) -> bool {
        self.message_control & (1 << 7) != 0
    }

    pub fn enable_single_vector(&mut self) {
        const MSI_ENABLE: u16 = 1 << 0;
        const MULTIPLE_MESSAGE_ENABLE_MASK: u16 = 0b111 << 4;

        self.message_control &= !MULTIPLE_MESSAGE_ENABLE_MASK;
        self.message_control |= MSI_ENABLE;
    }

    pub fn capable_vectors(&self) -> u16 {
        let mmc = (self.message_control >> 1) & 0b111;

        //safeguard for reserved 6 and 7 values
        let safe_mmc = core::cmp::min(mmc, 5);

        1 << safe_mmc //return as power of 2
    }
}

//=======================================================
//      MSI-X VECTOR
//=======================================================
pub struct MsiXVector {
    vector: InterruptVector,
    table: MsiXTableView,
    pba: MsixPBA,
}

impl MsiXVector {
    fn new(vector: InterruptVector, table: MsiXTableView, pba: MsixPBA) -> MsiXVector {
        Self { vector, table, pba }
    }
}

fn msi_message_address() -> u64 {
    let lapic_id = unsafe { LAPIC.get().map(|lapic| lapic.id()).unwrap_or(0) as u64 };
    LAPIC_MSI_ADDR | (lapic_id << 12)
}


pub struct MsixConfiguration {
    pub capability: MsixCapability,
    pub pba: MsixPBA,
    pub vectors: Vec<InterruptVector>,
}

pub struct MsiConfiguration {
    pub capability: MsiCapability,
    pub vectors: Vec<InterruptVector>,
}

impl PciDevice {
    pub fn configure_msix(
        &self,
        cap_ptr: u8,
        requested_vectors: u16
    ) -> Result<MsixConfiguration, PciDeviceInitError> {

        let mut msix_capability = MsixCapability {
            cap_id: self.pci_read8(cap_ptr as u32),
            next: self.pci_read8(
                cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET,
            ),
            message_control: self.pci_read16(
                cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
            ),
            table: self.pci_read32(cap_ptr as u32 + PCI_MSIX_TABLE_OFFSET),
            pba: self.pci_read32(cap_ptr as u32 + PCI_MSIX_PBA_OFFSET),
        };

        //mask all interrupts during configuration
        msix_capability.mask_all();
        self.pci_write16(
            cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
            msix_capability.message_control,
        );

        if msix_capability.table_size() < requested_vectors {
            return Err(InsufficientMsixVectors);
        }

        //vector allocation
        let mut allocated_vectors = Vec::with_capacity(requested_vectors as usize);
        for _ in 0..requested_vectors {
            let [vector] = allocate_vectors::<1>().ok_or(InsufficientMsixVectors)?;
            allocated_vectors.push(vector);
        }

        //msi-x table mapping
        let table_bar = PciBAR::from_bir(self, msix_capability.table_bir())
            .map_err(|_| InvalidBarType)?;
        let table_iomap = table_bar.ioremap_checked();
        let table_mmio = table_iomap
            .virt_addr
            .add(msix_capability.table_offset() as u64);

        let msix_table_ptr = table_mmio.as_mut_ptr::<MsixTableEntry>();
        let _msix_table_view = MsiXTableView::new(msix_table_ptr);

        let message_address = msi_message_address();

        //write the msix table
        for (i, vector) in allocated_vectors.iter().enumerate() {
            let entry_ptr = unsafe { msix_table_ptr.add(i) };
            let mut entry = unsafe { ptr::read_volatile(entry_ptr) };

            entry.msg_addr_low = message_address as u32;
            entry.msg_addr_high = (message_address >> 32) as u32;
            entry.msg_data = vector.as_u8() as u32;
            entry.vector_ctrl = 0; //0 = unmasked

            unsafe {
                ptr::write_volatile(entry_ptr, entry);
            }
        }

        //pba configuration
        let pba_bar = PciBAR::from_bir(self, msix_capability.pba_bir())
            .map_err(|_| MsixPbaBarInvalid)?;
        let pba_iomap = pba_bar.ioremap_checked();
        let msix_pba = MsixPBA::new(
            pba_iomap.virt_addr.as_mut_ptr::<u8>(),
            msix_capability.pba_offset(),
            requested_vectors as usize,
        );

        //enable msi-x
        msix_capability.unmask_all();
        msix_capability.enable();
        self.pci_write16(
            cap_ptr as u32 + PCI_MSIX_MESSAGE_CONTROL_OFFSET,
            msix_capability.message_control,
        );

        Ok(MsixConfiguration {
            capability: msix_capability,
            pba: msix_pba,
            vectors: allocated_vectors,
        })
    }

    pub fn configure_msi(
        &self,
        cap_ptr: u8,
        requested_vectors: u16,
    ) -> Result<MsiConfiguration, PciDeviceInitError> {

        let message_control = self.pci_read16(
            cap_ptr as u32 + PCI_MSI_MESSAGE_CONTROL_OFFSET,
        );

        let mut msi_capability = MsiCapability {
            cap_id: self.pci_read8(cap_ptr as u32),
            next: self.pci_read8(
                cap_ptr as u32 + PCI_CAPABILITY_NEXT_POINTER_OFFSET,
            ),
            message_control,
            message_address_low: 0,
            message_address_high: 0,
            message_data: 0,
        };

        // how many vectors device supports (round to the power of 2)
        let device_capable_vectors = msi_capability.capable_vectors();
        let actual_vectors = core::cmp::min(requested_vectors, device_capable_vectors);

        // allocate vectors
        let mut allocated_vectors = Vec::with_capacity(actual_vectors as usize);
        for _ in 0..actual_vectors {
            let [vector] = allocate_vectors::<1>().ok_or(PciDeviceInitError::InsufficientMsiVectors)?;
            allocated_vectors.push(vector);
        }

        let message_address = msi_message_address();
        let base_vector = allocated_vectors[0].as_u8() as u16;

        msi_capability.message_address_low = message_address as u32;
        msi_capability.message_address_high = (message_address >> 32) as u32;
        msi_capability.message_data = base_vector;

        //write to pci config space
        self.pci_write32(
            cap_ptr as u32 + PCI_MSI_MESSAGE_ADDRESS_LOW_OFFSET,
            msi_capability.message_address_low,
        );

        let message_data_offset = if msi_capability.is_64_bit_capable() {
            self.pci_write32(
                cap_ptr as u32 + PCI_MSI_MESSAGE_ADDRESS_HIGH_OFFSET,
                msi_capability.message_address_high,
            );
            PCI_MSI_MESSAGE_DATA_64_OFFSET
        } else {
            PCI_MSI_MESSAGE_DATA_32_OFFSET
        };

        self.pci_write16(
            cap_ptr as u32 + message_data_offset,
            msi_capability.message_data,
        );

        if actual_vectors == 1 {
            msi_capability.enable_single_vector();
        } else {
            //TODO: mme (multiple vectors) support
            msi_capability.enable_single_vector();
        }

        self.pci_write16(
            cap_ptr as u32 + PCI_MSI_MESSAGE_CONTROL_OFFSET,
            msi_capability.message_control,
        );

        Ok(MsiConfiguration {
            capability: msi_capability,
            vectors: allocated_vectors,
        })
    }
}

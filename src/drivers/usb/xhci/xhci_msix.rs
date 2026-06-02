#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
use crate::drivers::pci::pci_device::PciDeviceHeader;
use crate::drivers::pci::pci_io::pci_write16;
use crate::interrupts::vector::InterruptVector;
use crate::vgaprintln;
use core::ptr;

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

    //Number of MSI-X vectors supported
    pub fn table_size(&self) -> u16 {
        (self.message_control & 0x07FF) + 1
    }

    //Is MSI-X enabled?
    pub fn enabled(&self) -> bool {
        (self.message_control & (1 << 15)) != 0
    }

    //Enable MSI-X
    pub fn enable(&mut self) {
        self.message_control |= 1 << 15;
    }

    //Mask all MSI-X vectors
    pub fn mask_all(&mut self) {
        self.message_control |= 1 << 14;
    }

    //Unmask all MSI-X vectors
    pub fn unmask_all(&mut self) {
        self.message_control &= !(1 << 14);
    }

    //BAR index of the MSI-X table
    pub fn table_bir(&self) -> u8 {
        (self.table & 0x7) as u8
    }

    //Offset of MSI-X table in BAR
    pub fn table_offset(&self) -> u32 {
        self.table & !0x7
    }

    //BAR index of the PBA
    pub fn pba_bir(&self) -> u8 {
        (self.pba & 0x7) as u8
    }

    //Offset of the PBA in BAR
    pub fn pba_offset(&self) -> u32 {
        self.pba & !0x7
    }

    pub fn write_back(&self, pci: &PciDeviceHeader, cap_ptr: u8) {
        // Message Control is at offset +2
        pci_write16(pci.base_id(), (cap_ptr as u32) + 0x02, self.message_control);
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

        vgaprintln!("MSI-X Capability:");
        vgaprintln!("  Cap ID          : 0x{:02X}", self.cap_id);
        vgaprintln!("  Next Pointer    : 0x{:02X}", self.next);
        vgaprintln!("  Message Control : 0x{:04X}", msg_control);
        vgaprintln!("    Table Size    : {}", table_size);
        vgaprintln!("    Function Mask : {}", function_mask);
        vgaprintln!("    MSI-X Enabled : {}", msix_enabled);
        vgaprintln!("  Table:");
        vgaprintln!("    BIR           : {}", table_bir);
        vgaprintln!("    Offset        : 0x{:08X}", table_offset);
        vgaprintln!("  PBA:");
        vgaprintln!("    BIR           : {}", pba_bir);
        vgaprintln!("    Offset        : 0x{:08X}", pba_offset);
    }
}

//=======================================================
//      MSI-X TABLE ENTRY
//=======================================================
#[repr(C)]
pub struct MsixTableEntry {
    pub(crate) msg_addr_low: u32,
    pub(crate) msg_addr_high: u32,
    pub(crate) msg_data: u32,
    pub(crate) vector_ctrl: u32,
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
    pub(crate) fn new(base: *mut MsixTableEntry) -> Self {
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

            vgaprintln!(
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
    pub(crate) fn new(pba_bar_base: *mut u8, pba_offset: u32, vectors: usize) -> Self {
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
}

pub enum XhciInterruptConfig {
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

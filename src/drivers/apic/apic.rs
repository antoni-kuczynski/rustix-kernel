#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 30/05/2026
 */
use crate::{print_ok_msg, vgaprintln, VGAWRITER};
use crate::ColorTextMode;
use core::ops::Add;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Once;
use x86_64::PhysAddr;
use x86_64::structures::idt::InterruptStackFrame;
use crate::asm::{outb, rdmsr};
use crate::boot::cpuid::CpuId;
use crate::drivers::acpi::acpi_tables::{acpi_get_sdt_table, ACPISignature};
use crate::drivers::acpi::tables::madt::Madt;
use crate::drivers::apic::disable_pic;
use crate::memory::ioremap::{ioremap_permanent, IoAlloc};
use crate::memory::page_tables::PageSize;
use crate::{print_fail_msg, vgaprint};
use crate::drivers::apic::pit::_pit_wait_ms;
// ============================================================================
// Local APIC / xAPIC constants
// ============================================================================

// MSR
pub const IA32_APIC_BASE_MSR: u32 = 0x1B;

pub const IA32_APIC_BASE_BSP: u64 = 1 << 8;
pub const IA32_APIC_BASE_X2APIC_ENABLE: u64 = 1 << 10;
pub const IA32_APIC_BASE_APIC_ENABLE: u64 = 1 << 11;
pub const IA32_APIC_BASE_ADDR_MASK: u64 = 0xFFFF_F000;

// MMIO
pub const LAPIC_DEFAULT_PHYS_BASE: u64 = 0xFEE0_0000;
pub const LAPIC_MMIO_SIZE: u64 = 0x1000;

// ============================================================================
// Local APIC register offsets
// ============================================================================

pub const LAPIC_ID: usize = 0x020;
pub const LAPIC_VERSION: usize = 0x030;

pub const LAPIC_TPR: usize = 0x080;       // Task Priority Register
pub const LAPIC_APR: usize = 0x090;       // Arbitration Priority Register
pub const LAPIC_PPR: usize = 0x0A0;       // Processor Priority Register
pub const LAPIC_EOI: usize = 0x0B0;       // End Of Interrupt
pub const LAPIC_RRD: usize = 0x0C0;       // Remote Read Register
pub const LAPIC_LDR: usize = 0x0D0;       // Logical Destination Register
pub const LAPIC_DFR: usize = 0x0E0;       // Destination Format Register
pub const LAPIC_SVR: usize = 0x0F0;       // Spurious Interrupt Vector Register

pub const LAPIC_ISR_BASE: usize = 0x100;  // In-Service Register, 0x100-0x170
pub const LAPIC_TMR_BASE: usize = 0x180;  // Trigger Mode Register, 0x180-0x1F0
pub const LAPIC_IRR_BASE: usize = 0x200;  // Interrupt Request Register, 0x200-0x270

pub const LAPIC_ESR: usize = 0x280;       // Error Status Register

pub const LAPIC_ICR_LOW: usize = 0x300;   // Interrupt Command Register low
pub const LAPIC_ICR_HIGH: usize = 0x310;  // Interrupt Command Register high

pub const LAPIC_LVT_TIMER: usize = 0x320;
pub const LAPIC_LVT_THERMAL: usize = 0x330;
pub const LAPIC_LVT_PERF: usize = 0x340;
pub const LAPIC_LVT_LINT0: usize = 0x350;
pub const LAPIC_LVT_LINT1: usize = 0x360;
pub const LAPIC_LVT_ERROR: usize = 0x370;

pub const LAPIC_TIMER_INITIAL_COUNT: usize = 0x380;
pub const LAPIC_TIMER_CURRENT_COUNT: usize = 0x390;
pub const LAPIC_TIMER_DIVIDE_CONFIG: usize = 0x3E0;

// ============================================================================
// Spurious Interrupt Vector Register
// ============================================================================
pub const LAPIC_SVR_VECTOR_MASK: u32 = 0xFF;
pub const LAPIC_SVR_APIC_ENABLE: u32 = 1 << 8;
pub const LAPIC_SVR_FOCUS_PROCESSOR_CHECKING_DISABLE: u32 = 1 << 9;
pub const LAPIC_SVR_EOI_BROADCAST_SUPPRESSION: u32 = 1 << 12;
pub const LAPIC_SPURIOUS_VECTOR: u8 = 0xFF;


// ============================================================================
// LVT common bits
// ============================================================================

pub const LAPIC_LVT_VECTOR_MASK: u32 = 0xFF;

pub const LAPIC_LVT_DELIVERY_FIXED: u32 = 0b000 << 8;
pub const LAPIC_LVT_DELIVERY_SMI: u32 = 0b010 << 8;
pub const LAPIC_LVT_DELIVERY_NMI: u32 = 0b100 << 8;
pub const LAPIC_LVT_DELIVERY_INIT: u32 = 0b101 << 8;
pub const LAPIC_LVT_DELIVERY_EXTINT: u32 = 0b111 << 8;

pub const LAPIC_LVT_DELIVERY_STATUS: u32 = 1 << 12;
pub const LAPIC_LVT_POLARITY_LOW: u32 = 1 << 13;
pub const LAPIC_LVT_REMOTE_IRR: u32 = 1 << 14;
pub const LAPIC_LVT_TRIGGER_LEVEL: u32 = 1 << 15;
pub const LAPIC_LVT_MASKED: u32 = 1 << 16;
pub const LAPIC_LVT_UNMASKED: u32 = 0 << 16;


// ============================================================================
// Local APIC Timer
// ============================================================================

// Timer mode bits in LVT Timer register: bits 17..18
pub const LAPIC_TIMER_MODE_ONE_SHOT: u32 = 0b00 << 17;
pub const LAPIC_TIMER_MODE_PERIODIC: u32 = 0b01 << 17;
pub const LAPIC_TIMER_MODE_TSC_DEADLINE: u32 = 0b10 << 17;

// Divide Configuration Register values
pub const LAPIC_TIMER_DIVIDE_BY_2: u32 = 0x0;
pub const LAPIC_TIMER_DIVIDE_BY_4: u32 = 0x1;
pub const LAPIC_TIMER_DIVIDE_BY_8: u32 = 0x2;
pub const LAPIC_TIMER_DIVIDE_BY_16: u32 = 0x3;
pub const LAPIC_TIMER_DIVIDE_BY_32: u32 = 0x8;
pub const LAPIC_TIMER_DIVIDE_BY_64: u32 = 0x9;
pub const LAPIC_TIMER_DIVIDE_BY_128: u32 = 0xA;
pub const LAPIC_TIMER_DIVIDE_BY_1: u32 = 0xB;
pub const LAPIC_TIMER_VECTOR: u8 = 0x40;

// ============================================================================
// Interrupt Command Register - ICR
// ============================================================================

// ICR low bits
pub const LAPIC_ICR_VECTOR_MASK: u32 = 0xFF;

pub const LAPIC_ICR_DELIVERY_FIXED: u32 = 0b000 << 8;
pub const LAPIC_ICR_DELIVERY_LOWEST_PRIORITY: u32 = 0b001 << 8;
pub const LAPIC_ICR_DELIVERY_SMI: u32 = 0b010 << 8;
pub const LAPIC_ICR_DELIVERY_NMI: u32 = 0b100 << 8;
pub const LAPIC_ICR_DELIVERY_INIT: u32 = 0b101 << 8;
pub const LAPIC_ICR_DELIVERY_STARTUP: u32 = 0b110 << 8;

pub const LAPIC_ICR_DEST_MODE_PHYSICAL: u32 = 0 << 11;
pub const LAPIC_ICR_DEST_MODE_LOGICAL: u32 = 1 << 11;

pub const LAPIC_ICR_DELIVERY_STATUS: u32 = 1 << 12;

pub const LAPIC_ICR_LEVEL_DEASSERT: u32 = 0 << 14;
pub const LAPIC_ICR_LEVEL_ASSERT: u32 = 1 << 14;

pub const LAPIC_ICR_TRIGGER_EDGE: u32 = 0 << 15;
pub const LAPIC_ICR_TRIGGER_LEVEL: u32 = 1 << 15;

pub const LAPIC_ICR_DEST_SHORTHAND_NONE: u32 = 0b00 << 18;
pub const LAPIC_ICR_DEST_SHORTHAND_SELF: u32 = 0b01 << 18;
pub const LAPIC_ICR_DEST_SHORTHAND_ALL_INCLUDING_SELF: u32 = 0b10 << 18;
pub const LAPIC_ICR_DEST_SHORTHAND_ALL_EXCLUDING_SELF: u32 = 0b11 << 18;

// ICR high bits
pub const LAPIC_ICR_DESTINATION_SHIFT: u32 = 24;

pub const TIMER_HZ: u64 = 100;
pub const TIMER_PERIOD_MS: u64 = 1000 / TIMER_HZ;

pub const LAPIC_START_COUNT: u32 = 10_000_000;

pub const LAPIC_SPURIOUS_VECTOR_IDT_INDEX: u8 = 0xFF;
pub const LAPIC_ERROR_VECTOR: u8 = 0xFE;

pub struct Apic {
    phys_addr: PhysAddr,
    mmio_mapping: IoAlloc,
    ticks_per_ms: u64
}


impl Apic {
    unsafe fn new(phys_addr: PhysAddr) -> Apic {
        let mmio_mapping = ioremap_permanent(phys_addr, PageSize::SIZE_4KB, 16);

        if mmio_mapping.virt_addr.is_null() {
            panic!("mmio mapping for APIC failed!");
        }

        Apic {
            phys_addr,
            mmio_mapping,
            ticks_per_ms: 0
        }
    }

    unsafe fn enable(&mut self) {
        disable_pic();

        //set idt index and enable apic
        let mut val = self.lapic_read(LAPIC_SPURIOUS_VECTOR as usize);
        val &= LAPIC_SVR_VECTOR_MASK;
        val |= LAPIC_SPURIOUS_VECTOR_IDT_INDEX as u32;
        val |= LAPIC_SVR_APIC_ENABLE;

        self.lapic_write(LAPIC_TPR, 0);

        //mask all for now
        self.lapic_write(LAPIC_LVT_TIMER, LAPIC_LVT_MASKED | LAPIC_TIMER_VECTOR as u32);
        self.lapic_write(LAPIC_LVT_THERMAL, LAPIC_LVT_MASKED);
        self.lapic_write(LAPIC_LVT_PERF, LAPIC_LVT_MASKED);
        self.lapic_write(LAPIC_LVT_LINT0, LAPIC_LVT_MASKED);
        self.lapic_write(LAPIC_LVT_LINT1, LAPIC_LVT_MASKED);

        self.lapic_write(LAPIC_LVT_ERROR, LAPIC_ERROR_VECTOR as u32);
        self.lapic_write(LAPIC_SPURIOUS_VECTOR as usize, val);

        self.lapic_write(
            LAPIC_SVR,
            LAPIC_SVR_APIC_ENABLE | LAPIC_SPURIOUS_VECTOR_IDT_INDEX as u32,
        );

        //TIMER
        let calibration_ms = 50;

        self.lapic_write(LAPIC_TIMER_DIVIDE_CONFIG, LAPIC_TIMER_DIVIDE_BY_16);
        self.lapic_write(
            LAPIC_LVT_TIMER,
            LAPIC_TIMER_VECTOR as u32 | LAPIC_TIMER_MODE_ONE_SHOT | LAPIC_LVT_MASKED,
        );        self.lapic_write(LAPIC_TIMER_INITIAL_COUNT, LAPIC_START_COUNT);

        _pit_wait_ms(calibration_ms);

        let elapsed = LAPIC_START_COUNT - self.lapic_read(LAPIC_TIMER_CURRENT_COUNT);
        let ticks_per_ms = elapsed / calibration_ms as u32;

        self.ticks_per_ms = ticks_per_ms as u64;

        let period_ms = 1000 / TIMER_HZ;
        let initial_count = ticks_per_ms * period_ms as u32;


        self.lapic_write(LAPIC_TIMER_DIVIDE_CONFIG, LAPIC_TIMER_DIVIDE_BY_16);
        self.lapic_write(LAPIC_LVT_TIMER, LAPIC_TIMER_VECTOR as u32 | LAPIC_TIMER_MODE_PERIODIC);
        self.lapic_write(LAPIC_TIMER_INITIAL_COUNT, initial_count);
    }

    #[inline]
    pub unsafe fn lapic_write(&self, reg: usize, value: u32) {
        write_volatile(self.mmio_mapping.virt_addr.add(reg as u64).as_u64() as *mut u32, value);
    }

    #[inline]
    pub unsafe fn lapic_read(&self, reg: usize) -> u32 {
        read_volatile(self.mmio_mapping.virt_addr.add(reg as u64).as_u64() as *mut u32)
    }

    #[inline]
    pub unsafe fn eoi(&self) {
        self.lapic_write(LAPIC_EOI, 0);
    }
    #[inline]
    pub fn lapic_ticks_to_ms(&self, ticks: u64) -> u64 {
        ticks / self.ticks_per_ms
    }

    #[inline]
    pub fn ms_to_lapic_ticks(&self, ms: u64) -> u64 {
        ms * self.ticks_per_ms
    }
}



pub fn apic_bsp_init() {
    vgaprint!("Initializng APIC for BSP...");
    if !CpuId::has_apic() {
        print_fail_msg!();
        panic!(" [APIC] Apic is not present on the system!");
    }

    unsafe {
        //todo: uncomment
        
        // let addr = acpi_get_sdt_table(ACPISignature::MADT).expect("No MADT found on the system!");
        // let madt = Madt::new_from_virt_addr(addr);

        let msr_addr = rdmsr(IA32_APIC_BASE_MSR) & IA32_APIC_BASE_ADDR_MASK;
        // let madt_addr = madt.parse().local_apic_physical_address;
        //
        // if msr_addr != madt_addr {
        //     print_fail_msg!();
        //     vgaprintln!("MSR: {:#011x} | MADT: {:#011x}", msr_addr, madt_addr);
        //     panic!(" [APIC] Apic base address mismatch (MADT / MSR)!");
        // }

        let mut apic = Apic::new(PhysAddr::new(msr_addr));
        apic.enable();
        LAPIC.call_once(|| apic);

    }
    print_ok_msg!();
}

pub static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);
pub static LAPIC: Once<Apic> = Once::new();

pub extern "x86-interrupt" fn apic_spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    vgaprintln!("NOT IMPLEMENTED YET - spurious vector");
}

pub extern "x86-interrupt" fn apic_error_interrupt_handler(_stack_frame: InterruptStackFrame) {
    vgaprintln!("NOT IMPLEMENTED YET - error vector");
}

pub extern "x86-interrupt" fn lapic_timer_interrupt_handler(
    _stack_frame: InterruptStackFrame,
) {
    let ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed) + 1;

    unsafe {
        let lapic = LAPIC.get().expect("lapic not initialized");
        lapic.eoi();
    }
}

pub fn timer_lapic_uptime_ms() -> u64 {
    TIMER_TICKS.load(Ordering::Relaxed) * TIMER_PERIOD_MS
}

pub fn timer_lapic_sleep(ms: u64) {
    let start = timer_lapic_uptime_ms();

    while timer_lapic_uptime_ms().wrapping_sub(start) < ms {
        core::hint::spin_loop();
    }
}


use core::arch::asm;

//  **ASM FUNCTIONS**
#[inline(always)]
pub unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in ("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    unsafe {
        let value: u8;
        asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
        );
        value
    }
}

#[inline(always)]
pub unsafe fn inw(port: u16) -> u16 {
    unsafe {
        let value: u16;
        asm!(
        "in ax, dx",
        in("dx") port,
        out("ax") value,
        options(nomem, nostack, preserves_flags),
        );
        value
    }
}

#[inline(always)]
pub unsafe fn outl(port: u16, value: u32) {
    unsafe {
        asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn inl(port: u16) -> u32 {
    unsafe {
        let value: u32;
        asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") value,
        options(nomem, nostack, preserves_flags)
        );
        value
    }
}

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    unsafe {
        let low: u32;
        let high: u32;

        asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack, preserves_flags),
        );

        ((high as u64) << 32) | (low as u64)
    }
}

#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    unsafe {
        let low = value as u32;
        let high = (value >> 32) as u32;

        asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack, preserves_flags),
        );
    }
}
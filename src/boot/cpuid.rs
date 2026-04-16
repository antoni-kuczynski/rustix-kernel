/*
 * Created by Antoni Kuczyński
 * 16/04/2026
 */
use core::arch::x86_64::{CpuidResult, __cpuid};
use spin::Once;

pub struct CpuId {
    base: CpuidResult,
    extended: CpuidResult,
}

impl CpuId {
    #[inline(always)]
    fn base() -> &'static CpuidResult {
        &CPU_ID.get().expect("CpuId not initialized! Call cpuid_init() first.").base
    }

    #[inline(always)]
    fn ext() -> &'static CpuidResult {
        &CPU_ID.get().expect("CpuId not initialized! Call cpuid_init() first.").extended
    }

    // ==========================================
    //      EAX (version information)
    // ==========================================
    pub fn stepping_id() -> u8 {
        (Self::base().eax & 0xF) as u8
    }

    pub fn model() -> u8 {
        ((Self::base().eax >> 4) & 0xF) as u8
    }

    pub fn family_id() -> u8 {
        ((Self::base().eax >> 8) & 0xF) as u8
    }

    pub fn processor_type() -> u8 {
        ((Self::base().eax >> 12) & 0x3) as u8
    }

    pub fn extended_model_id() -> u8 {
        ((Self::base().eax >> 16) & 0xF) as u8
    }

    pub fn extended_family_id() -> u8 {
        ((Self::base().eax >> 20) & 0xFF) as u8
    }

    pub fn actual_family() -> u32 {
        let family = Self::family_id() as u32;
        if family == 0x0F {
            family + Self::extended_family_id() as u32
        } else {
            family
        }
    }

    pub fn actual_model() -> u32 {
        let family = Self::family_id();
        let model = Self::model() as u32;

        if family == 0x06 || family == 0x0F {
            ((Self::extended_model_id() as u32) << 4) | model
        } else {
            model
        }
    }

    // ==========================================
    //          EBX
    // ==========================================
    pub fn brand_index() -> u8 {
        (Self::base().ebx & 0xFF) as u8
    }

    pub fn clflush_line_size() -> u8 {
        ((Self::base().ebx >> 8) & 0xFF) as u8
    }

    pub fn apic_id_space() -> u8 {
        ((Self::base().ebx >> 16) & 0xFF) as u8
    }

    pub fn initial_apic_id() -> u8 {
        ((Self::base().ebx >> 24) & 0xFF) as u8
    }

    // ==========================================
    //          ECX
    // ==========================================
    pub fn has_sse3() -> bool { (Self::base().ecx & (1 << 0)) != 0 }
    pub fn has_pclmul() -> bool { (Self::base().ecx & (1 << 1)) != 0 }
    pub fn has_dtes64() -> bool { (Self::base().ecx & (1 << 2)) != 0 }
    pub fn has_monitor() -> bool { (Self::base().ecx & (1 << 3)) != 0 }
    pub fn has_ds_cpl() -> bool { (Self::base().ecx & (1 << 4)) != 0 }
    pub fn has_vmx() -> bool { (Self::base().ecx & (1 << 5)) != 0 }
    pub fn has_smx() -> bool { (Self::base().ecx & (1 << 6)) != 0 }
    pub fn has_est() -> bool { (Self::base().ecx & (1 << 7)) != 0 }
    pub fn has_tm2() -> bool { (Self::base().ecx & (1 << 8)) != 0 }
    pub fn has_ssse3() -> bool { (Self::base().ecx & (1 << 9)) != 0 }
    pub fn has_cid() -> bool { (Self::base().ecx & (1 << 10)) != 0 }
    pub fn has_sdbg() -> bool { (Self::base().ecx & (1 << 11)) != 0 }
    pub fn has_fma() -> bool { (Self::base().ecx & (1 << 12)) != 0 }
    pub fn has_cx16() -> bool { (Self::base().ecx & (1 << 13)) != 0 }
    pub fn has_xtpr() -> bool { (Self::base().ecx & (1 << 14)) != 0 }
    pub fn has_pdcm() -> bool { (Self::base().ecx & (1 << 15)) != 0 }
    pub fn has_pcid() -> bool { (Self::base().ecx & (1 << 17)) != 0 }
    pub fn has_dca() -> bool { (Self::base().ecx & (1 << 18)) != 0 }
    pub fn has_sse4_1() -> bool { (Self::base().ecx & (1 << 19)) != 0 }
    pub fn has_sse4_2() -> bool { (Self::base().ecx & (1 << 20)) != 0 }
    pub fn has_x2apic() -> bool { (Self::base().ecx & (1 << 21)) != 0 }
    pub fn has_movbe() -> bool { (Self::base().ecx & (1 << 22)) != 0 }
    pub fn has_popcnt() -> bool { (Self::base().ecx & (1 << 23)) != 0 }
    pub fn has_tsc_deadline() -> bool { (Self::base().ecx & (1 << 24)) != 0 }
    pub fn has_aes() -> bool { (Self::base().ecx & (1 << 25)) != 0 }
    pub fn has_xsave() -> bool { (Self::base().ecx & (1 << 26)) != 0 }
    pub fn has_osxsave() -> bool { (Self::base().ecx & (1 << 27)) != 0 }
    pub fn has_avx() -> bool { (Self::base().ecx & (1 << 28)) != 0 }
    pub fn has_f16c() -> bool { (Self::base().ecx & (1 << 29)) != 0 }
    pub fn has_rdrand() -> bool { (Self::base().ecx & (1 << 30)) != 0 }
    pub fn has_hypervisor() -> bool { (Self::base().ecx & (1 << 31)) != 0 }

    // ==========================================
    //          EDX
    // ==========================================
    pub fn has_fpu() -> bool { (Self::base().edx & (1 << 0)) != 0 }
    pub fn has_vme() -> bool { (Self::base().edx & (1 << 1)) != 0 }
    pub fn has_de() -> bool { (Self::base().edx & (1 << 2)) != 0 }
    pub fn has_pse() -> bool { (Self::base().edx & (1 << 3)) != 0 }
    pub fn has_tsc() -> bool { (Self::base().edx & (1 << 4)) != 0 }
    pub fn has_msr() -> bool { (Self::base().edx & (1 << 5)) != 0 }
    pub fn has_pae() -> bool { (Self::base().edx & (1 << 6)) != 0 }
    pub fn has_mce() -> bool { (Self::base().edx & (1 << 7)) != 0 }
    pub fn has_cx8() -> bool { (Self::base().edx & (1 << 8)) != 0 }
    pub fn has_apic() -> bool { (Self::base().edx & (1 << 9)) != 0 }
    pub fn has_sep() -> bool { (Self::base().edx & (1 << 11)) != 0 }
    pub fn has_mtrr() -> bool { (Self::base().edx & (1 << 12)) != 0 }
    pub fn has_pge() -> bool { (Self::base().edx & (1 << 13)) != 0 }
    pub fn has_mca() -> bool { (Self::base().edx & (1 << 14)) != 0 }
    pub fn has_cmov() -> bool { (Self::base().edx & (1 << 15)) != 0 }
    pub fn has_pat() -> bool { (Self::base().edx & (1 << 16)) != 0 }
    pub fn has_pse36() -> bool { (Self::base().edx & (1 << 17)) != 0 }
    pub fn has_psn() -> bool { (Self::base().edx & (1 << 18)) != 0 }
    pub fn has_clflush() -> bool { (Self::base().edx & (1 << 19)) != 0 }
    pub fn has_ds() -> bool { (Self::base().edx & (1 << 21)) != 0 }
    pub fn has_acpi() -> bool { (Self::base().edx & (1 << 22)) != 0 }
    pub fn has_mmx() -> bool { (Self::base().edx & (1 << 23)) != 0 }
    pub fn has_fxsr() -> bool { (Self::base().edx & (1 << 24)) != 0 }
    pub fn has_sse() -> bool { (Self::base().edx & (1 << 25)) != 0 }
    pub fn has_sse2() -> bool { (Self::base().edx & (1 << 26)) != 0 }
    pub fn has_ss() -> bool { (Self::base().edx & (1 << 27)) != 0 }
    pub fn has_htt() -> bool { (Self::base().edx & (1 << 28)) != 0 }
    pub fn has_tm() -> bool { (Self::base().edx & (1 << 29)) != 0 }
    pub fn has_ia64() -> bool { (Self::base().edx & (1 << 30)) != 0 }
    pub fn has_pbe() -> bool { (Self::base().edx & (1 << 31)) != 0 }

    // ==========================================
    // EXTENDED FEATURES (EAX = 0x8000_0001)
    // ==========================================
    pub fn has_pdpe1gb() -> bool {
        (Self::ext().edx & (1 << 26)) != 0
    }
}

pub fn cpuid_init() {
    unsafe {
        let base_result = __cpuid(0x01); // Podstawowe flagi
        let extended_result = __cpuid(0x8000_0001); // Rozszerzone flagi (w tym pdpe1gb)

        CPU_ID.call_once(|| CpuId {
            base: base_result,
            extended: extended_result,
        });
    }
}

pub static CPU_ID: Once<CpuId> = Once::new();

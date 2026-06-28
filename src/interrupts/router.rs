#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
use crate::drivers::apic::apic::LAPIC;
use crate::interrupts::install_dynamic_idt_route;
use crate::interrupts::vector::{IDT_VECTOR_COUNT, InterruptVector};
use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::kprintln;

pub type RoutedInterruptHandler = fn(InterruptVector, InterruptStackFrame);
pub type RoutedInterruptHandlerWithContext = fn(InterruptVector, InterruptStackFrame, usize);

static ROUTES: [AtomicUsize; IDT_VECTOR_COUNT] = [const { AtomicUsize::new(0) }; IDT_VECTOR_COUNT];
static ROUTE_CONTEXTS: [AtomicUsize; IDT_VECTOR_COUNT] =
    [const { AtomicUsize::new(0) }; IDT_VECTOR_COUNT];

pub fn register_handler(vector: InterruptVector, handler: RoutedInterruptHandler) -> bool {
    register_handler_with_context(vector, handler_without_context, handler as usize)
}

pub fn register_handler_with_context(
    vector: InterruptVector,
    handler: RoutedInterruptHandlerWithContext,
    context: usize,
) -> bool {
    if !install_dynamic_idt_route(vector) {
        return false;
    }

    ROUTE_CONTEXTS[vector.as_u8() as usize].store(context, Ordering::Release);

    let registered = ROUTES[vector.as_u8() as usize]
        .compare_exchange(0, handler as usize, Ordering::AcqRel, Ordering::Acquire)
        .is_ok();

    if !registered {
        ROUTE_CONTEXTS[vector.as_u8() as usize].store(0, Ordering::Release);
    }

    registered
}

pub fn unregister_handler(vector: InterruptVector) {
    ROUTES[vector.as_u8() as usize].store(0, Ordering::Release);
    ROUTE_CONTEXTS[vector.as_u8() as usize].store(0, Ordering::Release);
}

fn handler_without_context(
    vector: InterruptVector,
    stack_frame: InterruptStackFrame,
    handler: usize,
) {
    let handler: RoutedInterruptHandler = unsafe { core::mem::transmute(handler) };
    handler(vector, stack_frame);
}

fn dispatch(vector_raw: u8, stack_frame: InterruptStackFrame) {
    let Some(vector) = InterruptVector::new(vector_raw) else {
        return;
    };

    let handler = ROUTES[vector.as_u8() as usize].load(Ordering::Acquire);
    if handler == 0 {
        kprintln!(Warn, "Unhandled interrupt vector {:#04x}", vector_raw);
        unsafe {
            if let Some(lapic) = LAPIC.get() {
                lapic.eoi();
            }
        }
        return;
    }

    let context = ROUTE_CONTEXTS[vector.as_u8() as usize].load(Ordering::Acquire);
    let handler: RoutedInterruptHandlerWithContext = unsafe { core::mem::transmute(handler) };
    handler(vector, stack_frame, context);
}

macro_rules! vector_handler {
    ($name:ident, $vector:literal) => {
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            dispatch($vector, stack_frame);
        }
    };
}

macro_rules! define_vector_router {
    ($(($vector:literal, $name:ident)),+ $(,)?) => {
        $(vector_handler!($name, $vector);)+

        pub fn install_idt_route(
            idt: &mut InterruptDescriptorTable,
            vector: InterruptVector,
        ) -> bool {
            match vector.as_u8() {
                $(
                    $vector => {
                        idt[$vector].set_handler_fn($name);
                        true
                    }
                ),+
                _ => false,
            }
        }
    };
}

define_vector_router!(
    (0x20, vector_20_handler),
    (0x21, vector_21_handler),
    (0x22, vector_22_handler),
    (0x23, vector_23_handler),
    (0x24, vector_24_handler),
    (0x25, vector_25_handler),
    (0x26, vector_26_handler),
    (0x27, vector_27_handler),
    (0x28, vector_28_handler),
    (0x29, vector_29_handler),
    (0x2A, vector_2a_handler),
    (0x2B, vector_2b_handler),
    (0x2C, vector_2c_handler),
    (0x2D, vector_2d_handler),
    (0x2E, vector_2e_handler),
    (0x2F, vector_2f_handler),
    (0x30, vector_30_handler),
    (0x31, vector_31_handler),
    (0x32, vector_32_handler),
    (0x33, vector_33_handler),
    (0x34, vector_34_handler),
    (0x35, vector_35_handler),
    (0x36, vector_36_handler),
    (0x37, vector_37_handler),
    (0x38, vector_38_handler),
    (0x39, vector_39_handler),
    (0x3A, vector_3a_handler),
    (0x3B, vector_3b_handler),
    (0x3C, vector_3c_handler),
    (0x3D, vector_3d_handler),
    (0x3E, vector_3e_handler),
    (0x3F, vector_3f_handler),
    (0x41, vector_41_handler),
    (0x42, vector_42_handler),
    (0x43, vector_43_handler),
    (0x44, vector_44_handler),
    (0x45, vector_45_handler),
    (0x46, vector_46_handler),
    (0x47, vector_47_handler),
    (0x48, vector_48_handler),
    (0x49, vector_49_handler),
    (0x4A, vector_4a_handler),
    (0x4B, vector_4b_handler),
    (0x4C, vector_4c_handler),
    (0x4D, vector_4d_handler),
    (0x4E, vector_4e_handler),
    (0x4F, vector_4f_handler),
    (0x50, vector_50_handler),
    (0x51, vector_51_handler),
    (0x52, vector_52_handler),
    (0x53, vector_53_handler),
    (0x54, vector_54_handler),
    (0x55, vector_55_handler),
    (0x56, vector_56_handler),
    (0x57, vector_57_handler),
    (0x58, vector_58_handler),
    (0x59, vector_59_handler),
    (0x5A, vector_5a_handler),
    (0x5B, vector_5b_handler),
    (0x5C, vector_5c_handler),
    (0x5D, vector_5d_handler),
    (0x5E, vector_5e_handler),
    (0x5F, vector_5f_handler),
    (0x60, vector_60_handler),
    (0x61, vector_61_handler),
    (0x62, vector_62_handler),
    (0x63, vector_63_handler),
    (0x64, vector_64_handler),
    (0x65, vector_65_handler),
    (0x66, vector_66_handler),
    (0x67, vector_67_handler),
    (0x68, vector_68_handler),
    (0x69, vector_69_handler),
    (0x6A, vector_6a_handler),
    (0x6B, vector_6b_handler),
    (0x6C, vector_6c_handler),
    (0x6D, vector_6d_handler),
    (0x6E, vector_6e_handler),
    (0x6F, vector_6f_handler),
    (0x70, vector_70_handler),
    (0x71, vector_71_handler),
    (0x72, vector_72_handler),
    (0x73, vector_73_handler),
    (0x74, vector_74_handler),
    (0x75, vector_75_handler),
    (0x76, vector_76_handler),
    (0x77, vector_77_handler),
    (0x78, vector_78_handler),
    (0x79, vector_79_handler),
    (0x7A, vector_7a_handler),
    (0x7B, vector_7b_handler),
    (0x7C, vector_7c_handler),
    (0x7D, vector_7d_handler),
    (0x7E, vector_7e_handler),
    (0x7F, vector_7f_handler),
    (0x80, vector_80_handler),
    (0x81, vector_81_handler),
    (0x82, vector_82_handler),
    (0x83, vector_83_handler),
    (0x84, vector_84_handler),
    (0x85, vector_85_handler),
    (0x86, vector_86_handler),
    (0x87, vector_87_handler),
    (0x88, vector_88_handler),
    (0x89, vector_89_handler),
    (0x8A, vector_8a_handler),
    (0x8B, vector_8b_handler),
    (0x8C, vector_8c_handler),
    (0x8D, vector_8d_handler),
    (0x8E, vector_8e_handler),
    (0x8F, vector_8f_handler),
    (0x90, vector_90_handler),
    (0x91, vector_91_handler),
    (0x92, vector_92_handler),
    (0x93, vector_93_handler),
    (0x94, vector_94_handler),
    (0x95, vector_95_handler),
    (0x96, vector_96_handler),
    (0x97, vector_97_handler),
    (0x98, vector_98_handler),
    (0x99, vector_99_handler),
    (0x9A, vector_9a_handler),
    (0x9B, vector_9b_handler),
    (0x9C, vector_9c_handler),
    (0x9D, vector_9d_handler),
    (0x9E, vector_9e_handler),
    (0x9F, vector_9f_handler),
    (0xA0, vector_a0_handler),
    (0xA1, vector_a1_handler),
    (0xA2, vector_a2_handler),
    (0xA3, vector_a3_handler),
    (0xA4, vector_a4_handler),
    (0xA5, vector_a5_handler),
    (0xA6, vector_a6_handler),
    (0xA7, vector_a7_handler),
    (0xA8, vector_a8_handler),
    (0xA9, vector_a9_handler),
    (0xAA, vector_aa_handler),
    (0xAB, vector_ab_handler),
    (0xAC, vector_ac_handler),
    (0xAD, vector_ad_handler),
    (0xAE, vector_ae_handler),
    (0xAF, vector_af_handler),
    (0xB0, vector_b0_handler),
    (0xB1, vector_b1_handler),
    (0xB2, vector_b2_handler),
    (0xB3, vector_b3_handler),
    (0xB4, vector_b4_handler),
    (0xB5, vector_b5_handler),
    (0xB6, vector_b6_handler),
    (0xB7, vector_b7_handler),
    (0xB8, vector_b8_handler),
    (0xB9, vector_b9_handler),
    (0xBA, vector_ba_handler),
    (0xBB, vector_bb_handler),
    (0xBC, vector_bc_handler),
    (0xBD, vector_bd_handler),
    (0xBE, vector_be_handler),
    (0xBF, vector_bf_handler),
    (0xC0, vector_c0_handler),
    (0xC1, vector_c1_handler),
    (0xC2, vector_c2_handler),
    (0xC3, vector_c3_handler),
    (0xC4, vector_c4_handler),
    (0xC5, vector_c5_handler),
    (0xC6, vector_c6_handler),
    (0xC7, vector_c7_handler),
    (0xC8, vector_c8_handler),
    (0xC9, vector_c9_handler),
    (0xCA, vector_ca_handler),
    (0xCB, vector_cb_handler),
    (0xCC, vector_cc_handler),
    (0xCD, vector_cd_handler),
    (0xCE, vector_ce_handler),
    (0xCF, vector_cf_handler),
    (0xD0, vector_d0_handler),
    (0xD1, vector_d1_handler),
    (0xD2, vector_d2_handler),
    (0xD3, vector_d3_handler),
    (0xD4, vector_d4_handler),
    (0xD5, vector_d5_handler),
    (0xD6, vector_d6_handler),
    (0xD7, vector_d7_handler),
    (0xD8, vector_d8_handler),
    (0xD9, vector_d9_handler),
    (0xDA, vector_da_handler),
    (0xDB, vector_db_handler),
    (0xDC, vector_dc_handler),
    (0xDD, vector_dd_handler),
    (0xDE, vector_de_handler),
    (0xDF, vector_df_handler),
    (0xE0, vector_e0_handler),
    (0xE1, vector_e1_handler),
    (0xE2, vector_e2_handler),
    (0xE3, vector_e3_handler),
    (0xE4, vector_e4_handler),
    (0xE5, vector_e5_handler),
    (0xE6, vector_e6_handler),
    (0xE7, vector_e7_handler),
    (0xE8, vector_e8_handler),
    (0xE9, vector_e9_handler),
    (0xEA, vector_ea_handler),
    (0xEB, vector_eb_handler),
    (0xEC, vector_ec_handler),
    (0xED, vector_ed_handler),
    (0xEE, vector_ee_handler),
    (0xEF, vector_ef_handler),
    (0xF0, vector_f0_handler),
    (0xF1, vector_f1_handler),
    (0xF2, vector_f2_handler),
    (0xF3, vector_f3_handler),
    (0xF4, vector_f4_handler),
    (0xF5, vector_f5_handler),
    (0xF6, vector_f6_handler),
    (0xF7, vector_f7_handler),
    (0xF8, vector_f8_handler),
    (0xF9, vector_f9_handler),
    (0xFA, vector_fa_handler),
    (0xFB, vector_fb_handler),
    (0xFC, vector_fc_handler),
    (0xFD, vector_fd_handler)
);
//why did u scroll all the way down here? there's a lot more (shitty) code to look at.

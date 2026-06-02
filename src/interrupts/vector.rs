#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
use crate::drivers::apic::apic::{
    LAPIC_ERROR_VECTOR, LAPIC_SPURIOUS_VECTOR_IDT_INDEX, LAPIC_TIMER_VECTOR,
};
use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

pub const FIRST_EXTERNAL_VECTOR: u8 = 0x40;
pub const LAST_IDT_VECTOR: u8 = 0xFF;
pub const IDT_VECTOR_COUNT: usize = 256;

static NEXT_VECTOR: AtomicU8 = AtomicU8::new(FIRST_EXTERNAL_VECTOR);
static ALLOCATED_VECTORS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterruptVector(u8);

impl InterruptVector {
    pub const fn new(vector: u8) -> Option<Self> {
        if vector >= FIRST_EXTERNAL_VECTOR && !is_reserved_vector(vector) {
            Some(Self(vector))
        } else {
            None
        }
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }

    fn word_index(self) -> usize {
        (self.0 as usize) / 64
    }

    fn bit(self) -> u64 {
        1 << ((self.0 as usize) % 64)
    }
}

pub const fn is_reserved_vector(vector: u8) -> bool {
    vector < FIRST_EXTERNAL_VECTOR
        || vector == LAPIC_TIMER_VECTOR
        || vector == LAPIC_ERROR_VECTOR
        || vector == LAPIC_SPURIOUS_VECTOR_IDT_INDEX
}

fn next_vector_candidate() -> u8 {
    loop {
        let current = NEXT_VECTOR.load(Ordering::Acquire);
        let next = if current == LAST_IDT_VECTOR {
            FIRST_EXTERNAL_VECTOR
        } else {
            current + 1
        };

        if NEXT_VECTOR
            .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return current;
        }
    }
}

pub fn reserve_vector(vector: u8) -> Option<InterruptVector> {
    let vector = InterruptVector::new(vector)?;
    let word = &ALLOCATED_VECTORS[vector.word_index()];
    let bit = vector.bit();

    loop {
        let allocated = word.load(Ordering::Acquire);
        if allocated & bit != 0 {
            return None;
        }

        if word
            .compare_exchange(
                allocated,
                allocated | bit,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            return Some(vector);
        }
    }
}

pub fn allocate_vector() -> Option<InterruptVector> {
    for _ in FIRST_EXTERNAL_VECTOR..=LAST_IDT_VECTOR {
        let candidate = next_vector_candidate();

        if is_reserved_vector(candidate) {
            continue;
        }

        if let Some(vector) = reserve_vector(candidate) {
            return Some(vector);
        }
    }

    None
}

pub fn allocate_vectors<const N: usize>() -> Option<[InterruptVector; N]> {
    let mut vectors = [InterruptVector(0); N];
    let mut allocated = 0;

    while allocated < N {
        match allocate_vector() {
            Some(vector) => {
                vectors[allocated] = vector;
                allocated += 1;
            }
            None => {
                for vector in vectors.iter().take(allocated) {
                    free_vector(*vector);
                }
                return None;
            }
        }
    }

    Some(vectors)
}

pub fn free_vector(vector: InterruptVector) {
    ALLOCATED_VECTORS[vector.word_index()].fetch_and(!vector.bit(), Ordering::AcqRel);
}

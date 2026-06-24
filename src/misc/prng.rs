/*
 * Created by Antoni Kuczyński
 * 18/06/2026
 */
use lazy_static::lazy_static;
use spin::lock_api::Mutex;
use spin::Once;
use crate::boot::cpuid::CpuId;
use crate::{kprintln_failed, kprintln_ok};

//TODO: very primitive. fix
pub struct Prng {
    state: u32,
}

impl Prng {
    const fn new(seed: u32) -> Self {
        Self {
            state: 0
        }
    }

    fn init(&mut self) {
        let mut seed: u32 = 0;


        let result = if CpuId::has_rdseed() {
            unsafe { core::arch::x86_64::_rdseed32_step(&mut seed) }
        } else if CpuId::has_rdrand() {
            unsafe { core::arch::x86_64::_rdrand32_step(&mut seed) }
        } else {
            seed = unsafe { core::arch::x86_64::_rdtsc() as u32 };
            1
        };

        if result == 0 {
            kprintln_failed!("Initialized pseudo random number generator.");
            panic!("Failed to initialize pseudo random number generator");
        }

        self.state = seed;
        kprintln_ok!("Initialized pseudo random number generator.");
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    pub fn gen_range(&mut self, min: isize, max: isize) -> isize {
        if min >= max {
            return min;
        }

        let range = (max - min) as u32;
        let random_u32 = self.next_u32();
        let offset = random_u32 % range;

        min + offset as isize
    }
}

pub fn prng_next_isize(start: isize, end: isize) -> isize {
    PRNG.lock().gen_range(start, end)
}

pub fn prng_init() {
    PRNG.lock().init();
}

static PRNG: Mutex<Prng> = Mutex::new(Prng::new(0));
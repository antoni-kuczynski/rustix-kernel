#![allow(unused)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 31/05/2026
 */
use crate::asm::{inb, outb};

pub const PIT_FREQUENCY: u64 = 1_193_182;

pub const PIT_CHANNEL2: u16 = 0x42;
pub const PIT_COMMAND: u16 = 0x43;
pub const PIT_SPEAKER_PORT: u16 = 0x61;

// Channel 2, access lo/hi byte, mode 0, binary
pub const PIT_CMD_CHANNEL2_ONESHOT: u8 = 0b1011_0000;

// Port 0x61
pub const PIT_PORT61_GATE2: u8 = 1 << 0;
pub const PIT_PORT61_SPEAKER: u8 = 1 << 1;
pub const PIT_PORT61_OUT2: u8 = 1 << 5;



#[inline]
pub unsafe fn _pit_wait_ms(ms: u16) {
    assert!(ms > 0);
    assert!(ms <= 54);

    let count = ((PIT_FREQUENCY * ms as u64) / 1000) as u16;
    let count = if count == 0 { 1 } else { count };

    let old_port61 = inb(PIT_SPEAKER_PORT);

    outb(
        PIT_SPEAKER_PORT,
        old_port61 & !(PIT_PORT61_GATE2 | PIT_PORT61_SPEAKER),
    );

    outb(PIT_COMMAND, PIT_CMD_CHANNEL2_ONESHOT);
    outb(PIT_CHANNEL2, (count & 0xFF) as u8);
    outb(PIT_CHANNEL2, (count >> 8) as u8);

    outb(
        PIT_SPEAKER_PORT,
        (old_port61 & !PIT_PORT61_SPEAKER) | PIT_PORT61_GATE2,
    );

    while (inb(PIT_SPEAKER_PORT) & PIT_PORT61_OUT2) == 0 {
        core::hint::spin_loop();
    }

    outb(PIT_SPEAKER_PORT, old_port61);
}

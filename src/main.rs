#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod drivers;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {

    vgaprintln!("hello from {}",123);

    loop{}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    vgaprintln!("{}", _info);
    loop {}
}

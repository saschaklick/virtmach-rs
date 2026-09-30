#![no_main]
#![no_std]
#![feature(abi_avr_interrupt)]

use panic_halt as _;

#[avr_device::entry]
fn main() -> ! {
    let _dp = attiny_hal::Peripherals::take().unwrap();

    loop {}
}

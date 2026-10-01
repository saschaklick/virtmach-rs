#![no_main]
#![no_std]
#![feature(abi_avr_interrupt)]
#![feature(asm_experimental_arch)]

//! Runs the virtmach program blink.bin on an ATtiny85. The program stays in flash and the vm reads
//! it with LPM, see flash.rs, so its size only counts against the flash, not the 512 bytes of RAM.
//!
//! The interrupts are, in this order: proc, math, gpio, uart, i2c, time. Compile programs with the
//! same order, from the repository root:
//!
//!   cargo run --manifest-path sdk/Cargo.toml --bin basic -- examples/attiny/blink.bas proc math gpio uart i2c time -o examples/attiny/blink.bin
//!
//! The ATtiny85 has no UART or I2C peripheral, only the USI, so uart and i2c are the templates
//! from virtmach that set RuntimeError::UnimplementedInterruptFunc until they are implemented.
//! time waits on a free-running Timer1 clock, see clock.rs. Every HALT of the program waits until
//! TICK_MS passed since the last one, END and runtime errors stop the vm. Timer0 stays free for a
//! USI UART.

use attiny_hal::clock::{ Clock as _, MHz1 };
use panic_halt as _;
use virtmach::{ VirtMach, Program, Runtime, interrupts::{ SoftInterrupt, proc, math, uart, i2c } };
use flash::{ Flash, FlashBytes };
use clock::{ Clock as Timer1Clock, Schedule };

mod clock;
mod flash;
mod gpio;
mod time;
#[cfg(feature = "simavr")]
mod simavr;

/// The factory fuses run the internal 8 MHz oscillator divided by 8
type Clock = MHz1;

const TICK_MS: u32 = 10;

const TICK: u32 = TICK_MS * 1000 / clock::US_PER_COUNT;

#[unsafe(link_section = ".progmem.data")]
static PROGRAM: [u8; include_bytes!("../blink.bin").len()] = *include_bytes!("../blink.bin");

#[avr_device::entry]
fn main() -> ! {
    let dp = attiny_hal::Peripherals::take().unwrap();
    let clock = Timer1Clock::start(dp.TC1, &dp.CPU, Clock::FREQ);
    let mut tick = Schedule::new(&clock);

    let mut vm = VirtMach::<Flash>::with_storage();
    vm.load_program(Program { source: 0, id: "blink", data: FlashBytes::of(&PROGRAM) });

    let (mut p, mut m, mut g, mut u, mut i) = (proc::Interrupt {}, math::Interrupt {}, gpio::Gpio { port: dp.PORTB }, uart::Interrupt {}, i2c::Interrupt {});
    let mut t = time::Time { clock: &clock, schedule: Schedule::new(&clock) };
    let interrupts: &mut [&mut dyn SoftInterrupt<Flash>] = &mut [&mut p, &mut m, &mut g, &mut u, &mut i, &mut t];

    loop {
        vm.run(0, interrupts);
        match vm.state {
            Runtime::Hlt => { tick.wait(&clock, TICK); }
            _ => break
        }
    }

    loop {}
}

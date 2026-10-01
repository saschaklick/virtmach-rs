#![no_main]
#![no_std]
#![feature(abi_avr_interrupt)]
#![feature(asm_experimental_arch)]

//! Runs the virtmach program blink.bin on an ATtiny85. The program stays in flash and the vm reads
//! it with LPM, see flash.rs, so its size only counts against the flash, not the 512 bytes of RAM.
//!
//! Every interrupt is at its INDEX, see virtmach::interrupts::builtin_index, so programs are
//! compiled with the names of the interrupts they need in any order, from the repository root:
//!
//!   cargo run --manifest-path sdk/Cargo.toml --bin basic -- examples/attiny/blink.bas gpio time -o examples/attiny/blink.bin
//!
//! The features proc, gpio, uart, i2c and time select the interrupts, the ones left out and the
//! ones the firmware does not have (string, random, trig, surface) are dummy, which sets
//! RuntimeError::UnimplementedInterruptFunc when called. The ATtiny85 has no UART or I2C peripheral, only the USI, so
//! uart and i2c are the templates from virtmach that set RuntimeError::UnimplementedInterruptFunc
//! until they are implemented. time waits on a free-running Timer1 clock, see clock.rs, Timer0
//! stays free for a USI UART.
//!
//! All timing is up to the time interrupt: HALT only returns to the firmware, which continues the
//! vm right away. END and runtime errors stop it.

use panic_halt as _;
use virtmach::{ VirtMach, Program, Runtime, interrupts::{ SoftInterrupt, math } };
use virtmach::interrupts::dummy;
#[cfg(feature = "proc")]
use virtmach::interrupts::proc;
#[cfg(feature = "uart")]
use virtmach::interrupts::uart;
#[cfg(feature = "i2c")]
use virtmach::interrupts::i2c;
use flash::{ Flash, FlashBytes };
#[cfg(feature = "time")]
use attiny_hal::clock::{ Clock as _, MHz1 };
#[cfg(feature = "time")]
use clock::{ Clock as Timer1Clock, Schedule };

#[cfg(feature = "time")]
mod clock;
mod flash;
#[cfg(feature = "gpio")]
mod gpio;
#[cfg(feature = "time")]
mod time;
#[cfg(feature = "simavr")]
mod simavr;

/// The factory fuses run the internal 8 MHz oscillator divided by 8
#[cfg(feature = "time")]
type Clock = MHz1;

#[unsafe(link_section = ".progmem.data")]
static PROGRAM: [u8; include_bytes!("../blink.bin").len()] = *include_bytes!("../blink.bin");

#[avr_device::entry]
fn main() -> ! {
    #[allow(unused_variables)]
    let dp = attiny_hal::Peripherals::take().unwrap();
    #[cfg(feature = "time")]
    let clock = Timer1Clock::start(dp.TC1, &dp.CPU, Clock::FREQ);

    let mut vm = VirtMach::<Flash>::with_storage();
    vm.load_program(Program { source: 0, id: "blink", data: FlashBytes::of(&PROGRAM) });

    // at their INDEX: math, proc, string, random, time, trig, surface, gpio, uart, i2c
    let (mut math, mut proc, mut string, mut random, mut time, mut trig, mut surface, mut gpio, mut uart, mut i2c) = (
        math::Interrupt {},
        #[cfg(feature = "proc")]
        proc::Interrupt {},
        #[cfg(not(feature = "proc"))]
        dummy::Interrupt {},                
        dummy::Interrupt {},
        dummy::Interrupt {},
        #[cfg(feature = "time")]
        time::Time { clock: &clock, schedule: Schedule::new(&clock) },
        #[cfg(not(feature = "time"))]
        dummy::Interrupt {},
        dummy::Interrupt {},
        dummy::Interrupt {},
        #[cfg(feature ="gpio")]
        gpio::Gpio { port: dp.PORTB },
        #[cfg(not(feature ="gpio"))]
        dummy::Interrupt {},        
        #[cfg(feature ="uart")]
        uart::Interrupt {},
        #[cfg(not(feature ="uart"))]
        dummy::Interrupt {},        
        #[cfg(feature ="i2c")]
        i2c::Interrupt {},
        #[cfg(not(feature ="i2c"))]
        dummy::Interrupt {},
    );    
    let interrupts: &mut [&mut dyn SoftInterrupt<Flash>] = &mut [&mut math, &mut proc, &mut string, &mut random, &mut time, &mut trig, &mut surface, &mut gpio, &mut uart, &mut i2c];

    loop {
        vm.run(0, interrupts);
        match vm.state {
            // HALT only returns to the firmware, the time interrupt does the waiting
            Runtime::Hlt => {}
            _ => break
        }
    }

    loop {}
}

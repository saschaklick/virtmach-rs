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
//!   cargo run --manifest-path sdk/Cargo.toml --bin basic -- examples/attiny/blink.bas periph time -o examples/attiny/blink.bin
//!
//! The features proc and time select those interrupts, gpio, uart_tx, uart_rx, i2c and pwm the
//! peripherals in periph. The ones left out and the ones the firmware does not have (string and
//! surface) are dummy, or in periph functions that set
//! RuntimeError::UnimplementedInterruptFunc when called. The ATtiny85 has no UART or
//! I2C peripheral, only the USI, so uart is a software UART with TX on PB2 and RX on PB0, where
//! uart_tx and uart_rx compile in sending and receiving, see int_uart.rs, and i2c is a software
//! controller on PB3 and PB4, see int_i2c.rs. time waits on a free-running Timer1 clock, see
//! clock.rs, which also times the bits of the uart, pwm drives PB0 and PB1 from Timer0, see
//! int_pwm.rs, which a USI UART would need for its baud rate.
//!
//! simulate.sh runs the firmware in simavr and prints the changes of PB1. test.sh runs test programs
//! from tests/ in simavr and checks what the firmware does with the uart, i2c and math
//! interrupts, see the top of it and sim/simtest.c.
//!
//! All timing is up to the time interrupt: HALT only returns to the firmware, which continues the
//! vm right away. END and runtime errors stop it.

use panic_halt as _;
use virtmach::{ VirtMach, Program, Runtime, interrupts::{ SoftInterrupt, math } };
use virtmach::interrupts::dummy;
#[cfg(feature = "proc")]
use virtmach::interrupts::proc;
use flash::{ Flash, FlashBytes };
#[cfg(any(feature = "time", feature = "uart_tx", feature = "uart_rx"))]
use attiny_hal::clock::Clock as _;
#[cfg(any(feature = "time", feature = "pwm", feature = "i2c", feature = "uart_tx", feature = "uart_rx"))]
use attiny_hal::clock::MHz1;
#[cfg(any(feature = "time", feature = "uart_tx", feature = "uart_rx"))]
use clock::Clock as Timer1Clock;
#[cfg(feature = "time")]
use clock::Schedule;

// for time and the uart
#[cfg(any(feature = "time", feature = "uart_tx", feature = "uart_rx"))]
#[cfg_attr(not(feature = "time"), allow(dead_code))]
mod clock;
mod flash;
#[cfg(feature = "gpio")]
mod int_gpio;
#[cfg(feature = "i2c")]
mod int_i2c;
mod int_periph;
#[cfg(feature = "pwm")]
mod int_pwm;
#[cfg(feature = "time")]
mod int_time;
#[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
mod int_uart;
#[cfg(feature = "simavr")]
mod simavr;

/// The factory fuses run the internal 8 MHz oscillator divided by 8
#[cfg(any(feature = "time", feature = "pwm", feature = "i2c", feature = "uart_tx", feature = "uart_rx"))]
type Clock = MHz1;

#[unsafe(link_section = ".progmem.data")]
static PROGRAM: [u8; include_bytes!(env!("VIRTMACH_PROGRAM_PATH")).len()] = *include_bytes!(env!("VIRTMACH_PROGRAM_PATH"));

#[avr_device::entry]
fn main() -> ! {
    #[allow(unused_variables)]
    let dp = attiny_hal::Peripherals::take().unwrap();
    // the uart only reads Timer1 directly
    #[cfg(any(feature = "time", feature = "uart_tx", feature = "uart_rx"))]
    #[allow(unused_variables)]
    let clock = Timer1Clock::start(dp.TC1, &dp.CPU, Clock::FREQ);
    // shared by gpio, uart, i2c and pwm
    #[allow(unused_variables)]
    let portb = dp.PORTB;

    // set by the time interrupt, waited after every HLT
    #[cfg(feature = "time")]
    let tick = core::cell::Cell::new(0u32);

    let mut vm = VirtMach::<Flash>::with_storage();
    vm.load_program(Program { source: 0, id: "blink", data: FlashBytes::of(&PROGRAM) });

    // at their INDEX: math, proc, string, time, four unused slots, periph, surface
    let (mut math, mut proc, mut string, mut time, mut unused4, mut unused5, mut unused6, mut unused7, mut periph, mut surface) = (
        math::Interrupt {},
        #[cfg(feature = "proc")]
        proc::Interrupt {},
        #[cfg(not(feature = "proc"))]
        dummy::Interrupt {},
        dummy::Interrupt {},
        #[cfg(feature = "time")]
        int_time::IntTime { clock: &clock, schedule: Schedule::new(&clock), tick: &tick },
        #[cfg(not(feature = "time"))]
        dummy::Interrupt {},
        dummy::Interrupt {},
        dummy::Interrupt {},
        dummy::Interrupt {},
        dummy::Interrupt {},
        int_periph::IntPeriph {
            #[cfg(feature = "gpio")]
            gpio: int_gpio::IntGpio { port: &portb },
            #[cfg(feature = "pwm")]
            pwm: int_pwm::IntPwm::new(dp.TC0, &portb),
            #[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
            uart: int_uart::IntUart { port: &portb },
            #[cfg(feature = "i2c")]
            i2c: int_i2c::IntI2c::new(&portb),
            _lifetime: core::marker::PhantomData
        },
        dummy::Interrupt {},
    );
    let interrupts: &mut [&mut dyn SoftInterrupt<Flash>] = &mut [&mut math, &mut proc, &mut string, &mut time, &mut unused4, &mut unused5, &mut unused6, &mut unused7, &mut periph, &mut surface];

    loop {
        vm.run(0, interrupts);
        match vm.state {
            // HALT only returns to the firmware, the time interrupt does the waiting, except for the
            // tick the program set
            Runtime::Hlt => {
                #[cfg(feature = "time")]
                int_time::wait_tick(&clock, tick.get());
            }
            _ => break
        }
    }

    loop {}
}

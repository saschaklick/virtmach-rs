//! PWM template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the timers or the embedded framework (HAL) of the target.
//!
//! Outputs are addressed by pin number like in gpio. Only some pins of a target can output PWM,
//! the others are an InterruptError. Pins often share a timer, then setting the frequency of one
//! changes it for the others too.
//!
//! setup makes the pin a PWM output with the frequency in Hz and returns the frequency the timer
//! actually runs at, or 0 if it cannot be reached, then the pin stays as it was. The duty cycle
//! starts at 0 until duty or pulse sets it. Frequencies only go up to VMAtom::MAX, 32767 Hz with
//! 16 bit atoms.
//!
//! duty sets the duty cycle to value / max, so it does not depend on the resolution of the timer
//! or the atom size: duty(pin, 1, 2) is 50%, duty(pin, 191, 255) is 75%. value is clamped to max,
//! max must be above 0. pulse sets the high time in microseconds instead, for servos that expect
//! 1000 to 2000 microseconds at 50 Hz, and is clamped to the period. Negative values, a max of 0
//! or a pin that was not set up are an InterruptError.
//!
//! stop drives the pin low and stops its output, setup starts it again.

use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "pwm";

pub const INDEX: u8 = 10;

pub const FUNCTIONS: [SoftInterruptFunction;4] = [
    SoftInterruptFunction { no:  0, name: "setup", arguments: 2, returns: 1, help: "Start PWM on a pin at a frequency in Hz with duty 0, returns the frequency reached or 0 (pin,frequency)->(actual)" },
    SoftInterruptFunction { no:  1, name: "duty",  arguments: 3, returns: 0, help: "Set the duty cycle to value/max (pin,value,max)->()" },
    SoftInterruptFunction { no:  2, name: "pulse", arguments: 2, returns: 0, help: "Set the high time in microseconds (pin,microseconds)->()" },
    SoftInterruptFunction { no:  3, name: "stop",  arguments: 1, returns: 0, help: "Stop PWM on a pin and drive it low (pin)->()" }
];

pub struct Interrupt {}

impl <S: Storage> SoftInterrupt<S> for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        match op {
            0 => { let (_pin, _frequency) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_pin, _value, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            2 => { let (_pin, _microseconds) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            3 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

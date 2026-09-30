//! GPIO template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the embedded framework (HAL) of the target.
//!
//! Pins are addressed by number. For the masked functions the pins are grouped into ports of
//! VMAtom::BITS pins, bit n of a mask on port p is pin p * VMAtom::BITS + n.
//!
//! mode: 0 input, 1 output push-pull, 2 output open-drain
//! pull: 0 none, 1 pull-up, 2 pull-down
//! level: 0 low, anything else high

use crate::{RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "gpio";

pub const FUNCTIONS: [SoftInterruptFunction;9] = [
    SoftInterruptFunction { no:  0, name: "setup",      arguments: 2, returns: 0, help: "Set the pin mode, 0 input, 1 output, 2 open-drain (pin,mode)->()" },
    SoftInterruptFunction { no:  1, name: "set_pull",   arguments: 2, returns: 0, help: "Set the pull resistor, 0 none, 1 up, 2 down (pin,pull)->()" },
    SoftInterruptFunction { no:  2, name: "write",      arguments: 2, returns: 0, help: "Set an output low or high (pin,level)->()" },
    SoftInterruptFunction { no:  3, name: "high",       arguments: 1, returns: 0, help: "Set an output high (pin)->()" },
    SoftInterruptFunction { no:  4, name: "low",        arguments: 1, returns: 0, help: "Set an output low (pin)->()" },
    SoftInterruptFunction { no:  5, name: "toggle",     arguments: 1, returns: 0, help: "Toggle an output (pin)->()" },
    SoftInterruptFunction { no:  6, name: "read",       arguments: 1, returns: 1, help: "Read an input, 0 low, 1 high (pin)->(level)" },
    SoftInterruptFunction { no: 10, name: "write_mask", arguments: 3, returns: 0, help: "Set the outputs of a port whose mask bits are set to the bits of values (port,mask,values)->()" },
    SoftInterruptFunction { no: 11, name: "read_mask",  arguments: 2, returns: 1, help: "Read the inputs of a port whose mask bits are set, the other bits are 0 (port,mask)->(values)" }
];

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();
        match op {
            0 => { let (_pin, _mode) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_pin, _pull) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            2 => { let (_pin, _level) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            3 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            4 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            5 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            6 => { let _pin = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            10 => { let (_port, _mask, _values) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            11 => { let (_port, _mask) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

//! Time template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the timer and clock of the target.
//!
//! An interval is seconds * 1000000 + microseconds, microseconds is not limited to 999999 but
//! only goes up to VMAtom::MAX, 32767 with 16 bit atoms. Negative values are an InterruptError.
//!
//! wait_for waits the interval from the moment it is called, so the time the program spent before
//! adds to the period of a loop. wait_until waits until the interval has passed since the time the
//! last wait_until returned at, the first one counts from the start of the program, so a loop
//! with it keeps its period no matter what the program does in between. It returns 1 if it
//! waited and 0 if the interval had already passed, then it returns at once and the next interval
//! counts from now, instead of returning at once until it caught up.
//!
//! Waiting does not have to block inside call: the implementation can store the deadline, pause
//! the vm with vm.pause() and let the firmware continue with vm.run once the deadline is reached,
//! sleeping in between. The program continues after the interrupt call either way.
//!
//! get_time and get_date read the clock of the target, usually an RTC. Without a clock that was
//! set, get_time can count from the start and get_date return 0, 0, 0. The values are pushed in
//! the order they are listed, so H, M, S, MS = time.get_time() works in BASIC. year needs at
//! least 16 bit atoms.

use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "time";

pub const FUNCTIONS: [SoftInterruptFunction;4] = [
    SoftInterruptFunction { no:  0, name: "wait_for",   arguments: 2, returns: 0, help: "Wait the interval from now (seconds,microseconds)->()" },
    SoftInterruptFunction { no:  1, name: "wait_until", arguments: 2, returns: 1, help: "Wait until the interval passed since the last wait_until, 1 if it waited, 0 if it was too short (seconds,microseconds)->(waited)" },
    SoftInterruptFunction { no:  2, name: "get_time",   arguments: 0, returns: 4, help: "Time of day, hours 0-23 ()->(hours,minutes,seconds,ms)" },
    SoftInterruptFunction { no:  3, name: "get_date",   arguments: 0, returns: 3, help: "Date, month 1-12 and day 1-31 ()->(year,month,day)" }
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
            0 => { let (_seconds, _microseconds) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_seconds, _microseconds) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            // pushed as hours, minutes, seconds, ms
            2 => { for _ in 0..4 { vm.stack_push(0); } vm.error = RuntimeError::UnimplementedInterruptFunc; }
            // pushed as year, month, day
            3 => { for _ in 0..3 { vm.stack_push(0); } vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

//! Time template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the timer and clock of the target.
//!
//! An interval is seconds * 1000 + milliseconds, milliseconds is not limited to 999 but goes up
//! to VMAtom::MAX, 32767 with 16 bit atoms and 127 with 8 bit atoms. Negative values are an
//! InterruptError.
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
//! set_tick sets the tick, the interval the host waits after the program stopped with a HLT
//! instruction before it runs the vm again, so the program can sleep between its rounds. The
//! tick is 0 at first, then the host does not wait. Like the other intervals it is seconds * 1000
//! + milliseconds. The implementation keeps it, for example in a field, and the firmware or host
//! that runs the vm has to honor it: once vm.run returns with the state Hlt because of HLT, wait
//! the tick before the next vm.run. Waits of wait_for and wait_until do not count, they have
//! their own interval.
//!
//! get_time and get_date read the clock of the target, usually an RTC. Without a clock that was
//! set, get_time can count from the start and get_date return 0, 0, 0. The values are pushed in
//! the order they are listed, so H, M, S, MS = time.get_time() works in BASIC. year needs at
//! least 16 bit atoms.

use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "time";

pub const INDEX: u8 = 3;

pub const FUNCTIONS: [SoftInterruptFunction;5] = [
    SoftInterruptFunction { no:  0, name: "wait_for",   arguments: 2, returns: 0, help: "Wait the interval from now (seconds,milliseconds)->()" },
    SoftInterruptFunction { no:  1, name: "wait_until", arguments: 2, returns: 1, help: "Wait until the interval passed since the last wait_until, 1 if it waited, 0 if it was too short (seconds,milliseconds)->(waited)" },
    SoftInterruptFunction { no:  3, name: "set_tick",   arguments: 2, returns: 0, help: "Set the tick interval (seconds,milliseconds)->()" },
    SoftInterruptFunction { no: 10, name: "get_time",   arguments: 0, returns: 4, help: "Time of day, hours 0-23 ()->(hours,minutes,seconds,ms)" },
    SoftInterruptFunction { no: 11, name: "get_date",   arguments: 0, returns: 3, help: "Date, month 1-12 and day 1-31 ()->(year,month,day)" }
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
            0 => { let (_seconds, _milliseconds) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_seconds, _milliseconds) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            3 => { let (_seconds, _milliseconds) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            // pushed as hours, minutes, seconds, ms
            10 => { for _ in 0..4 { vm.stack_push(0); } vm.error = RuntimeError::UnimplementedInterruptFunc; }
            // pushed as year, month, day
            11 => { for _ in 0..3 { vm.stack_push(0); } vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

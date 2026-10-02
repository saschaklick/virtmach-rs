//! The time interrupt on the ATtiny85, waits on the Timer1 clock and blocks inside call.
//!
//! Intervals have a resolution of clock::US_PER_COUNT microseconds and must be shorter than
//! 2^31 counts, about 4.7 hours. Longer or negative ones set RuntimeError::InterruptError.
//! There is no RTC, so get_time and get_date are not implemented.

use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, time } };
use crate::clock::{ Clock, Schedule, US_PER_COUNT };

pub struct IntTime<'a> {
    pub clock: &'a Clock,
    /// for wait_until, the first interval counts from the start of the program
    pub schedule: Schedule
}

/// seconds * 1000 + milliseconds in clock counts, rounded, None if negative or too long
fn interval(seconds: VMAtom, milliseconds: VMAtom) -> Option<u32> {
    if seconds < 0 || milliseconds < 0 {
        return None;
    }
    let (seconds, milliseconds) = (seconds as u32, milliseconds as u32);
    // up to 16 bit atoms the counts fit into u32 without checks, only i32 atoms can overflow it
    let counts = if VMAtom::BITS <= 16 {
        seconds * (1_000_000 / US_PER_COUNT) + (milliseconds * 1000 + US_PER_COUNT / 2) / US_PER_COUNT
    } else {
        let milliseconds = milliseconds.checked_mul(1000)?.checked_add(US_PER_COUNT / 2)? / US_PER_COUNT;
        seconds.checked_mul(1_000_000 / US_PER_COUNT)?.checked_add(milliseconds)?
    };
    if counts > i32::MAX as u32 { None } else { Some(counts) }
}

impl <S: Storage> SoftInterrupt<S> for IntTime<'_> {
    fn name(&self) -> &str {
        return time::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        match op {
            // wait_for and wait_until share the arguments and their check
            0 | 1 => {
                let (seconds, milliseconds) = (vm.stack_pop(), vm.stack_pop());
                let waited = match interval(seconds, milliseconds) {
                    Some(counts) if op == 0 => { self.clock.wait_until(self.clock.now().wrapping_add(counts)); return; }
                    Some(counts) => self.schedule.wait(self.clock, counts),
                    None => { vm.error = RuntimeError::InterruptError; false }
                };
                if op == 1 { vm.stack_push(waited as VMAtom); }
            }
            // no RTC: get_time pushes hours, minutes, seconds, ms and get_date year, month, day
            2 | 3 => {
                for _ in 0..(if op == 2 { 4 } else { 3 }) { vm.stack_push(0); }
                vm.error = RuntimeError::UnimplementedInterruptFunc;
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

//! The time interrupt on the ATtiny85, waits on the Timer1 clock and blocks inside call.
//!
//! Intervals have a resolution of clock::US_PER_COUNT microseconds and must be shorter than
//! 2^31 counts, about 4.7 hours. Longer or negative ones set RuntimeError::InterruptError.
//! There is no RTC, so get_time and get_date are not implemented. The tick of set_tick is waited
//! by the main loop after every HLT, see wait_tick.

use core::cell::Cell;
use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, time } };
use crate::clock::{ Clock, Schedule, US_PER_COUNT };

pub struct IntTime<'a> {
    /// the tick of set_tick in milliseconds, 0 for none, the main loop waits it after every HLT
    pub tick: &'a Cell<u32>,
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

/// Waits the tick the program set with set_tick, in milliseconds
pub fn wait_tick(clock: &Clock, milliseconds: u32) {
    if milliseconds > 0 {
        let counts = milliseconds.saturating_mul(1000 / US_PER_COUNT).min(i32::MAX as u32);
        clock.wait_until(clock.now().wrapping_add(counts));
    }
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
            3 => {
                let (seconds, milliseconds) = (vm.stack_pop(), vm.stack_pop());
                if seconds < 0 || milliseconds < 0 {
                    vm.error = RuntimeError::InterruptError;
                } else {
                    self.tick.set((seconds as u32).saturating_mul(1000).saturating_add(milliseconds as u32));
                }
            }
            // no RTC: get_time pushes hours, minutes, seconds, ms and get_date year, month, day
            10 | 11 => {
                for _ in 0..(if op == 10 { 4 } else { 3 }) { vm.stack_push(0); }
                vm.error = RuntimeError::UnimplementedInterruptFunc;
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

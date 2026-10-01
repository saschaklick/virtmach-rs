//! A free-running clock from Timer1, in counts of US_PER_COUNT microseconds.
//!
//! Timer1 counts from 0 to 255 and its overflow interrupt counts the overflows, together they
//! make a 32 bit count that wraps after about 9.5 hours. Deadlines are compared with wrapping
//! arithmetic, so intervals up to half of that work. Timer0 is left free, it is the only clock
//! the USI can take besides software, so a USI UART needs it for the baud rate.

use core::{ arch::asm, cell::Cell };
use attiny_hal::pac::{ CPU, TC1 };
use avr_device::interrupt::{ self, Mutex };

pub const US_PER_COUNT: u32 = 8;

/// Overflows of Timer1, the upper 24 bits of the count
static OVERFLOWS: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

/// Prescaler select of Timer1 for counts of US_PER_COUNT at a clock of hz, CS1 = n divides the
/// clock by 2^(n-1)
const fn prescaler(hz: u32) -> u8 {
    let prescale = hz / 1_000_000 * US_PER_COUNT;
    assert!(prescale.is_power_of_two() && prescale <= 16384, "no Timer1 prescaler for this clock");
    prescale.trailing_zeros() as u8 + 1
}

pub struct Clock {
    tc1: TC1
}

impl Clock {
    /// Starts Timer1 and enables interrupts
    pub fn start(tc1: TC1, cpu: &CPU, hz: u32) -> Self {
        tc1.tcnt1().write(|w| unsafe { w.bits(0) });
        tc1.tccr1().write(|w| unsafe { w.bits(prescaler(hz)) });
        tc1.timsk().modify(|_, w| w.toie1().set_bit());
        // sleep enable, idle mode keeps the timers running
        cpu.mcucr().modify(|_, w| w.se().set_bit().sm().idle());
        unsafe { interrupt::enable() };
        Clock { tc1 }
    }

    /// The current count
    pub fn now(&self) -> u32 {
        interrupt::free(|cs| {
            let mut overflows = OVERFLOWS.borrow(cs).get();
            let count = self.tc1.tcnt1().read().bits();
            // an overflow that happened since interrupts were disabled is not counted yet
            if self.tc1.tifr().read().tov1().bit_is_set() && count < 128 {
                overflows = overflows.wrapping_add(1);
            }
            overflows << 8 | count as u32
        })
    }

    /// Waits in idle sleep until the count reaches deadline. The overflow interrupt wakes it until
    /// the deadline is in the current period of 256 counts, then the compare A interrupt wakes it
    /// at the count of the deadline, so the program continues the same few cycles after it.
    pub fn wait_until(&self, deadline: u32) {
        loop {
            interrupt::disable();
            let now = self.now();
            if deadline.wrapping_sub(now) as i32 <= 0 {
                break;
            }
            if deadline >> 8 == now >> 8 {
                // compare A matches when TCNT1 counts up to the low byte of the deadline, an old
                // match is cleared by writing its flag
                self.tc1.ocr1a().write(|w| unsafe { w.bits(deadline as u8) });
                self.tc1.tifr().write(|w| w.ocf1a().set_bit());
                self.tc1.timsk().modify(|_, w| w.ocie1a().set_bit());
                // the count may have passed the deadline before the compare was set
                if deadline.wrapping_sub(self.now()) as i32 <= 0 {
                    break;
                }
            }
            // sei only takes effect after the next instruction, so the interrupt that wakes the
            // sleep cannot happen between the check and the sleep
            unsafe { asm!("sei", "sleep") };
        }
        self.tc1.timsk().modify(|_, w| w.ocie1a().clear_bit());
        unsafe { interrupt::enable() };
    }
}

/// Waits for regular intervals: each one counts from the end of the last, not from the call
pub struct Schedule {
    last: u32
}

impl Schedule {
    /// Starts counting the first interval from now
    pub fn new(clock: &Clock) -> Self {
        Schedule { last: clock.now() }
    }

    /// Waits until interval counts passed since the last wait and returns true. If they already
    /// passed it returns false at once and the next interval counts from now.
    pub fn wait(&mut self, clock: &Clock, interval: u32) -> bool {
        let deadline = self.last.wrapping_add(interval);
        let now = clock.now();
        if deadline.wrapping_sub(now) as i32 <= 0 {
            self.last = now;
            return false;
        }
        clock.wait_until(deadline);
        self.last = deadline;
        true
    }
}

// only wakes the sleep in Clock::wait_until
#[avr_device::interrupt(attiny85)]
fn TIMER1_COMPA() {}

#[avr_device::interrupt(attiny85)]
fn TIMER1_OVF() {
    interrupt::free(|cs| {
        let overflows = OVERFLOWS.borrow(cs);
        overflows.set(overflows.get().wrapping_add(1));
    });
}

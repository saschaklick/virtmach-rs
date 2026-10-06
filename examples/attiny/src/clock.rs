//! A free-running clock from Timer1, in counts of US_PER_COUNT microseconds.
//!
//! Timer1 counts from 0 to 255 and its overflow interrupt counts the overflows, together they
//! make a 32 bit count that wraps after about 9.5 hours. Deadlines are compared with wrapping
//! arithmetic, so intervals up to half of that work. Timer0 is left free, it is the only clock
//! the USI can take besides software, so a USI UART needs it for the baud rate.
//!
//! The software uart in int_uart.rs times its bits with tcnt1 and runs with interrupts disabled
//! for a whole byte, longer than an overflow period at low baud rates, so it calls count_overflow.

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

/// The low byte of the count, for timing with interrupts disabled. Reads Timer1 directly, which
/// Clock::start must have started.
#[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
#[inline(always)]
pub fn tcnt1() -> u8 {
    unsafe { TC1::steal() }.tcnt1().read().bits()
}

/// Counts an overflow that is pending because interrupts are disabled, so the count does not lose
/// it if they stay disabled for more than one overflow period of 256 counts
#[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
#[inline(always)]
pub fn count_overflow(cs: interrupt::CriticalSection) {
    let tc1 = unsafe { TC1::steal() };
    if tc1.tifr().read().tov1().bit_is_set() {
        // writing the flag clears it
        tc1.tifr().write(|w| w.tov1().set_bit());
        let overflows = OVERFLOWS.borrow(cs);
        overflows.set(overflows.get().wrapping_add(1));
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

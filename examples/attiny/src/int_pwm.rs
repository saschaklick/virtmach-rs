//! The pwm interrupt on the ATtiny85, pins 0 and 1 are PB0 and PB1 driven by Timer0.
//!
//! Timer1 runs the clock of the time interrupt, its PWM mode would move the overflow flag the
//! clock counts and delay its compare register writes, so only the Timer0 outputs OC0A on PB0 and
//! OC0B on PB1 are used. Other pins set RuntimeError::InterruptError.
//!
//! Both pins share the frequency. It comes from the five prescalers with the counter running
//! 0-255 in fast PWM or 0-255-0 in phase correct PWM at half the frequency. setup takes the one
//! nearest to the requested frequency, at 1 MHz 3906, 1961, 488, 245, 61, 31, 15, 8, 4 or 2 Hz, and
//! returns 0 if that is off by more than a factor of 2. A new frequency keeps the duty cycle of
//! the other pin, pulse widths are converted once and change with it.
//!
//! The duty cycle has steps of 1/256. At 0 and 100% the timer is disconnected from the pin, which
//! is then driven low or high by PORTB, so there are no spikes of one timer step at the ends.
//!
//! Timer0 is also the clock the USI can use for the baud rate of a UART, so pwm and a USI UART
//! that needs it cannot run together.
//!
//! The ATtiny85 is an 8 bit cpu without division, so everything at runtime is u16: the frequencies
//! and the points between them are tables computed at compile time and kept in flash, so they do
//! not take any RAM, see flash.rs, pulse widths become timer
//! steps by shifting and the duty cycles are divided by shifting and subtracting, see ratio. So
//! with atoms above 16 bit pulse only takes up to 65535 microseconds.

use attiny_hal::{ clock::Clock as _, pac::{ PORTB, TC0 } };
use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, pwm } };
use crate::flash::{ read_u8, read_u16 };

const HZ: u32 = crate::Clock::FREQ;

/// cpu cycles per microsecond as a shift, clock.rs needs a power of two as well
const MHZ_SHIFT: u8 = {
    let mhz = HZ / 1_000_000;
    assert!(mhz.is_power_of_two(), "cpu clock is not a power of two MHz");
    mhz.trailing_zeros() as u8
};

/// Duty cycle of 100%
const FULL: u16 = 256;

/// The modes of Timer0 by ascending period: CS0 = n / 2 + 1 divides the clock by
/// 2^PRESCALER_SHIFTS[n / 2], odd n are phase correct with a period of 510 timer steps, even n
/// fast PWM with 256
const PRESCALER_SHIFTS: [u8; 5] = [0, 3, 6, 8, 10];

const fn steps(n: usize) -> u16 {
    if n % 2 == 1 { 510 } else { 256 }
}

/// The frequencies of the modes in Hz, rounded, descending
const fn frequencies() -> [u16; 10] {
    assert!(HZ / 256 <= u16::MAX as u32, "no u16 frequency table for this clock");
    let mut res = [0; 10];
    let mut n = 0;
    while n < 10 {
        let period = (steps(n) as u32) << PRESCALER_SHIFTS[n / 2];
        res[n] = ((HZ + period / 2) / period) as u16;
        n += 1;
    }
    res
}

// the tables below are in flash and only read with read_u8 and read_u16

#[unsafe(link_section = ".progmem.data")]
static SHIFTS: [u8; 5] = PRESCALER_SHIFTS;

#[unsafe(link_section = ".progmem.data")]
static FREQUENCIES: [u16; 10] = frequencies();

/// Geometric means of neighbouring FREQUENCIES, a frequency at or above THRESHOLDS[n] is nearer
/// to FREQUENCIES[n] than to FREQUENCIES[n + 1]
#[unsafe(link_section = ".progmem.data")]
static THRESHOLDS: [u16; 9] = {
    let frequencies = frequencies();
    let mut res = [0; 9];
    let mut n = 0;
    while n < 9 {
        res[n] = (frequencies[n] as u32 * frequencies[n + 1] as u32).isqrt() as u16;
        n += 1;
    }
    res
};

/// An atom checked to be positive as u16, larger ones are clamped
fn to_u16(atom: VMAtom) -> u16 {
    if VMAtom::BITS > 16 && atom as i32 > 0xffff { 0xffff } else { atom as u16 }
}

/// value / max in 1/FULL, rounded, value is clamped to max
fn ratio(value: u16, max: u16) -> u16 {
    if value >= max {
        return FULL;
    }
    // long division by max of value shifted left by the 8 bits of FULL, rem stays below max, so
    // max up to 0x8000 cannot overflow it
    let (mut rem, mut quotient) = (value, 0);
    for _ in 0..8 {
        rem <<= 1;
        quotient <<= 1;
        if rem >= max {
            rem -= max;
            quotient |= 1;
        }
    }
    if rem >= max - rem { quotient + 1 } else { quotient }
}

pub struct IntPwm<'a> {
    tc0: TC0,
    port: &'a PORTB,
    /// index of the frequency in FREQUENCIES, set by setup
    mode: usize,
    /// set up pins, bit n for PBn
    active: u8,
    /// duty cycles of PB0 and PB1 in 1/FULL
    levels: [u16; 2]
}

impl<'a> IntPwm<'a> {
    pub fn new(tc0: TC0, port: &'a PORTB) -> Self {
        IntPwm { tc0, port, mode: 0, active: 0, levels: [0; 2] }
    }

    /// Runs Timer0 at the frequency nearest to the given one and returns the frequency reached,
    /// or 0 and leaves the timer as it is if that is more than a factor of 2 away
    fn setup(&mut self, pin: usize, frequency: u16) -> u16 {
        let n = (0..9).filter(|&n| read_u16(&THRESHOLDS, n) > frequency).count();
        let reached = read_u16(&FREQUENCIES, n);
        if reached > frequency.saturating_mul(2) || reached.saturating_mul(2) < frequency {
            return 0;
        }
        self.mode = n;
        // WGM0 = 3 is fast PWM, 1 phase correct, both counting to 255
        self.tc0.tccr0a().modify(|r, w| unsafe { w.bits(r.bits() & !0b11 | if n % 2 == 1 { 0b01 } else { 0b11 }) });
        self.tc0.tccr0b().write(|w| unsafe { w.bits(n as u8 / 2 + 1) });
        self.port.ddrb().modify(|r, w| unsafe { w.bits(r.bits() | 1 << pin) });
        self.active |= 1 << pin;
        self.levels[pin] = 0;
        for other in 0..2 {
            if self.active & 1 << other != 0 {
                self.apply(other);
            }
        }
        reached
    }

    /// Outputs the duty cycle of the pin, connecting OC0A or OC0B or driving the pin with PORTB
    fn apply(&self, pin: usize) {
        let level = self.levels[pin];
        let phase = self.mode % 2 == 1;
        // COM0A is bits 7:6 for PB0 and COM0B bits 5:4 for PB1, 0b10 clears on compare match
        let com = 0b1100_0000u8 >> (2 * pin);
        if level == 0 || level >= if phase { FULL - 1 } else { FULL } {
            self.tc0.tccr0a().modify(|r, w| unsafe { w.bits(r.bits() & !com) });
            self.port.portb().modify(|r, w| unsafe { w.bits(if level == 0 { r.bits() & !(1 << pin) } else { r.bits() | 1 << pin }) });
        } else {
            // fast PWM is high for OCR0x + 1 of 256 steps, phase correct for OCR0x of 255
            let ocr = (if phase { level } else { level - 1 }) as u8;
            if pin == 0 {
                self.tc0.ocr0a().write(|w| unsafe { w.bits(ocr) });
            } else {
                self.tc0.ocr0b().write(|w| unsafe { w.bits(ocr) });
            }
            self.tc0.tccr0a().modify(|r, w| unsafe { w.bits(r.bits() & !com | com & 0b1010_1010) });
        }
    }
}

impl <S: Storage> SoftInterrupt<S> for IntPwm<'_> {
    fn name(&self) -> &str {
        return pwm::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        if !(0..=3).contains(&op) {
            vm.error = RuntimeError::UnimplementedInterruptFunc;
            return;
        }
        // pin, then frequency, value or microseconds, then max
        let pin = vm.stack_pop();
        let first = if op <= 2 { vm.stack_pop() } else { 0 };
        let second = if op == 1 { vm.stack_pop() } else { 0 };

        let bit = if (0..2).contains(&pin) { 1u8 << pin } else { 0 };
        let valid = bit != 0 && first >= 0 && second >= 0 && match op {
            0 => first > 0,
            1 => second > 0 && self.active & bit != 0,
            2 => self.active & bit != 0,
            _ => true
        };
        if !valid {
            vm.error = RuntimeError::InterruptError;
            if op == 0 { vm.stack_push(0); }
            return;
        }

        let pin = pin as usize;
        match op {
            0 => {
                let reached = self.setup(pin, to_u16(first));
                vm.stack_push(reached.min(VMAtom::MAX as u16) as VMAtom);
            }
            1 | 2 => {
                self.levels[pin] = if op == 1 {
                    let (mut value, mut max) = (first.min(second), second);
                    // only atoms above 16 bit need this to keep ratio in u16
                    while VMAtom::BITS > 16 && max as i32 > 0x8000 { (value, max) = (value >> 1, max >> 1); }
                    ratio(value as u16, max as u16)
                } else {
                    // microseconds in timer steps: times the cycles per microsecond, divided by the
                    // prescaler, ratio clamps to the period
                    let (us, steps, shift) = (to_u16(first), steps(self.mode), read_u8(&SHIFTS, self.mode / 2));
                    let ticks = if shift >= MHZ_SHIFT {
                        us >> (shift - MHZ_SHIFT)
                    } else if us >= steps >> (MHZ_SHIFT - shift) {
                        steps
                    } else {
                        us << (MHZ_SHIFT - shift)
                    };
                    ratio(ticks, steps)
                };
                self.apply(pin);
            }
            _ if self.active & bit != 0 => {
                self.levels[pin] = 0;
                self.apply(pin);
                self.active &= !bit;
                // the timer only runs while a pin uses it
                if self.active == 0 {
                    self.tc0.tccr0b().write(|w| unsafe { w.bits(0) });
                }
            }
            _ => {}
        }
    }
}

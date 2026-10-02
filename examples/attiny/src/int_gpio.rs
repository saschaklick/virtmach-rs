//! The gpio interrupt on the ATtiny85, pins 0-5 are PB0-PB5 and port 0 is PORTB.
//!
//! The ATtiny85 has pull-ups only and no open-drain outputs, pull-downs and mode 2 set
//! RuntimeError::InterruptError like invalid pins and ports do.
//!
//! All functions work on a set of bits of PORTB: one bit for the pin functions, the mask for the
//! masked ones. setup sets them in DDRB, set_pull, write, high, low and write_mask in PORTB, with
//! the pin as input PORTB switches the pull-up. So the arguments are popped and checked once and
//! only the register access differs.

use attiny_hal::pac::PORTB;
use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, gpio } };

const PINS: VMAtom = 6;
const MASK: u8 = (1 << PINS) - 1;

pub struct IntGpio {
    pub port: PORTB
}

impl <S: Storage> SoftInterrupt<S> for IntGpio {
    fn name(&self) -> &str {
        return gpio::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        if !matches!(op, 0..=6 | 10 | 11) {
            vm.error = RuntimeError::UnimplementedInterruptFunc;
            return;
        }
        let masked = op >= 10;
        // pin or port, then mode, pull, level or mask, then values
        let first = vm.stack_pop();
        let second = if masked || op <= 2 { vm.stack_pop() } else { 0 };
        let third = if op == 10 { vm.stack_pop() } else { 0 };

        let (mut valid, bits) = if masked {
            (first == 0, second as u8 & MASK)
        } else {
            let valid = (0..PINS).contains(&first);
            (valid, if valid { 1 << first } else { 0 })
        };
        // setup and set_pull only take 0 and 1
        if op <= 1 && !(0..=1).contains(&second) {
            valid = false;
        }
        let level: u8 = match op {
            0..=2 => if second != 0 { 0xff } else { 0 },
            3 => 0xff,
            10 => third as u8,
            _ => 0
        };

        if valid {
            match op {
                0 => { self.port.ddrb().modify(|r, w| unsafe { w.bits(r.bits() & !bits | level & bits) }); }
                1..=4 | 10 => { self.port.portb().modify(|r, w| unsafe { w.bits(r.bits() & !bits | level & bits) }); }
                // writing a 1 to PINB toggles the output
                5 => { self.port.pinb().write(|w| unsafe { w.bits(bits) }); }
                _ => {}
            }
        } else {
            vm.error = RuntimeError::InterruptError;
        }
        if op == 6 || op == 11 {
            let values = if valid { self.port.pinb().read().bits() & bits } else { 0 };
            vm.stack_push(if op == 6 { (values != 0) as VMAtom } else { values as VMAtom });
        }
    }
}

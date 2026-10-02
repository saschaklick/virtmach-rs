//! The i2c interrupt on the ATtiny85, a software controller with SDA on PB3 and SCL on PB4.
//!
//! The pins are driven like open-drain outputs: their PORTB bits stay 0 and DDRB pulls a line low
//! or releases it, so the bus needs external pull-ups, usually 4.7k to VCC. gpio must leave PB3
//! and PB4 alone. Only port 0 exists, other ports and devices above 127 set
//! RuntimeError::InterruptError, as do transfers before setup.
//!
//! Every transfer blocks until it is done. speed is an upper bound: setup adds a delay to every
//! half clock period, but the bit-banging itself is slow, at 1 MHz the bus runs at about 4.5 kHz
//! without one. I2C devices accept any slower clock, SMBus devices may need 10 kHz. Devices may stretch the clock by holding SCL low,
//! after about 50 ms the transfer ends with the timeout status -4. A device that still holds SDA
//! low before a start gets 9 clocks to let go, otherwise the transfer ends with the bus error -3.
//!
//! Transfers go through a buffer of 32 bytes, so lengths above 32 set RuntimeError::InterruptError
//! and write_str only sends from the first 32 bytes of a string. Memory ranges outside the memory
//! set RuntimeError::MemoryOutOfBounds. write_reg returns 1 for the data byte, the register is not
//! counted. Without the alloc feature strings cannot be set, so read_str reads and then sets
//! RuntimeError::DictionaryOutOfBound.

use core::arch::asm;
use attiny_hal::{ clock::Clock as _, pac::PORTB };
use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, i2c } };

const SDA: u8 = 1 << 3;
const SCL: u8 = 1 << 4;

/// Delay loops for a half clock period at 1 kHz, a loop takes about 4 cycles
const LOOPS_AT_1KHZ: u16 = (crate::Clock::FREQ / 8000) as u16;

/// Polls of SCL while a device stretches the clock, about 50 ms at 1 MHz
const STRETCH_POLLS: u16 = 5000;

const BUF: usize = 32;

const NACK_ADDRESS: i8 = -1;
const NACK_DATA: i8 = -2;
const BUS_ERROR: i8 = -3;
const TIMEOUT: i8 = -4;

pub struct IntI2c<'a> {
    port: &'a PORTB,
    /// delay loops per half clock period, None until setup
    delay: Option<u16>
}

impl<'a> IntI2c<'a> {
    pub fn new(port: &'a PORTB) -> Self {
        IntI2c { port, delay: None }
    }

    /// Pulls a line low or releases it, then waits the delay
    #[inline(never)]
    fn line(&self, line: u8, high: bool) {
        self.port.ddrb().modify(|r, w| unsafe { w.bits(if high { r.bits() & !line } else { r.bits() | line }) });
        for _ in 0..self.delay.unwrap_or(0) {
            unsafe { asm!("nop") };
        }
    }

    fn sda(&self) -> bool {
        self.port.pinb().read().bits() & SDA != 0
    }

    /// Releases SCL and waits while a device stretches the clock
    fn scl_high(&self) -> Result<(), i8> {
        self.line(SCL, true);
        let mut polls = STRETCH_POLLS;
        while self.port.pinb().read().bits() & SCL == 0 {
            polls -= 1;
            if polls == 0 { return Err(TIMEOUT); }
        }
        Ok(())
    }

    /// A start, or a repeated start after a byte, which leaves SCL low. A device that holds SDA
    /// low, after a transfer was cut off, gets up to 9 clocks to finish its bit and let go.
    fn start(&self) -> Result<(), i8> {
        self.line(SDA, true);
        self.scl_high()?;
        for _ in 0..9 {
            if self.sda() { break; }
            self.line(SCL, false);
            self.scl_high()?;
        }
        if !self.sda() { return Err(BUS_ERROR); }
        self.line(SDA, false);
        self.line(SCL, false);
        Ok(())
    }

    fn stop(&self) {
        self.line(SDA, false);
        let _ = self.scl_high();
        self.line(SDA, true);
    }

    /// Clocks out the 8 bits of out and then ninth, a high bit releases SDA, and returns the bits on
    /// SDA. Writing sends the data and releases the ninth bit for the acknowledge of the device,
    /// low is acknowledged, reading sends 0xff and pulls the ninth bit low to acknowledge.
    fn byte(&self, out: u8, ninth: bool) -> Result<(u8, bool), i8> {
        let mut data = 0;
        for i in 0..9 {
            self.line(SDA, if i < 8 { out << i & 0x80 != 0 } else { ninth });
            self.scl_high()?;
            data = data << 1 | self.sda() as u16;
            self.line(SCL, false);
        }
        Ok(((data >> 1) as u8, data & 1 != 0))
    }

    /// Writes the bytes of buf in out and then reads read bytes into the start of buf after a
    /// repeated start, only addresses the device if both are empty
    fn transfer(&self, device: u8, buf: &mut [u8; BUF], out: core::ops::Range<usize>, read: usize) -> Result<(), i8> {
        if !out.is_empty() || read == 0 {
            self.start()?;
            if self.byte(device << 1, true)?.1 { return Err(NACK_ADDRESS); }
            for &out in &buf[out] {
                if self.byte(out, true)?.1 { return Err(NACK_DATA); }
            }
        }
        if read > 0 {
            self.start()?;
            if self.byte(device << 1 | 1, true)?.1 { return Err(NACK_ADDRESS); }
            // every byte but the last is acknowledged
            for n in 0..read {
                buf[n] = self.byte(0xff, n + 1 == read)?.0;
            }
        }
        Ok(())
    }
}

/// The start and length of a range of memory cells, None if it is not in the memory
fn memory<S: Storage>(vm: &VirtMach<S>, address: VMAtom, len: VMAtom) -> Option<(usize, usize)> {
    // the memory is smaller than the buffer, so the min only checks that
    if address < 0 || len < 0 || address as usize + len as usize > vm.memory.len().min(BUF) {
        return None;
    }
    Some((address as usize, len as usize))
}

impl <S: Storage> SoftInterrupt<S> for IntI2c<'_> {
    fn name(&self) -> &str {
        return i2c::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        let count = match op {
            0 | 1 | 3 => 2,
            2 | 5 => 3,
            4 | 10 | 11 | 13 => 4,
            12 => 5,
            14 => 6,
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; return; }
        };
        let mut args = [0 as VMAtom; 6];
        for arg in args.iter_mut().take(count) {
            *arg = vm.stack_pop();
        }
        // port, then speed or device, then the arguments of the function
        let [port, device, a, b, c, d] = args;

        if op == 0 {
            if port == 0 && device > 0 {
                // PORTB 0 so releasing a line does not switch on the pull-up
                self.port.portb().modify(|r, w| unsafe { w.bits(r.bits() & !(SDA | SCL)) });
                self.port.ddrb().modify(|r, w| unsafe { w.bits(r.bits() & !(SDA | SCL)) });
                self.delay = Some(LOOPS_AT_1KHZ / (device as u32).min(0xffff) as u16);
            } else {
                vm.error = RuntimeError::InterruptError;
            }
            return;
        }

        let fail = |vm: &mut VirtMach<S>, error| { vm.error = error; vm.stack_push(0); };
        if port != 0 || !(0..128).contains(&device) || self.delay.is_none() {
            return fail(vm, RuntimeError::InterruptError);
        }
        // the bytes in buf from start to end are written, then read bytes are read into it, those
        // of read_mem and write_read_mem go to memory from dest
        let mut buf = [0u8; BUF];
        (buf[0], buf[1]) = (a as u8, b as u8);
        let (mut start, mut end) = (0, match op { 2 | 5 => 1, 4 => 2, _ => 0 });
        let (mut read, mut dest) = ((op == 3 || op == 5) as usize, 0);
        match op {
            10 | 11 | 14 => {
                let Some((address, len)) = memory(vm, a, b) else { return fail(vm, RuntimeError::MemoryOutOfBounds) };
                if op == 11 {
                    (dest, read) = (address, len);
                } else {
                    for n in 0..len { buf[n] = vm.memory[address + n] as u8; }
                    end = len;
                    if op == 14 {
                        let Some(range) = memory(vm, c, d) else { return fail(vm, RuntimeError::MemoryOutOfBounds) };
                        (dest, read) = range;
                    }
                }
            }
            12 => {
                if b < 0 || c < 0 { return fail(vm, RuntimeError::InterruptError); }
                if !vm.has_bin(a as u8) { return fail(vm, RuntimeError::DictionaryOutOfBound); }
                // the part from start that is in the buffer
                end = vm.copy_bin(a as u8, &mut buf).min(BUF);
                start = (b as usize).min(end);
                end = end.min(start.saturating_add(c as usize));
            }
            13 => {
                if !(0..=BUF as VMAtom).contains(&b) { return fail(vm, RuntimeError::InterruptError); }
                read = b as usize;
            }
            _ => {}
        }

        let res = self.transfer(device as u8, &mut buf, start..end, read);
        self.stop();
        let result = match res {
            Err(status) => status as VMAtom,
            Ok(()) => match op {
                1 => 0,
                3 | 5 => buf[0] as VMAtom,
                4 => 1,
                11 | 14 => {
                    for n in 0..read { vm.memory[dest + n] = buf[n] as VMAtom; }
                    read as VMAtom
                }
                13 => { vm.set_bin(a as u8, &buf[..read]); read as VMAtom }
                _ => (end - start) as VMAtom
            }
        };
        vm.stack_push(result);
    }
}

//! The uart interrupt on the ATtiny85, a software UART with TX on PB2 and RX on PB0.
//!
//! The ATtiny85 has no UART and Timer0, which a USI UART would need, drives pwm, so the bits are
//! timed with the Timer1 clock of clock.rs and the features choose the directions: uart_tx
//! compiles in write, write_available, write_str and write_mem, uart_rx available, read, read_str
//! and read_mem. The functions of a direction that is left out set
//! RuntimeError::UnimplementedInterruptFunc. gpio must leave the pins alone, RX shares PB0 with
//! pwm, so pwm can only use pin 1 together with uart_rx.
//!
//! Only port 0 exists, other ports set RuntimeError::InterruptError, as do the functions before
//! setup. setup takes 1200, 2400, 4800 and 9600 baud, divided by 10 like in the template, so 120,
//! 240, 480 or 960, and all configs: 5 to 8 data bits, no, even or odd parity, one or two stop
//! bits. With uart_rx the baud rate needs RX_CYCLES per bit, at the 1 MHz of the factory fuses
//! that is up to 4800 baud, at 8 MHz all of them: the pin change interrupt cannot sample the first
//! data bit in time at 9600. Other baud rates and configs set RuntimeError::InterruptError and
//! keep the last setup.
//!
//! Timer1 counts in steps of clock::US_PER_COUNT, 8 microseconds, at 9600 baud a bit takes 13 of
//! them. Every bit is timed from the start of its byte, so the steps do not add up. The bit times
//! are a table and only added up, the ATtiny85 has no instructions to divide or multiply.
//!
//! Sending blocks until the byte is out, with interrupts disabled so nothing delays the bits,
//! then the uart is free again: write_available always returns 1. The pin change interrupt of RX
//! receives a byte as soon as its start bit comes in, also during the program and the waits of
//! the time interrupt, into a buffer of RX_BUF bytes. It drops the bytes that do not fit, reading
//! skips those with a wrong parity or stop bit. The uart is half duplex: a byte that comes in
//! while one is sent is lost. read_str reads at most RX_BUF and write_str sends at most TX_BUF
//! bytes. Without the alloc feature strings cannot be set, so read_str reads and then sets
//! RuntimeError::DictionaryOutOfBound. Memory ranges outside the memory set
//! RuntimeError::MemoryOutOfBounds.

use core::cell::Cell;
#[cfg(feature = "uart_rx")]
use core::cell::RefCell;
#[cfg(feature = "uart_rx")]
use attiny_hal::clock::Clock as _;
use attiny_hal::pac::PORTB;
#[cfg(feature = "uart_rx")]
use attiny_hal::pac::EXINT;
use avr_device::interrupt::{ self, CriticalSection, Mutex };
use virtmach::{ RuntimeError, Storage, VirtMach, VMAtom, interrupts::{ SoftInterrupt, uart } };
use crate::clock::{ self, US_PER_COUNT };

#[cfg(feature = "uart_tx")]
const TX: u8 = 1 << 2;
#[cfg(feature = "uart_rx")]
const RX: u8 = 1 << 0;

#[cfg(feature = "uart_rx")]
pub const RX_BUF: usize = 16;
#[cfg(feature = "uart_tx")]
pub const TX_BUF: usize = 32;

/// Cycles from the pin change to the read of the count in __vector_2: 2 to synchronize the pin, up
/// to 4 to finish the instruction that runs, 4 to enter the interrupt, 2 for the rjmp of the vector
/// table and 2 for the push
#[cfg(feature = "uart_rx")]
const LATENCY_CYCLES: u32 = 12;

/// LATENCY_CYCLES in Timer1 counts, rounded
#[cfg(feature = "uart_rx")]
const LATENCY: u8 = ((LATENCY_CYCLES * 1_000_000 / crate::Clock::FREQ + US_PER_COUNT / 2) / US_PER_COUNT) as u8;

/// The baud rates / 10 setup takes
const BAUDS: [VMAtom; 4] = [120, 240, 480, 960];

/// Cycles per bit receiving needs at least: the pin change interrupt samples the first data bit
/// 1.5 bits after the start, its prologue and setup take some 200 cycles before that. So 4800
/// baud at 1 MHz.
#[cfg(feature = "uart_rx")]
const RX_CYCLES: u32 = 200;

/// Which of the BAUDS have RX_CYCLES, computed here so the firmware does not divide
#[cfg(feature = "uart_rx")]
const RX_BAUDS: [bool; 4] = {
    let mut ok = [false; 4];
    let mut i = 0;
    while i < 4 {
        ok[i] = crate::Clock::FREQ / 10 / BAUDS[i] as u32 >= RX_CYCLES;
        i += 1;
    }
    ok
};

/// Timer1 counts for half a bit of the BAUDS, times 16
const HALF_BITS: [u16; 4] = {
    let mut half = [0; 4];
    let mut i = 0;
    while i < 4 {
        half[i] = ((1_000_000 * 16 / 2 / US_PER_COUNT / 10 + BAUDS[i] as u32 / 2) / BAUDS[i] as u32) as u16;
        i += 1;
    }
    half
};

/// The format of the bytes, set by setup
#[derive(Clone, Copy)]
struct Frame {
    /// Timer1 counts for a bit and for one and a half bits, the middle of the first data bit,
    /// times 16. Computed by setup, the ATtiny85 multiplies in software, a hundred cycles.
    bit: u16,
    #[cfg(feature = "uart_rx")]
    first: u16,
    data: u8,
    /// 0 none, 1 even, 2 odd
    parity: u8,
    stop: u8
}

impl Frame {
    /// From the baud rate / 10 and the config of setup, None if they are not supported
    fn new(baud: VMAtom, config: VMAtom) -> Option<Frame> {
        let rate = BAUDS.iter().position(|b| *b == baud)?;
        #[cfg(feature = "uart_rx")]
        if !RX_BAUDS[rate] {
            return None;
        }
        if !(0..32).contains(&config) || config >> 2 & 3 == 3 {
            return None;
        }
        let half = HALF_BITS[rate];
        Some(Frame { bit: 2 * half, #[cfg(feature = "uart_rx")] first: half + 2 * half, data: 8 - (config & 3) as u8, parity: (config >> 2 & 3) as u8, stop: 1 + (config >> 4 & 1) as u8 })
    }

    /// Bits of a byte: start, data, parity and stop
    fn bits(&self) -> u8 {
        1 + self.data + (self.parity != 0) as u8 + self.stop
    }

    /// The parity bit of the data bits of byte
    fn parity_bit(&self, byte: u8) -> u16 {
        ((byte.count_ones() as u8 & 1) ^ (self.parity == 2) as u8) as u16
    }

    /// The bits of byte in the order they are sent, the first is bit 0
    #[cfg(feature = "uart_tx")]
    fn encode(&self, byte: u8) -> u16 {
        let byte = byte & (0xff >> (8 - self.data));
        let mut bits = (byte as u16) << 1;
        if self.parity != 0 { bits |= self.parity_bit(byte) << (1 + self.data); }
        // everything after the parity bit is high, so the line stays high after the stop bits
        bits | !0 << (1 + self.data + (self.parity != 0) as u8)
    }

    /// The byte of the bits after the start bit that receive shifted in from the top, None if the
    /// parity or a stop bit is wrong
    #[cfg(feature = "uart_rx")]
    fn decode_received(&self, bits: u16) -> Option<u8> {
        self.decode(bits >> (17 - self.bits()) << 1)
    }

    /// The byte of the received bits, None if the start, parity or a stop bit is wrong
    #[cfg(feature = "uart_rx")]
    fn decode(&self, bits: u16) -> Option<u8> {
        let byte = (bits >> 1) as u8 & (0xff >> (8 - self.data));
        let stops = 1 + self.data + (self.parity != 0) as u8;
        let stop_mask = ((1u16 << self.stop) - 1) << stops;
        let parity_ok = self.parity == 0 || bits >> (1 + self.data) & 1 == self.parity_bit(byte);
        if bits & 1 == 0 && bits & stop_mask == stop_mask && parity_ok { Some(byte) } else { None }
    }
}

/// The Timer1 count at a time from start in counts times 16, rounded
#[inline(always)]
fn at(start: u8, time: u16) -> u8 {
    start.wrapping_add(((time + 8) >> 4) as u8)
}

/// None until setup, shared with the pin change interrupt
static FRAME: Mutex<Cell<Option<Frame>>> = Mutex::new(Cell::new(None));

/// Waits until Timer1 reaches the count deadline, counted from the count from before it, so they
/// can be up to 255 counts apart: 1.5 bits at 1200 baud are 156. So at most one overflow of
/// Timer1 happens during a wait, a pending one is counted before it and the loop stays short.
#[inline(always)]
fn wait(cs: CriticalSection, from: u8, deadline: u8) {
    clock::count_overflow(cs);
    let span = deadline.wrapping_sub(from);
    while clock::tcnt1().wrapping_sub(from) < span {}
}

/// The received bytes as receive sampled them, they are only checked when they are read, so the
/// pin change interrupt is done right after the last one
#[cfg(feature = "uart_rx")]
struct Ring { buf: [u16; RX_BUF], start: u8, len: u8 }

#[cfg(feature = "uart_rx")]
static RECEIVED: Mutex<RefCell<Ring>> = Mutex::new(RefCell::new(Ring { buf: [0; RX_BUF], start: 0, len: 0 }));

#[cfg(feature = "uart_rx")]
impl Ring {
    /// Drops the bits if the buffer is full
    #[inline(always)]
    fn push(&mut self, bits: u16) {
        if (self.len as usize) < RX_BUF {
            self.buf[(self.start as usize + self.len as usize) % RX_BUF] = bits;
            self.len += 1;
        }
    }

    fn pop(&mut self) -> Option<u16> {
        if self.len == 0 { return None; }
        let bits = self.buf[self.start as usize];
        self.start = ((self.start as usize + 1) % RX_BUF) as u8;
        self.len -= 1;
        Some(bits)
    }

    /// The i-th bits from the start, the pin change interrupt only adds after them
    fn get(&self, i: usize) -> u16 {
        self.buf[(self.start as usize + i) % RX_BUF]
    }
}

// The bytes are checked with interrupts enabled, interrupts are only disabled to take the bits:
// at 1 MHz checking all of them would delay the pin change interrupt of a byte by bits.

/// How many bytes pop returns
#[cfg(feature = "uart_rx")]
fn available(frame: Frame) -> usize {
    let len = interrupt::free(|cs| RECEIVED.borrow(cs).borrow().len) as usize;
    (0..len).filter(|i| frame.decode_received(interrupt::free(|cs| RECEIVED.borrow(cs).borrow().get(*i))).is_some()).count()
}

/// The next byte that has the right parity and stop bits, the others are dropped
#[cfg(feature = "uart_rx")]
fn pop(frame: Frame) -> Option<u8> {
    loop {
        let bits = interrupt::free(|cs| RECEIVED.borrow(cs).borrow_mut().pop())?;
        if let Some(byte) = frame.decode_received(bits) { return Some(byte); }
    }
}

/// Up to max received bytes into buf, returns how many
#[cfg(feature = "uart_rx")]
fn read(frame: Frame, buf: &mut [u8], max: usize) -> usize {
    let mut n = 0;
    while n < max.min(buf.len()) {
        let Some(byte) = pop(frame) else { break };
        buf[n] = byte;
        n += 1;
    }
    n
}

/// The Timer1 count and PINB at the start of the pin change interrupt
#[cfg(feature = "uart_rx")]
#[no_mangle]
static mut UART_RX_EDGE: u8 = 0;
#[cfg(feature = "uart_rx")]
#[no_mangle]
static mut UART_RX_PINB: u8 = 0;

// The pin change interrupt reads the count and the pin before anything else and then continues
// in uart_rx_pin_change: the prologue of a Rust interrupt pushes some 30 registers first, at 1 MHz
// most of a start bit at 9600 baud, and that changes with the code. TCNT1 is I/O register 0x2f,
// PINB 0x16.
#[cfg(feature = "uart_rx")]
core::arch::global_asm!(
    ".global __vector_2",
    "__vector_2:",
    "push r24",
    "in r24, 0x2f",
    "sts UART_RX_EDGE, r24",
    "in r24, 0x16",
    "sts UART_RX_PINB, r24",
    "pop r24",
    "rjmp uart_rx_pin_change",
);

/// The rest of the pin change interrupt, receives the bytes from a start bit on, see receive
#[cfg(feature = "uart_rx")]
#[no_mangle]
pub unsafe extern "avr-interrupt" fn uart_rx_pin_change() {
    let (start, pinb) = unsafe { (UART_RX_EDGE.wrapping_sub(LATENCY), UART_RX_PINB) };
    // interrupts are disabled in an interrupt, free only hands out the CriticalSection
    interrupt::free(|cs| {
        let port = unsafe { PORTB::steal() };
        // a falling edge, the rising ones end a start bit or a byte
        if let Some(frame) = FRAME.borrow(cs).get() {
            if pinb & RX == 0 {
                receive(cs, &port, frame, start);
            }
        }
        // the edges of the bytes set the flag again
        unsafe { EXINT::steal() }.gifr().write(|w| w.pcif().set_bit());
    });
}

/// Timer1 counts after the middle of the stop bits in which receive waits for another byte, 100
/// are 3.8 bits at 4800 baud and more than half a bit at 1200
#[cfg(feature = "uart_rx")]
const WINDOW: u8 = 100;

/// Receives the byte whose start bit made the pin change at the Timer1 count start and the ones
/// that follow it within WINDOW, so bytes sent back to back or with short gaps are not lost:
/// returning from the interrupt and entering it again takes longer than the half bit that is left
/// after the middle of the stop bit. A byte that starts while the interrupt returns, some 70
/// cycles after WINDOW, is sampled that much late. Every bit after the start bit, which was low
/// when the interrupt began, is sampled in its middle with everything but the read of the pin done
/// before the wait, at 1 MHz an instruction takes a microsecond. The interrupt needs some 200
/// cycles until it can sample the first data bit, which RX_CYCLES leaves time for. The bits go
/// into RECEIVED as they are, the ones that do not fit are received and dropped.
#[cfg(feature = "uart_rx")]
#[inline(always)]
fn receive(cs: CriticalSection, port: &PORTB, frame: Frame, mut start: u8) {
    let n = frame.bits();
    let mut received = RECEIVED.borrow(cs).borrow_mut();
    loop {
        // shifted in from the top, the start bit is the 0 at the bottom
        let mut bits = 0u16;
        let mut time = frame.first;
        let mut last = start;
        for _ in 1..n {
            let deadline = at(start, time);
            time += frame.bit;
            bits >>= 1;
            wait(cs, last, deadline);
            if port.pinb().read().bits() & RX != 0 { bits |= 0x8000; }
            last = deadline;
        }
        received.push(bits);
        // last is the middle of the stop bits
        clock::count_overflow(cs);
        let next = loop {
            if port.pinb().read().bits() & RX == 0 { break Some(clock::tcnt1()); }
            if clock::tcnt1().wrapping_sub(last) >= WINDOW { break None; }
        };
        match next {
            Some(next) => start = next,
            None => break
        }
    }
}

/// The start and length of a range of memory cells, None if it is not in the memory
fn memory<S: Storage>(vm: &VirtMach<S>, address: VMAtom, len: VMAtom) -> Option<(usize, usize)> {
    if address < 0 || len < 0 || address as usize + len as usize > vm.memory.len() {
        return None;
    }
    Some((address as usize, len as usize))
}

pub struct IntUart<'a> {
    pub port: &'a PORTB
}

impl IntUart<'_> {
    /// Sends a byte with interrupts disabled, not inlined so the loops over bytes share it
    #[cfg(feature = "uart_tx")]
    #[inline(never)]
    fn send(&self, frame: Frame, byte: u8) {
        interrupt::free(|cs| {
            let mut bits = frame.encode(byte);
            // nothing else writes PORTB while interrupts are disabled, so every bit is one write
            // right after the wait, the next value and deadline are ready before it: at 1 MHz an
            // instruction takes a microsecond
            let low = self.port.portb().read().bits() & !TX;
            let value = |bits: u16| if bits & 1 != 0 { low | TX } else { low };
            let start = clock::tcnt1();
            self.port.portb().write(|w| unsafe { w.bits(value(bits)) });
            // the end of each bit
            let mut time = 0;
            let mut last = start;
            for _ in 0..frame.bits() {
                time += frame.bit;
                let deadline = at(start, time);
                bits >>= 1;
                let next = value(bits);
                wait(cs, last, deadline);
                self.port.portb().write(|w| unsafe { w.bits(next) });
                last = deadline;
            }
            // a byte that came in meanwhile is lost, its start bit is long gone
            #[cfg(feature = "uart_rx")]
            unsafe { EXINT::steal() }.gifr().write(|w| w.pcif().set_bit());
        });
    }

    fn setup(&self, frame: Frame) {
        interrupt::free(|cs| FRAME.borrow(cs).set(Some(frame)));
        // TX an output that idles high
        #[cfg(feature = "uart_tx")]
        {
            self.port.portb().modify(|r, w| unsafe { w.bits(r.bits() | TX) });
            self.port.ddrb().modify(|r, w| unsafe { w.bits(r.bits() | TX) });
        }
        // RX an input with the pull-up, so it idles high when nothing is connected, and its pin
        // change interrupt
        #[cfg(feature = "uart_rx")]
        {
            self.port.ddrb().modify(|r, w| unsafe { w.bits(r.bits() & !RX) });
            self.port.portb().modify(|r, w| unsafe { w.bits(r.bits() | RX) });
            let exint = unsafe { EXINT::steal() };
            exint.pcmsk().modify(|r, w| unsafe { w.bits(r.bits() | RX) });
            exint.gifr().write(|w| w.pcif().set_bit());
            exint.gimsk().modify(|_, w| w.pcie().set_bit());
        }
    }
}

impl <S: Storage> SoftInterrupt<S> for IntUart<'_> {
    fn name(&self) -> &str {
        return uart::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        // only the functions of the directions the features compile in
        let count = match op {
            0 => 3,
            #[cfg(feature = "uart_rx")]
            1 | 2 => 1,
            #[cfg(feature = "uart_rx")]
            10 | 11 => 3,
            #[cfg(feature = "uart_tx")]
            3 => 2,
            #[cfg(feature = "uart_tx")]
            4 => 1,
            #[cfg(feature = "uart_tx")]
            12 => 4,
            #[cfg(feature = "uart_tx")]
            13 => 3,
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; return; }
        };
        let mut args = [0 as VMAtom; 4];
        for arg in args.iter_mut().take(count) {
            *arg = vm.stack_pop();
        }
        // port, then the arguments of the function
        #[allow(unused_variables)]
        let [port, a, b, c] = args;

        let frame = interrupt::free(|cs| FRAME.borrow(cs).get());
        let fail = |vm: &mut VirtMach<S>, error| {
            vm.error = error;
            match op { 0 | 3 => {} 2 => vm.stack_push(-1), _ => vm.stack_push(0) }
        };
        if port != 0 {
            return fail(vm, RuntimeError::InterruptError);
        }
        if op == 0 {
            match Frame::new(a, b) {
                Some(frame) => self.setup(frame),
                None => vm.error = RuntimeError::InterruptError
            }
            return;
        }
        #[allow(unused_variables)]
        let Some(frame) = frame else { return fail(vm, RuntimeError::InterruptError) };

        let result = match op {
            #[cfg(feature = "uart_rx")]
            1 => available(frame) as VMAtom,
            #[cfg(feature = "uart_rx")]
            2 => pop(frame).map_or(-1, |byte| byte as VMAtom),
            #[cfg(feature = "uart_rx")]
            10 => {
                if b < 0 { return fail(vm, RuntimeError::InterruptError); }
                let mut buf = [0u8; RX_BUF];
                let n = read(frame, &mut buf, b as usize);
                vm.set_bin(a as u8, &buf[..n]);
                n as VMAtom
            }
            #[cfg(feature = "uart_rx")]
            11 => {
                if b < 0 { return fail(vm, RuntimeError::InterruptError); }
                // only as many as there are, so the range is checked for those
                let Some((address, len)) = memory(vm, a, b.min(available(frame) as VMAtom)) else { return fail(vm, RuntimeError::MemoryOutOfBounds) };
                let mut n = 0;
                while n < len {
                    let Some(byte) = pop(frame) else { break };
                    vm.memory[address + n] = byte as VMAtom;
                    n += 1;
                }
                n as VMAtom
            }
            #[cfg(feature = "uart_tx")]
            3 => { self.send(frame, a as u8); return; }
            #[cfg(feature = "uart_tx")]
            4 => 1,
            #[cfg(feature = "uart_tx")]
            12 => {
                if b < 0 || c < 0 { return fail(vm, RuntimeError::InterruptError); }
                if !vm.has_bin(a as u8) { return fail(vm, RuntimeError::DictionaryOutOfBound); }
                let mut buf = [0u8; TX_BUF];
                // the part from start that is in the buffer
                let len = vm.copy_bin(a as u8, &mut buf).min(TX_BUF);
                let start = (b as usize).min(len);
                let end = len.min(start.saturating_add(c as usize));
                for byte in &buf[start..end] { self.send(frame, *byte); }
                (end - start) as VMAtom
            }
            #[cfg(feature = "uart_tx")]
            13 => {
                let Some((address, len)) = memory(vm, a, b) else { return fail(vm, RuntimeError::MemoryOutOfBounds) };
                for i in 0..len { self.send(frame, vm.memory[address + i] as u8); }
                len as VMAtom
            }
            _ => 0
        };
        vm.stack_push(result);
    }
}

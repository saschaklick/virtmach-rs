//! The .mmcu section read by simavr, built with the simavr feature only.
//!
//! It tells simavr the mcu and clock and to trace the pin PB1 into blink.vcd, the layout follows
//! simavr's avr_mcu_section.h. The pin follows PORTB and the timer outputs, so pwm shows too.

const TAG_NAME: u8 = 1;
const TAG_FREQUENCY: u8 = 2;
const TAG_VCD_FILENAME: u8 = 12;
const TAG_VCD_PERIOD: u8 = 13;
const TAG_VCD_PORTPIN: u8 = 15;

#[repr(C, packed)]
struct Str<const N: usize> { tag: u8, len: u8, string: [u8; N] }

#[repr(C, packed)]
struct Long { tag: u8, len: u8, value: u32 }

#[repr(C, packed)]
/// mask is the port letter and what the pin for TAG_VCD_PORTPIN
struct Trace { tag: u8, len: u8, mask: u8, what: u16, name: [u8; 32] }

#[repr(C, packed)]
struct Mmcu { name: Str<9>, frequency: Long, vcd_filename: Str<10>, vcd_period: Long, trace: Trace, end: [u8; 2] }

const fn name32(name: &[u8]) -> [u8; 32] {
    let mut res = [0u8; 32];
    let mut i = 0;
    while i < name.len() { res[i] = name[i]; i += 1; }
    res
}

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".mmcu")]
static SIMAVR_MMCU: Mmcu = Mmcu {
    name: Str { tag: TAG_NAME, len: 9, string: *b"attiny85\0" },
    frequency: Long { tag: TAG_FREQUENCY, len: 4, value: 1_000_000 },
    vcd_filename: Str { tag: TAG_VCD_FILENAME, len: 10, string: *b"blink.vcd\0" },
    vcd_period: Long { tag: TAG_VCD_PERIOD, len: 4, value: 10_000 },
    trace: Trace { tag: TAG_VCD_PORTPIN, len: 35, mask: b'B', what: 1, name: name32(b"PB1") },
    end: [0, 0]
};

//! Sine, cosine, arc tangent and angle conversions of the math interrupt.
//!
//! Angles are atoms that wrap around the circle: the whole range MIN..MAX is one turn, so
//! 0 is 0 degrees, MIN is 180 degrees and MAX is just below it. Sine and cosine return
//! -MAX..MAX for -1..1.
//!
//! With 8 and 16 bit atoms everything is done in 16 bit integers, apart from the 16 bit scale, which
//! multiplies two atoms. They use a quarter wave table of 65 values with linear interpolation
//! for sine and cosine (within 3 of the exact value), and a CORDIC in 16 bit integers for atan2
//! (within 4 of 65536 parts of a turn). With 32 bit atoms the calculations are done in Q15 fixed
//! point with 32 bit integers.

use crate::{Storage, VirtMach, VMAtom, RuntimeError};

#[cfg(not(feature = "i32"))]
use narrow as imp;
#[cfg(feature = "i32")]
use wide as imp;
pub use imp::{ sin, cos, atan2, deg_to_rad, rad_to_deg, scale };

#[cfg(not(feature = "i32"))]
mod narrow {
    use crate::VMAtom;

    const BITS: u32 = VMAtom::BITS;

    /// Shifts an angle to the 16 bit turn
    const SHIFT: u32 = 16 - BITS;

    /// Entry i of a table. On AVR the tables are in the flash, where normal loads do not reach, so they
    /// are read with LPM, see examples/attiny/src/flash.rs, and do not take any of the little RAM.
    #[cfg(target_arch = "avr")]
    fn flash_byte(address: u16) -> u8 {
        let byte: u8;
        unsafe { core::arch::asm!("lpm {0}, Z", out(reg) byte, in("Z") address, options(pure, readonly, nostack, preserves_flags)) };
        byte
    }

    #[cfg(all(target_arch = "avr", not(feature = "i8")))]
    fn read<const N: usize>(table: &'static [u16; N], i: usize) -> u16 {
        let address = (table.as_ptr() as u16).wrapping_add(2 * i as u16);
        u16::from_le_bytes([flash_byte(address), flash_byte(address.wrapping_add(1))])
    }

    #[cfg(all(not(target_arch = "avr"), not(feature = "i8")))]
    fn read<const N: usize>(table: &'static [u16; N], i: usize) -> u16 {
        table[i]
    }

    #[cfg(all(target_arch = "avr", feature = "i8"))]
    fn read<const N: usize>(table: &'static [u8; N], i: usize) -> u8 {
        flash_byte((table.as_ptr() as u16).wrapping_add(i as u16))
    }

    #[cfg(all(not(target_arch = "avr"), feature = "i8"))]
    fn read<const N: usize>(table: &'static [u8; N], i: usize) -> u8 {
        table[i]
    }

    /// The first quarter of the sine, 0 to 90 degrees in 64 steps, scaled to MAX. With 16 bit atoms the
    /// values are raised a little, which halves the error of the linear interpolation between them.
    #[cfg(not(feature = "i8"))]
    #[cfg_attr(target_arch = "avr", unsafe(link_section = ".progmem.data"))]
    static TABLE: [u16; 65] = [
        0, 804, 1608, 2411, 3212, 4011, 4808, 5602, 6393, 7180, 7962, 8740, 9512, 10279, 11039, 11793,
        12540, 13279, 14010, 14733, 15447, 16152, 16846, 17531, 18205, 18868, 19520, 20160, 20788, 21403, 22006, 22595,
        23171, 23732, 24280, 24813, 25330, 25833, 26320, 26791, 27246, 27685, 28106, 28511, 28899, 29270, 29622, 29957,
        30274, 30573, 30853, 31115, 31357, 31581, 31786, 31972, 32139, 32286, 32414, 32522, 32611, 32680, 32729, 32759, 32767
    ];

    /// The same for 8 bit atoms, an angle has 256 steps, so there is no need to interpolate
    #[cfg(feature = "i8")]
    #[cfg_attr(target_arch = "avr", unsafe(link_section = ".progmem.data"))]
    static TABLE: [u8; 65] = [
        0, 3, 6, 9, 12, 16, 19, 22, 25, 28, 31, 34, 37, 40, 43, 46,
        49, 51, 54, 57, 60, 63, 65, 68, 71, 73, 76, 78, 81, 83, 85, 88,
        90, 92, 94, 96, 98, 100, 102, 104, 106, 107, 109, 111, 112, 113, 115, 116,
        117, 118, 120, 121, 122, 122, 123, 124, 125, 125, 126, 126, 126, 127, 127, 127, 127
    ];

    /// atan(2^-i) in 65536 parts of a turn, rounded
    #[cfg_attr(target_arch = "avr", unsafe(link_section = ".progmem.data"))]
    static ATAN: [u16; 15] = [8192, 4836, 2555, 1297, 651, 326, 163, 81, 41, 20, 10, 5, 3, 1, 1];

    fn read_atan(i: usize) -> u16 {
        #[cfg(target_arch = "avr")]
        {
            let address = (ATAN.as_ptr() as u16).wrapping_add(2 * i as u16);
            u16::from_le_bytes([flash_byte(address), flash_byte(address.wrapping_add(1))])
        }
        #[cfg(not(target_arch = "avr"))]
        ATAN[i]
    }

    /// The angle in 65536 parts of a turn
    fn to_turn(angle: VMAtom) -> u16 {
        (angle as u16) << SHIFT
    }

    /// 65536 parts of a turn to an angle, rounded
    fn from_turn(turn: u16) -> VMAtom {
        (turn.wrapping_add((1u16 << SHIFT) >> 1) >> SHIFT) as VMAtom
    }

    /// The sine of x, 0 to 16384 parts of a turn, 0..=MAX
    fn quarter(x: u16) -> u16 {
        #[cfg(feature = "i8")]
        return read(&TABLE, (x >> 8) as usize) as u16;
        #[cfg(not(feature = "i8"))]
        {
            let (index, fraction) = ((x >> 8) as usize, x & 0xff);
            if index >= 64 {
                return read(&TABLE, 64);
            }
            let low = read(&TABLE, index);
            let delta = read(&TABLE, index + 1) - low;
            // delta * fraction / 256 in two products that fit into 16 bits
            low + (delta >> 8) * fraction + (((delta & 0xff) * fraction + 128) >> 8)
        }
    }

    fn sin_turn(turn: u16) -> VMAtom {
        let rest = turn & 0x3fff;
        let value = if turn & 0x4000 == 0 { quarter(rest) } else { quarter(0x4000 - rest) } as VMAtom;
        if turn & 0x8000 == 0 { value } else { -value }
    }

    /// Sine, -MAX..MAX
    pub fn sin(angle: VMAtom) -> VMAtom {
        sin_turn(to_turn(angle))
    }

    /// Cosine, -MAX..MAX
    pub fn cos(angle: VMAtom) -> VMAtom {
        sin_turn(to_turn(angle).wrapping_add(0x4000))
    }

    /// The angle of the vector. The part of the vector in the first octant is scaled to 14 bits and
    /// turned onto the x axis with a CORDIC, then the octant, quadrant and signs are added.
    pub fn atan2(y: VMAtom, x: VMAtom) -> VMAtom {
        let (ax, ay) = ((x as i16).unsigned_abs(), (y as i16).unsigned_abs());
        let swap = ay > ax;
        let (high, low) = if swap { (ay, ax) } else { (ax, ay) };
        if high == 0 {
            return 0;
        }
        // the larger part gets its top bit at bit 13, the CORDIC adds about 16% to it
        let top = 15 - high.leading_zeros();
        let (mut cx, mut cy) = if top <= 13 {
            ((high << (13 - top)) as i16, (low << (13 - top)) as i16)
        } else {
            ((high >> (top - 13)) as i16, (low >> (top - 13)) as i16)
        };
        // the angle is at most 45 degrees, so it starts with the second step. Without y it would
        // not stop at 0 but turn back and forth around it.
        let mut turn = 0u16;
        for i in 1..if cy == 0 { 1 } else { ATAN.len() } {
            let (dx, dy) = (cx >> i, cy >> i);
            if cy >= 0 {
                (cx, cy, turn) = (cx + dy, cy - dx, turn.wrapping_add(read_atan(i)));
            } else {
                (cx, cy, turn) = (cx - dy, cy + dx, turn.wrapping_sub(read_atan(i)));
            }
        }
        if swap { turn = 0x4000u16.wrapping_sub(turn); }
        if x < 0 { turn = 0x8000u16.wrapping_sub(turn); }
        if y < 0 { turn = 0u16.wrapping_sub(turn); }
        from_turn(turn)
    }

    /// Degrees to an angle, rounded and wrapped around the circle
    pub fn deg_to_rad(deg: VMAtom) -> VMAtom {
        // deg * 65536 / 360 is deg * 182 + deg * 2 / 45, which fits into 16 bits
        let deg = (deg as i16).rem_euclid(360) as u16;
        from_turn(deg * 182 + (deg * 2 + 22) / 45)
    }

    /// An angle to rounded degrees, clamped with carry set if they do not fit into an atom
    pub fn rad_to_deg(angle: VMAtom) -> (VMAtom, bool) {
        // turn * 360 / 65536 is turn * 45 / 8192, split at the high byte to stay in 16 bits
        let turn = to_turn(angle) as i16;
        let (high, low) = (turn >> 8, (turn as u16 & 0xff) * 45);
        let high = high * 45;
        let rest = ((high & 31) as u16 * 256 + low + 4096) >> 13;
        let deg = (high >> 5) + rest as i16;
        let clamped = deg.clamp(VMAtom::MIN as i16, VMAtom::MAX as i16);
        (clamped as VMAtom, clamped != deg)
    }

    /// value * factor / MAX, rounded, clamped with carry set if it does not fit into an atom. The
    /// division is a shift: x / MAX is (x + x / (MAX + 1) + (MAX + 1) / 2) / (MAX + 1).
    pub fn scale(value: VMAtom, factor: VMAtom) -> (VMAtom, bool) {
        #[cfg(feature = "i8")]
        type Wide = u16;
        #[cfg(not(feature = "i8"))]
        type Wide = u32;
        let one = 1 << (BITS - 1);
        let product = (value.unsigned_abs() as Wide) * (factor.unsigned_abs() as Wide);
        let res = (product + (product >> (BITS - 1)) + (one >> 1)) >> (BITS - 1);
        let negative = (value < 0) != (factor < 0) && res != 0;
        if negative {
            if res > one { (VMAtom::MIN, true) } else { ((res as VMAtom).wrapping_neg(), false) }
        } else if res >= one {
            (VMAtom::MAX, true)
        } else {
            (res as VMAtom, false)
        }
    }
}

#[cfg(feature = "i32")]
mod wide {
    use crate::VMAtom;

    const BITS: u32 = VMAtom::BITS;

    /// 1.0 in the Q15 fixed point format of the calculations, values are i16 and products i32
    const ONE: i32 = 1 << 15;

    /// π/4 in Q15
    const QUARTER_PI: i32 = 25736;

    /// Odd polynomial for atan on 0..1 in Q15 (Abramowitz and Stegun 4.4.49, error below 1e-5)
    const ATAN_POLY: [i32; 5] = [32763, -10823, 5903, -2790, 683];

    /// 2^16 parts of a turn per radian, in Q15 of the result
    const TURN_PER_RAD: i32 = 10430;

    /// The angle in 2^16 parts of a turn
    fn to_turn(angle: VMAtom) -> u16 {
        (((angle as i32) << 16u32.saturating_sub(BITS)) >> BITS.saturating_sub(16)) as u16
    }

    /// 2^16 parts of a turn to an angle, rounded
    fn from_turn(turn: u16) -> VMAtom {
        let half = (1u16 << 16u32.saturating_sub(BITS)) >> 1;
        (((turn.wrapping_add(half) as i16 as i32) >> 16u32.saturating_sub(BITS)) << BITS.saturating_sub(16)) as VMAtom
    }

    /// Q15 multiplication, rounded
    fn mul(a: i32, b: i32) -> i32 {
        (a * b + (ONE >> 1)) >> 15
    }

    /// Taylor series of sin and cos for 0..π/4 in Q15
    fn sin_cos_octant(x: i32) -> (i32, i32) {
        let x2 = mul(x, x);
        let sin = mul(x, ONE - mul(x2, ONE - mul(x2, ONE - x2 / 42) / 20) / 6);
        let cos = ONE - mul(x2, ONE - mul(x2, ONE - x2 / 30) / 12) / 2;
        (sin, cos)
    }

    /// sin and cos of 2^16 parts of a turn in Q15
    fn sin_cos(turn: u16) -> (i32, i32) {
        let quarter = turn >> 14;
        let rest = (turn & (1 << 14) - 1) as i32;
        let (sin, cos) = if rest <= 1 << 13 {
            sin_cos_octant((rest * QUARTER_PI + (1 << 12)) >> 13)
        } else {
            let (sin, cos) = sin_cos_octant((((1 << 14) - rest) * QUARTER_PI + (1 << 12)) >> 13);
            (cos, sin)
        };
        match quarter {
            0 => (sin, cos),
            1 => (cos, -sin),
            2 => (-sin, -cos),
            _ => (-cos, sin)
        }
    }

    /// -VMAtom::MAX..VMAtom::MAX to Q15 -1.0..1.0
    fn to_fixed(value: VMAtom) -> i32 {
        let value = value as i32;
        if BITS > 16 {
            ((value >> 15) + 1) >> 1
        } else {
            let max = VMAtom::MAX as i32;
            (value * ONE + value.signum() * (max / 2)) / max
        }
    }

    /// Q15 -1.0..1.0 to -VMAtom::MAX..VMAtom::MAX
    fn from_fixed(value: i32) -> VMAtom {
        let max = VMAtom::MAX as i32;
        let res = if BITS > 16 {
            if value.abs() >= ONE { value.signum() * max } else { value << 16 }
        } else {
            ((value * max + (ONE >> 1)) >> 15).clamp(-max, max)
        };
        res as VMAtom
    }

    /// atan of 0..1 in Q15 as 2^16 parts of a turn, 0..8192
    fn atan_ratio(z: i32) -> i32 {
        let z2 = mul(z, z);
        let rad = mul(z, ATAN_POLY.iter().rev().fold(0, |acc, c| c + mul(z2, acc)));
        mul(rad, TURN_PER_RAD)
    }

    /// Angle of the vector in 2^16 parts of a turn, from the ratio of its smaller to its larger part
    fn atan2_turn(y: VMAtom, x: VMAtom) -> u16 {
        let (mut ax, mut ay) = ((x as i32).unsigned_abs(), (y as i32).unsigned_abs());
        let shift = (32 - ax.max(ay).leading_zeros()).saturating_sub(15);
        (ax, ay) = (ax >> shift, ay >> shift);
        if ax == 0 && ay == 0 {
            return 0;
        }
        let (min, max) = (ax.min(ay) as i32, ax.max(ay) as i32);
        let octant = atan_ratio(((min << 15) + max / 2) / max);
        let quarter = if ay > ax { (1 << 14) - octant } else { octant };
        let half = if x < 0 { (1 << 15) - quarter } else { quarter };
        (if y < 0 { -half } else { half }) as u16
    }

    /// Degrees to an angle, rounded and wrapped around the circle
    pub fn deg_to_rad(deg: VMAtom) -> VMAtom {
        let deg = (deg as i32).rem_euclid(360);
        from_turn(((deg * 65536 + 180) / 360) as u16)
    }

    /// An angle to rounded degrees, clamped with carry set if they do not fit into an atom
    pub fn rad_to_deg(angle: VMAtom) -> (VMAtom, bool) {
        let deg = (to_turn(angle) as i16 as i32 * 360 + (1 << 15)) >> 16;
        let clamped = deg.clamp(VMAtom::MIN as i32, VMAtom::MAX as i32);
        (clamped as VMAtom, clamped != deg)
    }

    /// value * factor / VMAtom::MAX, rounded, clamped with carry set if it does not fit into an atom.
    /// The factor is taken as Q15 and the value split into 15 bit halves, so the products fit into i32.
    pub fn scale(value: VMAtom, factor: VMAtom) -> (VMAtom, bool) {
        let (value, factor) = (value as i32, to_fixed(factor));
        let res = (value >> 15).checked_mul(factor).and_then(|high| high.checked_add(mul(value & (ONE - 1), factor)));
        match res {
            Some(res) if res >= VMAtom::MIN as i32 && res <= VMAtom::MAX as i32 => (res as VMAtom, false),
            Some(res) if res < 0 => (VMAtom::MIN, true),
            None if (value < 0) != (factor < 0) => (VMAtom::MIN, true),
            _ => (VMAtom::MAX, true)
        }
    }

    pub fn sin(angle: VMAtom) -> VMAtom { from_fixed(sin_cos(to_turn(angle)).0) }

    pub fn cos(angle: VMAtom) -> VMAtom { from_fixed(sin_cos(to_turn(angle)).1) }

    pub fn atan2(y: VMAtom, x: VMAtom) -> VMAtom { from_turn(atan2_turn(y, x)) }
}

/// Calls the trig function op of the math interrupt, 20..=25
pub fn call<S: Storage>(vm: &mut VirtMach<S>, op: VMAtom) {
    let res = match op {
        20 => { let angle = vm.stack_pop(); (sin(angle), false) }
        21 => { let angle = vm.stack_pop(); (cos(angle), false) }
        22 => { let (y, x) = (vm.stack_pop(), vm.stack_pop()); (atan2(y, x), false) }
        23 => { let deg = vm.stack_pop(); (deg_to_rad(deg), false) }
        24 => { let angle = vm.stack_pop(); rad_to_deg(angle) }
        25 => { let (value, factor) = (vm.stack_pop(), vm.stack_pop()); scale(value, factor) }
        _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; (0, false) }
    };
    vm.processor.zero = res.0 == 0;
    vm.processor.carry = res.1;
    vm.stack_push(res.0);
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::f64::consts::PI;

    const BITS: u32 = VMAtom::BITS;
    const MAX: f64 = VMAtom::MAX as f64;
    const TURN: f64 = (1u64 << BITS) as f64;

    /// The angle for radians, with 32 bit atoms only the upper 16 bits count
    fn angle(rad: f64) -> VMAtom {
        let bits = BITS.min(16);
        let steps = ((rad / (2.0 * PI)).rem_euclid(1.0) * (1u64 << bits) as f64).round() as u64;
        (steps << (BITS - bits)) as VMAtom
    }

    #[test]
    fn sin_cos_match_f64() {
        // 8 bit atoms are exact to the rounding, the others within 2 or 3
        let tolerance = if BITS <= 8 { 1.0 } else { 1.0 + MAX * 6e-5 };
        for i in -1000..=1000 {
            let a = angle(i as f64 * PI / 1000.0);
            let rad = a as f64 / TURN * 2.0 * PI;
            assert!((sin(a) as f64 - rad.sin() * MAX).abs() <= tolerance, "sin {} got {}", rad, sin(a));
            assert!((cos(a) as f64 - rad.cos() * MAX).abs() <= tolerance, "cos {} got {}", rad, cos(a));
        }
    }

    #[test]
    fn sin_cos_cover_all_angles() {
        let tolerance = if BITS <= 8 { 1.0 } else { 1.0 + MAX * 6e-5 };
        let step = if BITS > 16 { 65536 } else { 1 };
        for a in (VMAtom::MIN..=VMAtom::MAX).step_by(step) {
            let rad = a as f64 / TURN * 2.0 * PI;
            assert!((sin(a) as f64 - rad.sin() * MAX).abs() <= tolerance, "sin {}", a);
            assert!((cos(a) as f64 - rad.cos() * MAX).abs() <= tolerance, "cos {}", a);
        }
    }

    #[test]
    fn atan2_matches_f64() {
        let tolerance = if BITS <= 8 { 1 } else if BITS <= 16 { 4 } else { 1 << (BITS - 16) };
        for r in [VMAtom::MAX as f64 * 0.9, 50.0, 5.0] {
            for i in -99..=99 {
                let rad = i as f64 * PI / 100.0;
                let (y, x) = ((rad.sin() * r).round() as VMAtom, (rad.cos() * r).round() as VMAtom);
                let expected = angle((y as f64).atan2(x as f64));
                assert!((atan2(y, x).wrapping_sub(expected) as i64).abs() <= tolerance, "atan2 {} {} got {} want {}", r, rad, atan2(y, x), expected);
            }
        }
        assert_eq!(atan2(0, 0), 0);
        assert_eq!(atan2(0, 1), 0);
        assert_eq!(atan2(1, 0), angle(PI / 2.0));
        assert_eq!(atan2(-1, 0), angle(-PI / 2.0));
        assert_eq!(atan2(0, -1), VMAtom::MIN);
        assert_eq!(atan2(5, 5), angle(PI / 4.0));
    }

    #[test]
    fn scaling() {
        assert_eq!(scale(100, VMAtom::MAX), (100, false));
        assert_eq!(scale(100, -VMAtom::MAX), (-100, false));
        assert_eq!(scale(100, 0), (0, false));
        assert_eq!(scale(0, VMAtom::MAX), (0, false));
        assert_eq!(scale(100, VMAtom::MAX / 2 + 1), (50, false));
        assert_eq!(scale(-100, VMAtom::MAX / 2 + 1), (-50, false));
        assert_eq!(scale(VMAtom::MIN, VMAtom::MIN), (VMAtom::MAX, true));
        assert_eq!(scale(VMAtom::MIN, VMAtom::MAX), (VMAtom::MIN, false));
        assert_eq!(scale(VMAtom::MAX, VMAtom::MAX), (VMAtom::MAX, false));
        assert_eq!(scale(20, sin(deg_to_rad(30))), (10, false));
    }

    #[test]
    fn degrees() {
        assert_eq!(deg_to_rad(0), 0);
        assert_eq!(deg_to_rad(90), angle(PI / 2.0));
        assert_eq!(deg_to_rad(-90), angle(-PI / 2.0));
        for deg in [-90, -45, -1, 0, 1, 30, 45, 90] {
            assert_eq!(rad_to_deg(deg_to_rad(deg)), (deg, false), "{}", deg);
        }
        #[cfg(not(feature = "i8"))] {
            assert_eq!(deg_to_rad(180), VMAtom::MIN);
            assert_eq!(deg_to_rad(360), 0);
            assert_eq!(deg_to_rad(-270), angle(PI / 2.0));
            assert_eq!(rad_to_deg(VMAtom::MIN), (-180, false));
        }
        #[cfg(feature = "i8")]
        assert_eq!(rad_to_deg(VMAtom::MIN), (VMAtom::MIN, true));
    }

    #[test]
    fn degrees_match_f64() {
        let step = if BITS > 16 { 65536 } else { 1 };
        for a in (VMAtom::MIN..=VMAtom::MAX).step_by(step) {
            let exact = (a as f64 / TURN * 360.0 + 0.5).floor();
            let (deg, carry) = rad_to_deg(a);
            assert_eq!(deg as f64, exact.clamp(VMAtom::MIN as f64, VMAtom::MAX as f64), "{}", a);
            assert_eq!(carry, exact < VMAtom::MIN as f64 || exact > VMAtom::MAX as f64);
        }
        for deg in -720..720i32 {
            if deg < VMAtom::MIN as i32 || deg > VMAtom::MAX as i32 { continue; }
            let expected = angle(deg as f64 / 360.0 * 2.0 * PI);
            // 8 bit atoms round twice, a step is a degree and a half, so one off is fine
            assert!((deg_to_rad(deg as VMAtom).wrapping_sub(expected) as i64).abs() <= (BITS <= 8) as i64, "{}", deg);
        }
    }
}

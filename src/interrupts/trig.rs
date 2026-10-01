use crate::{Storage, RuntimeError, VirtMach, VMAtom, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "trig";

pub const FUNCTIONS: [SoftInterruptFunction;6] = [
    SoftInterruptFunction { no:  0, name: "sin",        arguments: 1, returns: 1, help: "Sine, -MAX..MAX for -1..1 (angle)->(res)" },
    SoftInterruptFunction { no:  1, name: "cos",        arguments: 1, returns: 1, help: "Cosine, -MAX..MAX for -1..1 (angle)->(res)" },
    SoftInterruptFunction { no:  2, name: "atan2",      arguments: 2, returns: 1, help: "Angle of the vector (y,x)->(angle)" },
    SoftInterruptFunction { no:  3, name: "deg_to_rad", arguments: 1, returns: 1, help: "Degrees to angle, MIN..MAX for -180..180 (deg)->(angle)" },
    SoftInterruptFunction { no:  4, name: "rad_to_deg", arguments: 1, returns: 1, help: "Angle to degrees, MIN..MAX for -180..180 (angle)->(deg)" },
    SoftInterruptFunction { no:  5, name: "scale",      arguments: 2, returns: 1, help: "Multiply by a sin or cos result, value*factor/MAX rounded (value,factor)->(res)" }
];

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
fn atan2(y: VMAtom, x: VMAtom) -> u16 {
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
fn deg_to_rad(deg: VMAtom) -> VMAtom {
    let deg = (deg as i32).rem_euclid(360);
    from_turn(((deg * 65536 + 180) / 360) as u16)
}

/// An angle to rounded degrees, clamped with carry set if they do not fit into an atom
fn rad_to_deg(angle: VMAtom) -> (VMAtom, bool) {
    let deg = (to_turn(angle) as i16 as i32 * 360 + (1 << 15)) >> 16;
    let clamped = deg.clamp(VMAtom::MIN as i32, VMAtom::MAX as i32);
    (clamped as VMAtom, clamped != deg)
}

/// value * factor / VMAtom::MAX, rounded, clamped with carry set if it does not fit into an atom.
/// The factor is taken as Q15 and the value split into 15 bit halves, so the products fit into i32.
fn scale(value: VMAtom, factor: VMAtom) -> (VMAtom, bool) {
    let (value, factor) = (value as i32, to_fixed(factor));
    let res = (value >> 15).checked_mul(factor).and_then(|high| high.checked_add(mul(value & (ONE - 1), factor)));
    match res {
        Some(res) if res >= VMAtom::MIN as i32 && res <= VMAtom::MAX as i32 => (res as VMAtom, false),
        Some(res) if res < 0 => (VMAtom::MIN, true),
        None if (value < 0) != (factor < 0) => (VMAtom::MIN, true),
        _ => (VMAtom::MAX, true)
    }
}

pub struct Interrupt {}

impl <S: Storage> SoftInterrupt<S> for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        let res = match op {
            0 => { let angle = vm.stack_pop(); (from_fixed(sin_cos(to_turn(angle)).0), false) }
            1 => { let angle = vm.stack_pop(); (from_fixed(sin_cos(to_turn(angle)).1), false) }
            2 => { let (y, x) = (vm.stack_pop(), vm.stack_pop()); (from_turn(atan2(y, x)), false) }
            3 => { let deg = vm.stack_pop(); (deg_to_rad(deg), false) }
            4 => { let angle = vm.stack_pop(); rad_to_deg(angle) }
            5 => { let (value, factor) = (vm.stack_pop(), vm.stack_pop()); scale(value, factor) }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; (0, false) }
        };
        vm.processor.zero = res.0 == 0;
        vm.processor.carry = res.1;
        vm.stack_push(res.0);
    }

}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::f64::consts::PI;

    const MAX: f64 = VMAtom::MAX as f64;
    const TURN: f64 = 2.0 * (1u64 << (BITS - 1)) as f64;

    fn angle(rad: f64) -> VMAtom {
        from_turn((rad / (2.0 * PI) * 65536.0).round().rem_euclid(65536.0) as u16)
    }

    #[test]
    fn sin_cos_match_f64() {
        for i in -1000..=1000 {
            let a = angle(i as f64 * PI / 1000.0);
            let rad = a as f64 / TURN * 2.0 * PI;
            let (sin, cos) = sin_cos(to_turn(a));
            assert!((from_fixed(sin) as f64 - rad.sin() * MAX).abs() <= 1.0 + MAX * 6e-5, "sin {}", rad);
            assert!((from_fixed(cos) as f64 - rad.cos() * MAX).abs() <= 1.0 + MAX * 6e-5, "cos {}", rad);
        }
    }

    #[test]
    fn atan2_matches_f64() {
        let tolerance = if BITS <= 8 { 1 } else { 1 << BITS.saturating_sub(16) };
        for r in [VMAtom::MAX as f64 * 0.9, 50.0, 5.0] {
            for i in -99..=99 {
                let rad = i as f64 * PI / 100.0;
                let (y, x) = ((rad.sin() * r).round() as VMAtom, (rad.cos() * r).round() as VMAtom);
                let expected = angle((y as f64).atan2(x as f64));
                assert!((from_turn(atan2(y, x)).wrapping_sub(expected) as i64).abs() <= tolerance, "atan2 {} {} got {} want {}", r, rad, from_turn(atan2(y, x)), expected);
            }
        }
        assert_eq!(from_turn(atan2(0, 0)), 0);
    }

    #[test]
    fn scaling() {
        assert_eq!(scale(100, VMAtom::MAX), (100, false));
        assert_eq!(scale(100, -VMAtom::MAX), (-100, false));
        assert_eq!(scale(100, 0), (0, false));
        assert_eq!(scale(100, VMAtom::MAX / 2 + 1), (50, false));
        assert_eq!(scale(-100, VMAtom::MAX / 2 + 1), (-50, false));
        assert_eq!(scale(VMAtom::MIN, VMAtom::MIN), (VMAtom::MAX, true));
        assert_eq!(scale(VMAtom::MIN, VMAtom::MAX), (VMAtom::MIN, false));
        assert_eq!(scale(20, from_fixed(sin_cos(to_turn(deg_to_rad(30))).0)), (10, false));
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
}

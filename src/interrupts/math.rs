use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};
#[cfg(any(feature = "int_math_sqr", feature = "int_math_pow"))]
use crate::{VMAtom, VMUAtom};
#[cfg(feature = "int_math_random")]
use crate::interrupts::math_random;
#[cfg(feature = "int_math_trig")]
use crate::interrupts::math_trig;

pub const NAME: &str = "math";

pub const INDEX: u8 = 0;

pub const FUNCTIONS: [SoftInterruptFunction;18] = [
    SoftInterruptFunction { no:  0, name: "and", arguments: 2, returns: 1, help: "Logical AND (num0,num1)->(res)" },
    SoftInterruptFunction { no:  1, name: "or",  arguments: 2, returns: 1, help: "Logical OR (num0,num1)->(res)" },
    SoftInterruptFunction { no:  2, name: "xor", arguments: 2, returns: 1, help: "Logical XOR (num0,num1)->(res)" },
    SoftInterruptFunction { no:  3, name: "not", arguments: 1, returns: 1, help: "Logical NOT (num)->(res)" },
    SoftInterruptFunction { no:  4, name: "lsh", arguments: 2, returns: 1, help: "Bitwise left-shift (num,bits_to_shift)->(res)" },
    SoftInterruptFunction { no:  5, name: "rsh", arguments: 2, returns: 1, help: "Bitwise right-shift (num,bits_to_shift)->(res)" },
    SoftInterruptFunction { no:  6, name: "mul", arguments: 2, returns: 1, help: "Multiplication (fac0,fac1)->(res)" },
    SoftInterruptFunction { no:  7, name: "div", arguments: 2, returns: 1, help: "Division (dividend,divisor)->(res)" },
    SoftInterruptFunction { no:  8, name: "mod", arguments: 2, returns: 1, help: "Modulo (dividend,divisor)->(res)" },
    SoftInterruptFunction { no:  9, name: "pow", arguments: 2, returns: 1, help: "Exponentiation (base, exponent)->(res)" },
    SoftInterruptFunction { no: 10, name: "sqr", arguments: 1, returns: 1, help: "Squareroot (num)->(res)" },
    SoftInterruptFunction { no: 11, name: "random", arguments: 2, returns: 1, help: "Generate random value in provided value range (start,end)->(res)" },
    SoftInterruptFunction { no: 20, name: "sin",        arguments: 1, returns: 1, help: "Sine, -MAX..MAX for -1..1 (angle)->(res)" },
    SoftInterruptFunction { no: 21, name: "cos",        arguments: 1, returns: 1, help: "Cosine, -MAX..MAX for -1..1 (angle)->(res)" },
    SoftInterruptFunction { no: 22, name: "atan2",      arguments: 2, returns: 1, help: "Angle of the vector (y,x)->(angle)" },
    SoftInterruptFunction { no: 23, name: "deg_to_rad", arguments: 1, returns: 1, help: "Degrees to angle, MIN..MAX for -180..180 (deg)->(angle)" },
    SoftInterruptFunction { no: 24, name: "rad_to_deg", arguments: 1, returns: 1, help: "Angle to degrees, MIN..MAX for -180..180 (angle)->(deg)" },
    SoftInterruptFunction { no: 25, name: "scale",      arguments: 2, returns: 1, help: "Multiply by a sin or cos result, value*factor/MAX rounded (value,factor)->(res)" }
];

/// Integer square root by shifting and subtracting, None for negative numbers. Unlike
/// checked_isqrt it needs no lookup table, which would take 512 bytes of RAM on AVR. It works in
/// the size of an atom, so an 8 bit atom takes 8 bit steps.
#[cfg(feature = "int_math_sqr")]
fn isqrt(num: VMAtom) -> Option<VMAtom> {
    if num < 0 {
        return None;
    }
    let (mut rem, mut res) = (num as VMUAtom, 0 as VMUAtom);
    let mut bit: VMUAtom = 1 << (VMUAtom::BITS - 2);
    while bit > rem { bit >>= 2; }
    while bit != 0 {
        if rem >= res + bit {
            rem -= res + bit;
            res = (res >> 1) + bit;
        } else {
            res >>= 1;
        }
        bit >>= 2;
    }
    Some(res as VMAtom)
}

/// base to the power of exp by squaring, in the size of an atom, with carry set if anything
/// overflowed. A negative exponent counts as the unsigned number with the same bits.
#[cfg(feature = "int_math_pow")]
fn pow(base: VMAtom, exp: VMAtom) -> (VMAtom, bool) {
    let (mut res, mut square, mut exp, mut carry) = (1 as VMAtom, base, exp as VMUAtom, false);
    while exp != 0 {
        if exp & 1 != 0 {
            let (value, overflow) = res.overflowing_mul(square);
            (res, carry) = (value, carry | overflow);
        }
        exp >>= 1;
        if exp != 0 {
            let (value, overflow) = square.overflowing_mul(square);
            (square, carry) = (value, carry | overflow);
        }
    }
    (res, carry)
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
        match op {
            3 | 10 => {
                let a = vm.stack_pop();
                let res =
                match op {
                    3 => ( !a, false),
                    #[cfg(feature = "int_math_sqr")]
                    10 => { match isqrt(a) { Some(r) => (r, false), None => { vm.error = RuntimeError::IllegalInstructionValue; (0, false) } } }
                    _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; (0, false) }
                };               
                vm.processor.zero = res.0 == 0;
                vm.processor.carry = res.1;
                vm.stack_push(res.0);                  
            }
            0 .. 3 | 4 .. 10 => {
                let a = vm.stack_pop();
                let b = vm.stack_pop();
                let res;                
                match op {
                    0  => { res = (a & b, false); }
                    1  => { res = (a | b, false); }
                    2  => { res = (a ^ b, false); }
                    4  => { res = (a << (b as u32), false); }
                    5  => { res = (a >> (b as u32), false); }
                    6  => { res = a.overflowing_mul(b); }                    
                    7  => { res = if b != 0 { a.overflowing_div(b) } else { (0, false) }; if b == 0 { vm.error = RuntimeError::InterruptError; } }
                    8  => { res = if b != 0 { (a % b, false) } else { (0, false) }; if b == 0 { vm.error = RuntimeError::InterruptError; } }
                    #[cfg(feature = "int_math_pow")]
                    9  => { res = pow(a, b); }                                          
                    _ => { res = (0, false); vm.error = RuntimeError::UnimplementedInterruptFunc; }
                }
                vm.processor.zero = res.0 == 0;
                vm.processor.carry = res.1;
                vm.stack_push(res.0);  
                
            }            
            #[cfg(feature = "int_math_random")]
            11 => { math_random::call(vm); }
            #[cfg(feature = "int_math_trig")]
            20..=25 => { math_trig::call(vm, op); }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VMAtom;

    /// Calls a math function with the arguments pushed so the first is popped first
    fn call(op: VMAtom, args: &[VMAtom]) -> (VMAtom, RuntimeError, usize) {
        let mut vm = VirtMach::new();
        let start = vm.processor.stack_ptr;
        for arg in args.iter().rev() { vm.stack_push(*arg); }
        vm.stack_push(op);
        Interrupt {}.call(&mut vm);
        let depth = (start - vm.processor.stack_ptr) as usize;
        (vm.stack_pop(), vm.error.clone(), depth)
    }

    #[test]
    fn pow_and_sqr() {
        #[cfg(feature = "int_math_pow")]
        assert_eq!(call(9, &[3, 4]), (81, RuntimeError::NoError, 1));
        #[cfg(not(feature = "int_math_pow"))]
        assert_eq!(call(9, &[3, 4]), (0, RuntimeError::UnimplementedInterruptFunc, 1));
        #[cfg(feature = "int_math_sqr")]
        assert_eq!(call(10, &[49]), (7, RuntimeError::NoError, 1));
        #[cfg(not(feature = "int_math_sqr"))]
        assert_eq!(call(10, &[49]), (0, RuntimeError::UnimplementedInterruptFunc, 1));
    }

    #[cfg(feature = "int_math_sqr")]
    #[test]
    fn isqrt_matches_core() {
        for num in (VMAtom::MIN..=VMAtom::MAX).step_by(if VMAtom::BITS > 16 { 65521 } else { 1 }).chain([VMAtom::MAX]) {
            assert_eq!(isqrt(num), num.checked_isqrt(), "{}", num);
        }
    }

    #[cfg(feature = "int_math_pow")]
    #[test]
    fn pow_matches_core() {
        for base in (-12..=12).chain([VMAtom::MIN, VMAtom::MAX]) {
            for exp in 0..40 {
                let (res, carry) = pow(base, exp);
                assert_eq!((res, carry), base.overflowing_pow(exp as u32), "{}^{}", base, exp);
            }
        }
    }
}

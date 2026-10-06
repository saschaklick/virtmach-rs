use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "math";

pub const INDEX: u8 = 0;

pub const FUNCTIONS: [SoftInterruptFunction;11] = [
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
    SoftInterruptFunction { no: 10, name: "sqr", arguments: 1, returns: 1, help: "Squareroot (num)->(res)" }
];

/// Integer square root by shifting and subtracting, None for negative numbers. Unlike
/// checked_isqrt it needs no lookup table, which would take 512 bytes of RAM on AVR.
#[cfg(not(feature = "int_math_nosqr"))]
fn isqrt(num: crate::VMAtom) -> Option<crate::VMAtom> {
    if num < 0 {
        return None;
    }
    let (mut rem, mut res) = (num as u32, 0u32);
    let mut bit = 1u32 << 30;
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
    Some(res as crate::VMAtom)
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
                    #[cfg(not(feature = "int_math_nosqr"))]
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
                    #[cfg(not(feature = "int_math_nopow"))]
                    9  => { res = a.overflowing_pow(b as u32); }                                          
                    _ => { res = (0, false); vm.error = RuntimeError::UnimplementedInterruptFunc; }
                }
                vm.processor.zero = res.0 == 0;
                vm.processor.carry = res.1;
                vm.stack_push(res.0);  
                
            }            
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
        let depth = start - vm.processor.stack_ptr;
        (vm.stack_pop(), vm.error.clone(), depth)
    }

    #[test]
    fn pow_and_sqr() {
        #[cfg(not(feature = "int_math_nopow"))]
        assert_eq!(call(9, &[3, 4]), (81, RuntimeError::NoError, 1));
        #[cfg(feature = "int_math_nopow")]
        assert_eq!(call(9, &[3, 4]), (0, RuntimeError::UnimplementedInterruptFunc, 1));
        #[cfg(not(feature = "int_math_nosqr"))]
        assert_eq!(call(10, &[49]), (7, RuntimeError::NoError, 1));
        #[cfg(feature = "int_math_nosqr")]
        assert_eq!(call(10, &[49]), (0, RuntimeError::UnimplementedInterruptFunc, 1));
    }

    #[cfg(not(feature = "int_math_nosqr"))]
    #[test]
    fn isqrt_matches_core() {
        for num in (VMAtom::MIN..=VMAtom::MAX).step_by(if VMAtom::BITS > 16 { 65521 } else { 1 }).chain([VMAtom::MAX]) {
            assert_eq!(isqrt(num), num.checked_isqrt(), "{}", num);
        }
    }
}

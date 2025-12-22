use crate::{RuntimeError, VirtMach, interrupts::{ SoftInterrupt }};

#[allow(dead_code)]
pub const MAP: (&str, &str) = (
"math",
"0,  and, 2, 1,
 1,  or,  2, 1,
 2,  xor, 2, 1,
 3,  not, 1, 1,
 4,  lsh, 2, 1,
 5,  rsh, 2, 1,
 6,  mul, 2, 1,
 7,  div, 2, 1,
 8,  mod, 2, 1,
 9,  pow, 2, 1,
 10, sqr, 1, 1,
");

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "math";
    }

    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();        
        match op {
            3 | 9 => {
                let a = vm.stack_pop();
                let res;                
                match op {
                    3 => { res = ( !a, false); }
                    9 => { res = (0, false); vm.error = RuntimeError::UnimplementedInterruptFunc; }                    
                    _ => { res = (0, false); vm.error = RuntimeError::UnimplementedInterruptFunc; }
                }
                vm.processor.zero = res.0 == 0;
                vm.processor.carry = res.1;
                vm.stack_push(res.0);                  
            }
            0 .. 3 | 4 .. 9 => {
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

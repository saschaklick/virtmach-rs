use crate::{RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptDef, SoftInterruptFunction }};

#[allow(dead_code)]
pub static DEF: SoftInterruptDef = SoftInterruptDef {
    name: "math",
    functions: &[
        SoftInterruptFunction { no:  0, name: "and", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  1, name: "or",  arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  2, name: "xor", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  3, name: "not", arguments: 1, returns: 1 },
        SoftInterruptFunction { no:  4, name: "lsh", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  5, name: "rsh", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  6, name: "mul", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  7, name: "div", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  8, name: "mod", arguments: 2, returns: 1 },
        SoftInterruptFunction { no:  9, name: "pow", arguments: 2, returns: 1 },
        SoftInterruptFunction { no: 10, name: "sqr", arguments: 1, returns: 1 }
    ]
};

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

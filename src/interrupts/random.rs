use crate::{VirtMach, VMAtom, RuntimeError, interrupts::{SoftInterrupt, SoftInterruptFunction, SoftInterruptDef}};

#[allow(dead_code)]
pub static DEF: SoftInterruptDef = SoftInterruptDef { 
    name: "random",   
    functions: &[
        SoftInterruptFunction { no:  0, name: "rand", arguments: 2, returns: 1 }
    ]
};

use nostd_structs::algos::rand;

static mut SEED: u64 = 0;

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "math";
    }
    
    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();        
        let a = vm.stack_pop();
        let b = vm.stack_pop();
        let res;                
        match op {            
            0 => {
                    let len = a.saturating_sub(b).abs() + 1;
                    let min = if a <= b { a } else { b };                                                      
                    let value = rand::lcg::LcgRng::new(unsafe { SEED }.wrapping_add((vm.cycle_cnt as u64).wrapping_add(vm.program.id.as_ptr() as u64))).next();
                    unsafe { SEED = value; }
                    res = ( min + ((value % len as u64)) as VMAtom, false);                        
                }
            _ => { res = (0, false); vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
        vm.processor.zero = res.0 == 0;
        vm.processor.carry = res.1;
        vm.stack_push(res.0);                                      
    }

}

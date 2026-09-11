use crate::{VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "random";

pub const FUNCTIONS: [SoftInterruptFunction;1] = [
    SoftInterruptFunction { no:  0, name: "range", arguments: 2, returns: 1, help: "Generate random value in provided value range (start,end)->(res)" }
];

use nostd_structs::algos::rand;

static mut SEED: u64 = 0;

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS
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

use crate::{VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt }};

#[allow(dead_code)]
pub const MAP: (&str, &str) = (
"proc",
"0, version,   0, 3,
 1, atom_size, 0, 1,
 2, mem_size,  0, 1,
 3, stack_ptr, 0, 1,
 4, prog_cnt,  0, 1,
 5, cycle_cnt, 0, 1,
");

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "proc";
    }

    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();        
        match op {
            0 => { vm.stack_push(0); vm.stack_push(1); vm.stack_push(0); }            
            1 => { vm.stack_push(core::mem::size_of::<VMAtom>() as VMAtom); }
            2 => { vm.stack_push(crate::MEM_SIZE as VMAtom); }
            3 => { vm.stack_push(vm.processor.stack_ptr as VMAtom); }
            4 => { vm.stack_push(vm.processor.prog_cnt as VMAtom); }
            5 => { vm.stack_push(vm.cycle_cnt as VMAtom); }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }

}

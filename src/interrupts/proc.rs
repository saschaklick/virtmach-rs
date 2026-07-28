use crate::{VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "proc";
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &[
            SoftInterruptFunction { no:  0, name: "version",   arguments: 0, returns: 3, help: "Nodem version" },
            SoftInterruptFunction { no:  1, name: "atom_size", arguments: 0, returns: 1, help: "Atom-size in bytes" },
            SoftInterruptFunction { no:  2, name: "mem_size",  arguments: 0, returns: 1, help: "Memory-size" },
            SoftInterruptFunction { no:  3, name: "stack_ptr", arguments: 0, returns: 1, help: "Current address of stack pointer" },
            SoftInterruptFunction { no:  4, name: "prog_cnt",  arguments: 0, returns: 1, help: "Current program pointer" },
            SoftInterruptFunction { no:  5, name: "stack_ptr", arguments: 0, returns: 1, help: "Current total cycle count" }          
        ];
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

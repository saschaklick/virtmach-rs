use crate::{Storage,  VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction }};
#[cfg(feature = "alloc")]
use crate::{ VMStringIndex };

pub const NAME: &str = "proc";

pub const INDEX: u8 = 1;

pub const FUNCTIONS: [SoftInterruptFunction;8] = [
    SoftInterruptFunction { no:  0, name: "version",     arguments: 0, returns: 3, help: "Nodem version ()->(major,minor,revision)" },
    SoftInterruptFunction { no:  1, name: "atom_size",   arguments: 0, returns: 1, help: "Atom-size in bytes ()->(size)" },
    SoftInterruptFunction { no:  2, name: "mem_size",    arguments: 0, returns: 1, help: "Memory-size ()->(size)" },
    SoftInterruptFunction { no:  3, name: "stack_ptr",   arguments: 0, returns: 1, help: "Current address of stack pointer ()->(sp)" },
    SoftInterruptFunction { no:  4, name: "prog_cnt",    arguments: 0, returns: 1, help: "Current program pointer ()->(pc)" },
    SoftInterruptFunction { no:  5, name: "stack_ptr",   arguments: 0, returns: 1, help: "Current total cycle count ()->(cc)" },
    SoftInterruptFunction { no: 10, name: "run_via_no",  arguments: 2, returns: 0, help: "Make a request for a new program to be loaded and run by number; when call is set, return to the caller from the callee with results on the stack (prg_no, call)->()" },
    SoftInterruptFunction { no: 11, name: "run_via_str", arguments: 2, returns: 0, help: "Make a request for a new program to be loaded and run by name (string_idx, call)->()" },    
];

pub struct Interrupt {}

impl <S: Storage> SoftInterrupt<S> for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();        
        match op {
            0  => { vm.stack_push(0); vm.stack_push(1); vm.stack_push(0); }            
            1  => { vm.stack_push(core::mem::size_of::<VMAtom>() as VMAtom); }
            2  => { vm.stack_push(crate::MEM_SIZE as VMAtom); }
            3  => { vm.stack_push(vm.processor.stack_ptr as VMAtom); }
            4  => { vm.stack_push(vm.processor.prog_cnt as VMAtom); }
            5  => { vm.stack_push(vm.cycle_cnt as VMAtom); }
            #[cfg(not(feature = "alloc"))]
            10 => { vm.load = (Some(vm.stack_pop()), vm.stack_pop() != 0); }
            #[cfg(not(feature = "alloc"))]
            11 => { vm.error = RuntimeError::AllocFeatureRequired; }
            #[cfg(feature = "alloc")]
            10 => { vm.load = (Some(vm.stack_pop()), None, vm.stack_pop() != 0); }
            #[cfg(feature = "alloc")]
            11 => { vm.load = (None, Some(vm.stack_pop() as VMStringIndex), vm.stack_pop() != 0); }
            _  => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }

}

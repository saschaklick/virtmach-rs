use crate::{VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

#[cfg(feature = "alloc")]
extern crate alloc;

pub const NAME: &str = "string";

pub const INDEX: u8 = 2;

pub const FUNCTIONS: [SoftInterruptFunction;5] = [
    SoftInterruptFunction { no:  0, name: "get_length", arguments: 1, returns: 1, help: "Get the length of a string (index)->(length)" },
    SoftInterruptFunction { no:  1, name: "substr", arguments: 4, returns: 0, help: "Truncate a string (dest_index,src_index,start,end)->()" },
    SoftInterruptFunction { no:  2, name: "format", arguments: 2, returns: 0, help: "Converts an atom to a string (dest_index,number)->()" },
    SoftInterruptFunction { no:  3, name: "concat", arguments: 3, returns: 0, help: "Concats two strings into one (dest_index,first_index,second_index)->()" },
    SoftInterruptFunction { no:  4, name: "compare", arguments: 2, returns: 1, help: "Compares two strings byte by byte (first_index,second_index)->(-1 less, 0 equal, 1 greater)" }
];

fn format_atom(num: VMAtom, buf: &mut [u8; 24]) -> &[u8] {
    let mut value = (num as i64).unsigned_abs();
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 { break; }
    }
    if num < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    &buf[i..]
}

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
        let index = vm.stack_pop() as u8;                        
        match op {            
            0 => {
                let length = vm.get_bin(index).len() as VMAtom;
                vm.stack_push(length);
            }
            1 => {      
                #[cfg(feature = "alloc")]
                use core::cmp;
                let src_index = vm.stack_pop() as u8;                
                let start = vm.stack_pop() as usize;
                let end = vm.stack_pop() as usize;
                let string = vm.get_str(src_index);
                // copied, set_bin cannot take a borrow of the vm
                #[cfg(feature = "alloc")]
                let part = string.get(cmp::max(0, start)..cmp::min(string.len(), end)).unwrap_or("").as_bytes().to_vec();
                #[cfg(not(feature = "alloc"))]
                let part: [u8; 0] = { let _ = (string, start, end); [] };
                vm.set_bin(index, &part);
            }
            2 => {
                let num = vm.stack_pop();
                let mut buf = [0u8; 24];
                vm.set_bin(index, format_atom(num, &mut buf));
            }
            3 => {
                let first_index = vm.stack_pop() as u8;
                let second_index = vm.stack_pop() as u8;
                #[cfg(feature = "alloc")]
                let joined = {
                    let mut joined = vm.get_bin(first_index).to_vec();
                    joined.extend_from_slice(vm.get_bin(second_index));
                    joined
                };
                #[cfg(not(feature = "alloc"))]
                let joined: [u8; 0] = { vm.get_bin(first_index); vm.get_bin(second_index); [] };
                vm.set_bin(index, &joined);
            }
            4 => {
                let second_index = vm.stack_pop() as u8;
                if !vm.has_bin(index) || !vm.has_bin(second_index) { vm.error = RuntimeError::DictionaryOutOfBound; }
                let order = vm.program.get_bin(index).cmp(vm.program.get_bin(second_index)) as i8;
                vm.stack_push(order as VMAtom);
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }                
    }

}

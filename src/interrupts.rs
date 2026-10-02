use crate::{ VirtMach, VMAtom, Storage, Ram };

pub mod dummy;
pub mod proc;
pub mod math;
pub mod string;
#[cfg(feature = "random")]
pub mod random;   
pub mod surface;
pub mod trig;
pub mod gpio;
pub mod uart;
pub mod i2c;
pub mod time;
pub mod pwm;

/// Built-in interrupts have fixed numbers, their INDEX, so programs compiled with any selection of
/// them run on every runtime that places them there. Unused numbers are filled with dummy.
pub const BUILTIN_SLOTS: u8 = 11;

/// Interrupt numbers are 4 bits in the INT instruction
pub const MAX_SLOTS: u8 = 16;

/// The INDEX of a built-in interrupt by its NAME
pub fn builtin_index(name: &str) -> Option<u8> {
    match name {
        n if n == math::NAME => Some(math::INDEX),
        n if n == proc::NAME => Some(proc::INDEX),
        n if n == string::NAME => Some(string::INDEX),
        #[cfg(feature = "random")]
        n if n == random::NAME => Some(random::INDEX),
        n if n == time::NAME => Some(time::INDEX),
        n if n == trig::NAME => Some(trig::INDEX),
        n if n == surface::NAME => Some(surface::INDEX),
        n if n == gpio::NAME => Some(gpio::INDEX),
        n if n == uart::NAME => Some(uart::INDEX),
        n if n == i2c::NAME => Some(i2c::INDEX),
        n if n == pwm::NAME => Some(pwm::INDEX),
        _ => None
    }
}

#[cfg(feature = "compile")]
extern crate std;
#[cfg(feature = "compile")]
use std::{ format, string::String, vec::Vec };

/// The number of each interrupt: built-in ones use their INDEX, others the numbers after the
/// built-in ones in the given order. Fails if a number is used twice or is too high.
#[cfg(feature = "compile")]
pub fn interrupt_numbers<N: AsRef<str>>(names: &[N]) -> Result<Vec<u8>, String> {
    let mut custom = BUILTIN_SLOTS;
    let mut numbers: Vec<u8> = Vec::new();
    for name in names.iter().map(AsRef::as_ref) {
        let int_no = match builtin_index(name) {
            Some(index) => index,
            None => { custom += 1; custom - 1 }
        };
        if int_no >= MAX_SLOTS {
            return Err(format!("too many interrupts, {} would be number {} but the last is {}", name, int_no, MAX_SLOTS - 1));
        }
        if let Some(other) = numbers.iter().position(|n| *n == int_no) {
            return Err(format!("{} and {} are both interrupt number {}", names[other].as_ref(), name, int_no));
        }
        numbers.push(int_no);
    }
    Ok(numbers)
}

#[derive(Clone, Copy)]
pub struct SoftInterruptFunction <'a> {
    pub no: VMAtom,
    pub name: &'a str,    
    pub arguments: usize,
    pub returns: usize,
    pub help: &'a str
}

#[cfg(feature = "compile")]
pub struct SoftInterruptFunctionOwned {
    pub no: VMAtom,
    pub name: String,    
    pub arguments: usize,
    pub returns: usize,
    pub help: String
}

/// An interrupt for vms with programs in the storage S, Ram unless given. Interrupts that do not
/// read bins with get_bin or get_str can implement it for every storage with
/// impl<S: Storage> SoftInterrupt<S> for ..., copy_bin reads bins in any storage.
pub trait SoftInterrupt<S: Storage = Ram> {        
    fn name(&self) -> &str;    

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>];
    
    fn call(&mut self, vm: &mut VirtMach<'_, S>);
}
#[cfg(all(test, feature = "compile"))]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(interrupt_numbers(&["proc", "math", "surface"]), Ok(std::vec![1, 0, 6]));
        assert_eq!(interrupt_numbers(&["math", "proc", "surface"]), Ok(std::vec![0, 1, 6]));
        // not built in: after the built-in numbers in the given order
        assert_eq!(interrupt_numbers(&["joystick", "math", "sound"]), Ok(std::vec![11, 0, 12]));
        assert!(interrupt_numbers(&["math", "math"]).is_err());
        assert!(interrupt_numbers(&["a", "b", "c", "d", "e", "f"]).is_err());
        assert_eq!(builtin_index(dummy::NAME), None);
    }
}

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

#[cfg(feature = "compile")]
extern crate std;
#[cfg(feature = "compile")]
use std::string::String;

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
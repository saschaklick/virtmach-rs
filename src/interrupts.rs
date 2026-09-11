use crate::{ VirtMach, VMAtom };

pub mod dummy;
pub mod proc;
pub mod math;
#[cfg(feature = "random")]
pub mod random;   
pub mod surface;

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

pub trait SoftInterrupt {        
    fn name(&self) -> &str;    

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>];
    
    fn call(&mut self, vm: &mut VirtMach);
}
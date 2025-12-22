use cfg_block::cfg_block;
use crate::{ VirtMach, VMAtom};

mod dummy;
pub use dummy::Interrupt as Dummy;

mod proc;
pub use proc::Interrupt as Proc;

mod math;
pub use math::Interrupt as Math;

mod surface;
pub use surface::MAP as SurfaceMap;

cfg_block! {
    #[cfg(feature = "random")] {
        mod random;
        pub use random::Interrupt as Random;        
    }
}

#[derive(Clone, Copy)]
pub struct SoftInterruptDef <'a> {
    pub name: &'a str,
    pub functions: &'a [SoftInterruptFunction <'a>]
}

#[derive(Clone, Copy)]
pub struct SoftInterruptFunction <'a> {
    pub no: VMAtom,
    pub name: &'a str,    
    pub arguments: usize,
    pub returns: usize,
    pub help: &'a str
}

pub const BASE_INTERRUPT_MAPS: &[(&str, &str)] = &[
    proc::MAP,
    math::MAP,
    #[cfg(feature = "random")]
    random::MAP
];

pub trait SoftInterrupt {    
    fn name(&self) -> &str;    
    
    fn call(&mut self, vm: &mut VirtMach);
}
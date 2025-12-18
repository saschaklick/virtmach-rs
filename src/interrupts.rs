use cfg_block::cfg_block;
use crate::{ VirtMach, VMAtom};

mod dummy;
pub use dummy::Interrupt as Dummy;
pub use dummy::DEF as DummyDef;

mod proc;
pub use proc::Interrupt as Proc;
pub use proc::DEF as ProcDef;

mod math;
pub use math::Interrupt as Math;
pub use math::DEF as MathDef;

cfg_block! {
    #[cfg(feature = "random")] {
        mod random;
        pub use random::Interrupt as Random;
        pub use random::DEF as RandomDef;
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
    pub returns: usize
}

pub const BASE_INTERRUPTS_DEFS: &[&SoftInterruptDef] = &[
    &proc::DEF,
    &math::DEF,
    #[cfg(feature = "random")]
    &random::DEF
];

pub trait SoftInterrupt {    
    fn name(&self) -> &str;    
    
    fn call(&mut self, vm: &mut VirtMach);
}
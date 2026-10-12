#![no_std]
#![cfg_attr(target_arch = "avr", feature(asm_experimental_arch))]

use cfg_block::cfg_block;

const MEM_SIZE:usize = 23;
const REG_MAX:usize  = 15;

/// Picks the type A if C is true and B otherwise
#[doc(hidden)]
pub trait Pick { type T; }
#[doc(hidden)]
pub struct Sel<const C: bool, A, B>(core::marker::PhantomData<(A, B)>);
impl<A, B> Pick for Sel<true, A, B> { type T = A; }
impl<A, B> Pick for Sel<false, A, B> { type T = B; }

/// The smallest unsigned integer that holds every address of the memory, so the stack pointer is
/// 8 bit on an 8 bit cpu as long as the memory has up to 256 atoms
pub type StackPtr = <Sel<{ MEM_SIZE as u64 <= 0x100 }, u8, <Sel<{ MEM_SIZE as u64 <= 0x10000 }, u16, u32> as Pick>::T> as Pick>::T;

mod atom;
mod opcodes;
mod processor;
mod virtmach;
mod errors;
mod program;
mod string;
mod writer;
mod decompile;
mod reporting;
pub mod interrupts;

pub use atom::*;
pub use virtmach::*;
pub use processor::*;
pub use string::VMStringIndex;

cfg_block!{
    #[cfg(feature="compile")] {
        mod compile;

        pub use compile::*;
    }
}

#[cfg(feature="basic")]
extern crate std;

#[cfg(feature="basic")]
pub mod basic;
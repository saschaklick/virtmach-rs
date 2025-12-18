#![no_main]
#![no_std]

use cfg_block::cfg_block;

const MEM_SIZE:usize = 23;
const REG_MAX:usize  = 15;

mod atom;
mod opcodes;
mod processor;
mod virtmach;
mod errors;
mod program;
mod writer;
mod decompile;
mod reporting;
pub mod interrupts;

pub use atom::*;
pub use virtmach::*;
pub use processor::*;

cfg_block!{
    #[cfg(feature="std")] {        
        mod compile;  
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 { 0 }
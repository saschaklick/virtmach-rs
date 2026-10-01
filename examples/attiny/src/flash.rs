//! Programs in flash, read byte by byte with LPM, so they do not take any RAM.
//!
//! The AVR has separate address spaces for flash and RAM and normal loads only reach RAM, that
//! is why constants are copied into RAM at startup. Statics in the .progmem.data section stay in
//! flash, their address is a flash address and must only be read with LPM, never through a
//! reference.

use core::arch::asm;
use virtmach::{ ProgramData, Storage };

/// Bytes in flash
#[derive(Clone, Copy)]
pub struct FlashBytes {
    addr: u16,
    len: u16
}

impl FlashBytes {
    /// The bytes of a static in .progmem.data
    pub fn of<const N: usize>(data: &'static [u8; N]) -> Self {
        FlashBytes { addr: data.as_ptr() as u16, len: N as u16 }
    }
}

impl ProgramData for FlashBytes {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len as usize
    }

    #[inline(always)]
    fn byte(&self, index: usize) -> u8 {
        if index >= self.len as usize {
            return 0;
        }
        let byte: u8;
        unsafe { asm!("lpm {0}, Z", out(reg) byte, in("Z") self.addr.wrapping_add(index as u16), options(pure, readonly, nostack, preserves_flags)) };
        byte
    }
}

/// Storage of programs in flash
pub struct Flash;

impl Storage for Flash {
    type Data<'a> = FlashBytes;

    fn empty<'a>() -> Self::Data<'a> {
        FlashBytes { addr: 0, len: 0 }
    }
}

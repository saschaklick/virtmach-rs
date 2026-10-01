use crate::{ATOM_ID};

/// Read access to the bytes of a program, wherever they are stored
pub trait ProgramData: Copy {
    fn len(&self) -> usize;

    /// The byte at index, 0 past the end
    fn byte(&self, index: usize) -> u8;
}

impl ProgramData for &[u8] {
    #[inline(always)]
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    #[inline(always)]
    fn byte(&self, index: usize) -> u8 {
        self.get(index).copied().unwrap_or(0)
    }
}

/// Where programs are stored. Ram is the default and keeps them in byte slices that normal
/// pointers can read, which is all memory on most platforms. Other storages, like the flash of
/// AVR that is only readable with LPM, are implemented by the firmware.
pub trait Storage {
    type Data<'a>: ProgramData;

    /// Data without bins and instructions, for the empty and error programs
    fn empty<'a>() -> Self::Data<'a>;
}

/// Programs in byte slices
pub struct Ram;

impl Storage for Ram {
    type Data<'a> = &'a [u8];

    fn empty<'a>() -> &'a [u8] {
        &[ATOM_ID, 0]
    }
}

pub struct Program <'a, S: Storage = Ram> {
    pub source: u8,
    pub id: &'a str,
    pub data: S::Data<'a>,
}

impl Program <'_> {
    pub const EMPTY: Program <'static> = Program { source: 255, id: "-empty-", data: &[ATOM_ID, 0] };
    pub const ERROR: Program <'static> = Program { source: 255, id: "-error-", data: &[ATOM_ID, 0] };
}

impl <S: Storage> Program <'_, S> {
    /// The empty program of any storage, see Program::EMPTY
    pub fn empty<'b>() -> Program<'b, S> {
        Program { source: 255, id: "-empty-", data: S::empty() }
    }

    /// The error program of any storage, see Program::ERROR
    pub fn error<'b>() -> Program<'b, S> {
        Program { source: 255, id: "-error-", data: S::empty() }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;
    use crate::{ VirtMach, VMAtom, Runtime, interrupts::SoftInterrupt };
    use super::*;

    /// Bins "hi" and "abc", then reg r0, set #100, reg r1, set #-7, add #1, end, atoms in the size of
    /// VMAtom. The add needs r1 to still be the active register after the immediate of the set.
    fn program() -> Vec<u8> {
        let mut data = std::vec![ATOM_ID, 2, 2, 0, 5, 0, b'h', b'i', b'a', b'b', b'c', 0x00, 0xf1];
        data.extend_from_slice(&(100 as VMAtom).to_le_bytes());
        data.extend_from_slice(&[0x10, 0xf1]);
        data.extend_from_slice(&(-7 as VMAtom).to_le_bytes());
        data.push(0xf6);
        data.extend_from_slice(&(1 as VMAtom).to_le_bytes());
        data.push(0xff);
        data
    }

    /// A storage that is not a slice, like flash on AVR
    struct Wrapped;

    #[derive(Clone, Copy)]
    struct WrappedBytes<'a>(&'a [u8]);

    impl ProgramData for WrappedBytes<'_> {
        fn len(&self) -> usize { self.0.len() }
        fn byte(&self, index: usize) -> u8 { self.0.get(index).copied().unwrap_or(0) }
    }

    impl Storage for Wrapped {
        type Data<'a> = WrappedBytes<'a>;
        fn empty<'a>() -> Self::Data<'a> { WrappedBytes(&[]) }
    }

    #[test]
    fn storages_run_the_same() {
        let program = program();
        let mut ram = VirtMach::new();
        ram.load_program(Program { source: 0, id: "ram", data: &program });
        // limited, so a vm that does not stop fails the test instead of hanging it
        ram.run(100, &mut []);

        let mut wrapped = VirtMach::<Wrapped>::with_storage();
        wrapped.load_program(Program { source: 0, id: "wrapped", data: WrappedBytes(&program) });
        let interrupts: &mut [&mut dyn SoftInterrupt<Wrapped>] = &mut [&mut crate::interrupts::math::Interrupt {}];
        wrapped.run(100, interrupts);

        for vm_registers in [ram.registers, wrapped.registers] {
            assert_eq!(vm_registers[..2], [100, -6]);
        }
        assert!(ram.state == Runtime::Stp && wrapped.state == Runtime::Stp);
        assert!(ram.error == crate::RuntimeError::NoError && wrapped.error == crate::RuntimeError::NoError);

        assert_eq!(ram.get_str(0), "hi");
        assert_eq!(ram.get_str(1), "abc");
        let mut buf = [0u8; 2];
        assert_eq!((wrapped.copy_bin(1, &mut buf), buf), (3, *b"ab"));
        assert_eq!((wrapped.bin_len(0), wrapped.has_bin(2)), (2, false));
    }

    #[test]
    fn running_past_the_end_stops() {
        // reg r0 without an end
        let mut vm = VirtMach::new();
        vm.load_program(Program { source: 0, id: "open", data: &[ATOM_ID, 0, 0x00] });
        vm.run(0, &mut []);
        assert!(vm.state == Runtime::Err && vm.error == crate::RuntimeError::ProgramOutOfBounds);
    }

    #[test]
    fn empty_and_wrong_programs() {
        let mut vm = VirtMach::<Wrapped>::with_storage();
        vm.load_program(Program { source: 0, id: "empty", data: WrappedBytes(&[]) });
        vm.run(0, &mut []);
        // like Program::EMPTY, an empty program is not started
        assert!(vm.state == Runtime::Ini && vm.error == crate::RuntimeError::NoError);
        vm.load_program(Program { source: 0, id: "wrong", data: WrappedBytes(&[ATOM_ID + 1, 0]) });
        assert_eq!(vm.error, crate::RuntimeError::MismatchedAtomType);
    }
}

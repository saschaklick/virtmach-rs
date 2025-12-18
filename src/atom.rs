use cfg_block::cfg_block;

cfg_block! {

    #[cfg(feature = "i8")] {
        pub const ATOM_ID: u8 = 1;
        
        pub use i8 as VMAtom;
        pub use i16 as VMAddr;        

        pub trait VAtom { fn get_atom(&mut self) -> VMAtom; }

        pub trait VAtomMut { fn put_atom(&mut self, a:VMAtom); }

        cfg_block! {
            if #[cfg(feature="std")] {
                use bytes::{Buf, BufMut};
                
                impl<T> VAtom for T where T: Buf { fn get_atom(&mut self) -> VMAtom { return self.get_i8(); } }
                
                impl<T> VAtomMut for T where T: BufMut { fn put_atom(&mut self, a:VMAtom) { self.put_i8(a); } }        
            } else {
                impl VAtom for &[u8] { fn get_atom(&mut self) -> VMAtom { return if self.len() < size_of::<VMAtom>() { VMAtom::MIN } else { let v = (self[0] as u8); if v >= 0x80 { -((0xff - v) as VMAtom) - 1 } else { v as VMAtom } } } }

                impl VAtomMut for &[u8] { fn put_atom(&mut self, a:VMAtom) { unimplemented!("writing atoms not implement for no_std"); } }
            }
        }
    }

    #[cfg(any(feature = "i16", all(not(feature = "i8"), not(feature = "i32"))))] {
        pub const ATOM_ID: u8 = 2;

        pub use i16 as VMAtom;
        pub use i16 as VMAddr;

        pub trait VAtom { fn get_atom(&mut self) -> VMAtom; }

        pub trait VAtomMut { fn put_atom(&mut self, a:VMAtom); }

        cfg_block! {
            if #[cfg(feature="std")] {
                use bytes::{Buf, BufMut};
                
                impl<T> VAtom for T where T: Buf { fn get_atom(&mut self) -> VMAtom { return self.get_i16_ne(); } }
                
                impl<T> VAtomMut for T where T: BufMut { fn put_atom(&mut self, a:VMAtom) { self.put_i16_ne(a); } }        
            } else {
                impl VAtom for &[u8] { fn get_atom(&mut self) -> VMAtom { return if self.len() < size_of::<VMAtom>() { VMAtom::MIN } else { let v = (self[0] as u16) + ((self[1] as u16) << 8); if v >= 0x8000 { -((0xffff - v) as VMAtom) - 1 } else { v as VMAtom } } } }

                impl VAtomMut for &[u8] { fn put_atom(&mut self, a:VMAtom) { unimplemented!("writing atoms not implement for no_std"); } }
            }
        }
    }

    #[cfg(feature = "i32")] {
        pub const ATOM_ID: u8 = 4;
        
        pub use i32 as VMAtom;
        pub use i32 as VMAddr;

        pub trait VAtom { fn get_atom(&mut self) -> VMAtom; }

        pub trait VAtomMut { fn put_atom(&mut self, a:VMAtom); }

        cfg_block! {
            if #[cfg(feature="std")] {
                use bytes::{Buf, BufMut};
                
                impl<T> VAtom for T where T: Buf { fn get_atom(&mut self) -> VMAtom { return self.get_i32_ne(); } }
                
                impl<T> VAtomMut for T where T: BufMut { fn put_atom(&mut self, a:VMAtom) { self.put_i32_ne(a); } }        
            } else {
                impl VAtom for &[u8] { fn get_atom(&mut self) -> VMAtom { return if self.len() < size_of::<VMAtom>() { VMAtom::MIN } else { let v = (self[0] as u32) + ((self[1] as u32) << 8) + ((self[1] as u32) << 16) + ((self[1] as u32) << 24); if v >= 0x80000000 { -((0xffffffff - v) as VMAtom) - 1 } else { v as VMAtom } } } }

                impl VAtomMut for &[u8] { fn put_atom(&mut self, a:VMAtom) { unimplemented!("writing atoms not implement for no_std"); } }
            }
        }
    }
}



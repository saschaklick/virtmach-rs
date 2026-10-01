use crate::{Program, VirtMach, RuntimeError, ProgramData, Storage};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Dynamic bins, the entries after the program's fixed bins. Every entry owns its bytes, which are
/// freed when the entry is set again, so slices from get_bin are only valid until then.
#[cfg(feature = "alloc")]
static mut DICT: Vec<Vec<u8>> = Vec::new();

pub use u8 as VMStringIndex;

impl <S: Storage> VirtMach <'_, S> {
    /// Whether a bin exists, without setting an error
    pub fn has_bin(&self, index: VMStringIndex) -> bool {
        self.program.has_bin(index)
    }

    /// The length of a bin, sets RuntimeError::DictionaryOutOfBound and returns 0 if it does not exist
    pub fn bin_len(&mut self, index: VMStringIndex) -> usize {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.bin_len(index)
    }

    /// Copies a bin into buf as far as it fits and returns its whole length, works for programs in
    /// any storage, sets RuntimeError::DictionaryOutOfBound and returns 0 if it does not exist
    pub fn copy_bin(&mut self, index: VMStringIndex, buf: &mut [u8]) -> usize {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.copy_bin(index, buf)
    }

    /// Sets a dynamic bin, sets RuntimeError::DictionaryOutOfBound if it cannot be stored: the index
    /// belongs to a fixed bin, the memory cannot be allocated or the alloc feature is off
    pub fn set_bin(&mut self, index: VMStringIndex, bin: &[u8]) {
        if !self.program.set_bin(index, bin) { self.error = RuntimeError::DictionaryOutOfBound; }
    }
}

impl VirtMach <'_> {
    /// Reads a bin, sets RuntimeError::DictionaryOutOfBound and returns an empty one if it does not exist
    pub fn get_bin(&mut self, index: VMStringIndex) -> &[u8] {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.get_bin(index)
    }

    /// Reads a bin as string, see get_bin
    pub fn get_str(&mut self, index: VMStringIndex) -> &str {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.get_str(index)
    }
}

/// The dynamic bins
#[cfg(feature = "alloc")]
fn dict() -> &'static Vec<Vec<u8>> {
    unsafe { &*(&raw const DICT) }
}

/// The header of a program: the atom id, the number of fixed bins, their 16 bit end addresses,
/// the bins and then the instructions
impl <S: Storage> Program <'_, S> {
    pub fn get_fixed_bin_count(&self) -> VMStringIndex {        
        self.data.byte(1)
    }

    fn get_bin_addr(&self, index: VMStringIndex) -> usize {
        if index == 0 || index > self.get_fixed_bin_count() {
            0
        }else{
            let pos = 2 + (index as usize - 1) * 2;
            self.data.byte(pos) as usize | ((self.data.byte(pos + 1) as usize) << 8)
        }
    }

    fn get_dict_start(&self) -> usize {
        2 + (self.get_fixed_bin_count() as usize * 2)
    }

    /// Where the instructions start in data
    pub fn get_code_start(&self) -> usize {
        self.get_dict_start() + self.get_bin_addr(self.get_fixed_bin_count())
    }

    /// The number of instruction bytes
    pub fn get_code_len(&self) -> usize {
        self.data.len().saturating_sub(self.get_code_start())
    }

    pub fn has_bin(&self, index: VMStringIndex) -> bool {
        if index < self.get_fixed_bin_count() { return true; }
        #[cfg(feature = "alloc")]
        {
            ((index - self.get_fixed_bin_count()) as usize) < dict().len()
        }
        #[cfg(not(feature = "alloc"))]
        false
    }

    pub fn bin_len(&self, index: VMStringIndex) -> usize {
        if index >= self.get_fixed_bin_count() {
            #[cfg(not(feature = "alloc"))]
            return 0;
            #[cfg(feature = "alloc")]
            return dict().get((index - self.get_fixed_bin_count()) as usize).map(Vec::len).unwrap_or(0);
        }
        self.get_bin_addr(index + 1) - self.get_bin_addr(index)
    }

    /// Copies a bin into buf as far as it fits and returns its whole length
    pub fn copy_bin(&self, index: VMStringIndex, buf: &mut [u8]) -> usize {
        if index >= self.get_fixed_bin_count() {
            #[cfg(not(feature = "alloc"))]
            return 0;
            #[cfg(feature = "alloc")]
            {
                let bin = dict().get((index - self.get_fixed_bin_count()) as usize).map(Vec::as_slice).unwrap_or(&[]);
                let n = bin.len().min(buf.len());
                buf[..n].copy_from_slice(&bin[..n]);
                return bin.len();
            }
        }
        let start = self.get_dict_start() + self.get_bin_addr(index);
        let len = self.bin_len(index);
        for (i, b) in buf.iter_mut().take(len).enumerate() { *b = self.data.byte(start + i); }
        len
    }

    /// Stores a dynamic bin, returns false if it cannot be stored
    pub fn set_bin(&self, _index: VMStringIndex, _bin: &[u8]) -> bool {
        #[cfg(feature = "alloc")]
        if _index >= self.get_fixed_bin_count() {
            let index = (_index - self.get_fixed_bin_count()) as usize;
            // copy first, _bin may point into the entry that is replaced
            let mut bin = Vec::new();
            if bin.try_reserve_exact(_bin.len()).is_err() { return false; }
            bin.extend_from_slice(_bin);
            let dict = unsafe { &mut *(&raw mut DICT) };
            if dict.len() <= index {
                if dict.try_reserve(index + 1 - dict.len()).is_err() { return false; }
                dict.resize_with(index + 1, Vec::new);
            }
            dict[index] = bin;
            return true;
        }
        false
    }    
}

/// Slices of programs in RAM
impl Program <'_>  {
    pub fn get_str(&self, index: VMStringIndex) -> &str {
        str::from_utf8(self.get_bin(index)).unwrap_or("")
    }

    pub fn get_bin(&self, index: VMStringIndex) -> &[u8] {        
        if index >= self.get_fixed_bin_count() {                        
            #[cfg(not(feature = "alloc"))]                        
            return &[];
            #[cfg(feature = "alloc")]
            return dict().get((index - self.get_fixed_bin_count()) as usize).map(Vec::as_slice).unwrap_or(&[]);
        }
        let dict_start = self.get_dict_start();
        &self.data[dict_start + self.get_bin_addr(index)..dict_start + self.get_bin_addr(index + 1)]
    }
    
    pub fn get_instructions(&self) -> &[u8] {   
        self.data.get(self.get_code_start()..).unwrap_or(&[])
    }
}

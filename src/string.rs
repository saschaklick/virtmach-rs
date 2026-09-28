use crate::{Program, VirtMach, RuntimeError};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Dynamic bins, the entries after the program's fixed bins. Every entry owns its bytes, which are
/// freed when the entry is set again, so slices from get_bin are only valid until then.
#[cfg(feature = "alloc")]
static mut DICT: Vec<Vec<u8>> = Vec::new();

impl VirtMach <'_> {
    /// Whether a bin exists, without setting an error
    pub fn has_bin(&self, index: u8) -> bool {
        self.program.has_bin(index)
    }

    /// Reads a bin, sets RuntimeError::DictionaryOutOfBound and returns an empty one if it does not exist
    pub fn get_bin(&mut self, index: u8) -> &[u8] {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.get_bin(index)
    }

    /// Reads a bin as string, see get_bin
    pub fn get_str(&mut self, index: u8) -> &str {
        if !self.program.has_bin(index) { self.error = RuntimeError::DictionaryOutOfBound; }
        self.program.get_str(index)
    }

    /// Sets a dynamic bin, sets RuntimeError::DictionaryOutOfBound if it cannot be stored: the index
    /// belongs to a fixed bin, the memory cannot be allocated or the alloc feature is off
    pub fn set_bin(&mut self, index: u8, bin: &[u8]) {
        if !self.program.set_bin(index, bin) { self.error = RuntimeError::DictionaryOutOfBound; }
    }
}

impl Program <'_>  {
    pub fn get_fixed_bin_count(&self) -> u8 {        
        self.data[1]
    }

    fn get_bin_addr(&self, index: u8) -> usize {
        if index == 0 || index > self.get_fixed_bin_count() {
            0
        }else{
            let buf = &self.data[2 + (index as usize - 1) * 2..];
            buf[0] as usize | (buf[1] as usize).strict_shl(8)
        }
    }
    
    pub fn get_str(&self, index: u8) -> &str {
        str::from_utf8(self.get_bin(index)).unwrap_or("")
    }

    pub fn get_bin(&self, index: u8) -> &[u8] {        
        
        if index >= self.get_fixed_bin_count() {                        
            #[cfg(not(feature = "alloc"))]                        
            return &[];
            #[cfg(feature = "alloc")]
            {
                let dict = unsafe { &*(&raw const DICT) };
                dict.get((index - self.get_fixed_bin_count()) as usize).map(Vec::as_slice).unwrap_or(&[])
            }
        }else{            
            let dict_start = 2 + (self.get_fixed_bin_count() as usize * 2);
            &self.data[dict_start + self.get_bin_addr(index)..dict_start + self.get_bin_addr(index + 1)]
        }
    }
    
    pub fn has_bin(&self, index: u8) -> bool {
        if index < self.get_fixed_bin_count() { return true; }
        #[cfg(feature = "alloc")]
        {
            let dict = unsafe { &*(&raw const DICT) };
            ((index - self.get_fixed_bin_count()) as usize) < dict.len()
        }
        #[cfg(not(feature = "alloc"))]
        false
    }

    /// Stores a dynamic bin, returns false if it cannot be stored
    pub fn set_bin(&self, _index: u8, _bin: &[u8]) -> bool {
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
    
    pub fn get_instructions(&self) -> &[u8] {   
        if self.get_fixed_bin_count() == 0 {
            &self.data[2..]                    
        }else{
            &self.data[2 + (self.get_fixed_bin_count() as usize * 2) + (self.get_bin_addr(self.get_fixed_bin_count()))..]                
        }
    }
}
use core::fmt::{Write};

pub struct Writer <'a> {
    buf: &'a mut [u8],
    pos: usize
} 

impl Writer <'_> {
    pub fn from_buffer(buf: &mut [u8]) -> Writer <'_> {
        Writer {
            buf: buf,
            pos : 0
        }
    }

    pub fn to_str(&self) -> &str {
        return str::from_utf8(&self.buf[0..self.pos]).unwrap_or("");
    }
}

impl Write for Writer <'_> {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        for b in s.as_bytes() {
            if self.pos >= self.buf.len() { return Err(core::fmt::Error {}); }
            self.buf[self.pos] = *b;
            self.pos += 1;
        };
        Ok(())
    }
}                                
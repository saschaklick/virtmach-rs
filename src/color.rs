//! Color atoms: the encoder for the # color literals of the BASIC compiler and the decoder for interrupts.
//!
//! A color is encoded in an atom as, from the highest bit down:
//!   - i8:  A RR GGG BB
//!   - i16: A RRRRR GGGGG BBBBB
//!   - i32: A _______ RRRRRRRR GGGGGGGG BBBBBBBB
//!
//! The alpha bit is the sign bit, so alpha 1 (also the default) is a negative value and alpha 0 a positive one.

use crate::VMAtom;

/// Namespace for encoding and decoding color atoms
pub struct Color;

impl Color {
    /// Converts a CSS style color (#rgb, #rgba, #rrggbb, #rrggbbaa) to a color atom.
    /// Alpha is reduced to 1 bit (0 stays 0, everything else is 1) and is 1 if not given.
    /// The text must be a valid color, the BASIC grammar makes sure of that.
    #[cfg(feature = "basic")]
    pub fn parse(text: &str) -> VMAtom {
        let mut channels = [0u8; 4];
        let mut count = 0;
        let mut digits = text[1..].chars().map(|c| c.to_digit(16).unwrap() as u8);
        let short = text.len() <= 5;
        while let Some(first) = digits.next() {
            channels[count] = if short { first * 17 } else { first * 16 + digits.next().unwrap() };
            count += 1;
        }
        Self::from_rgba(channels[0], channels[1], channels[2], if count == 4 { channels[3] } else { 255 })
    }

    /// Encodes 8 bit channels as a color atom, the channels lose their lower bits to fit and
    /// alpha is reduced to 1 bit (0 stays 0, everything else is 1). Works on the bytes of the atom.
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> VMAtom {
        let a = ((a != 0) as u8) << 7;
        let mut bytes = [0u8; size_of::<VMAtom>()];
        match bytes.as_mut_slice() {
            [v] => *v = a | (r >> 6) << 5 | (g >> 5) << 2 | b >> 6,
            [hi, lo] => {
                let g = g >> 3;
                *hi = a | (r >> 3) << 2 | g >> 3;
                *lo = g << 5 | b >> 3;
            }
            [alpha, red, green, blue] => (*alpha, *red, *green, *blue) = (a, r, g, b),
            _ => unreachable!()
        }
        VMAtom::from_be_bytes(bytes)
    }

    /// Decodes a color atom to [r, g, b, a]. Alpha becomes 0 or 255, the color channels are widened to
    /// 8 bits by repeating their bits, so full intensity is 255. Works on the bytes of the atom.
    pub fn to_rgba(atom: VMAtom) -> [u8; 4] {
        let alpha = if atom < 0 { 255 } else { 0 };
        match atom.to_be_bytes().as_slice() {
            [v] => [widen(v >> 5 & 0b11, 2), widen(v >> 2 & 0b111, 3), widen(v & 0b11, 2), alpha],
            [hi, lo] => [widen(hi >> 2 & 0x1f, 5), widen((hi & 0b11) << 3 | lo >> 5, 5), widen(lo & 0x1f, 5), alpha],
            [_, r, g, b] => [*r, *g, *b, alpha],
            _ => unreachable!()
        }
    }
}

/// Widens a value of the given number of bits (1 to 8) to 8 bits by repeating its bits
fn widen(value: u8, bits: u8) -> u8 {
    let mut result = 0u8;
    let mut shift = 8i8;
    while shift > 0 {
        shift -= bits as i8;
        result |= if shift >= 0 { value << shift } else { value >> -shift };
    }
    result
}

#[cfg(all(test, feature = "basic"))]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let c = |s| Color::parse(s) as i64;
        #[cfg(feature = "i8")] {
            assert_eq!(c("#fff") as u8, 0xff);
            assert_eq!(c("#000") as u8, 0x80);
            assert_eq!(c("#ff0000") as u8, 0xe0);
            assert_eq!(c("#00ff00ff") as u8, 0x9c);
            assert_eq!(c("#0000ff00") as u8, 0x03);
            assert_eq!(c("#ffffff00"), 0x7f);
            assert!(c("#000") < 0 && c("#00000000") == 0);
        }
        #[cfg(feature = "i16")] {
            assert_eq!(c("#fff") as u16, 0xffff);
            assert_eq!(c("#ff0000") as u16, 0xfc00);
            assert_eq!(c("#f008") as u16, 0xfc00);
            assert_eq!(c("#0000ff00") as u16, 0x001f);
            assert_eq!(c("#000000ff") as u16, 0x8000);
        }
        #[cfg(feature = "i32")] {
            assert_eq!(c("#fff") as u32, 0x80ffffff);
            assert_eq!(c("#123456") as u32, 0x80123456);
            assert_eq!(c("#12345600") as u32, 0x00123456);
            assert_eq!(c("#1234567f") as u32, 0x80123456);
        }
    }

    #[test]
    fn decode() {
        #[cfg(feature = "i8")] {
            assert_eq!(Color::to_rgba(0xffu8 as VMAtom), [255, 255, 255, 255]);
            assert_eq!(Color::to_rgba(0x80u8 as VMAtom), [0, 0, 0, 255]);
            assert_eq!(Color::to_rgba(0x1c), [0, 255, 0, 0]);
            assert_eq!(Color::to_rgba(0xe0u8 as VMAtom), [255, 0, 0, 255]);
        }
        #[cfg(feature = "i16")] {
            assert_eq!(Color::to_rgba(0xffffu16 as VMAtom), [255, 255, 255, 255]);
            assert_eq!(Color::to_rgba(0xfc00u16 as VMAtom), [255, 0, 0, 255]);
            assert_eq!(Color::to_rgba(0x001f), [0, 0, 255, 0]);
        }
        #[cfg(feature = "i32")] {
            assert_eq!(Color::to_rgba(0x80123456u32 as VMAtom), [0x12, 0x34, 0x56, 255]);
            assert_eq!(Color::to_rgba(0x00123456), [0x12, 0x34, 0x56, 0]);
        }
    }

    #[test]
    fn from_rgba() {
        assert_eq!(Color::from_rgba(255, 0, 0, 255), Color::parse("#f00"));
        assert_eq!(Color::from_rgba(0x12, 0x34, 0x56, 0), Color::parse("#12345600"));
        assert_eq!(Color::from_rgba(0x12, 0x34, 0x56, 1), Color::parse("#12345601"));
        assert!(Color::from_rgba(0, 0, 0, 7) < 0 && Color::from_rgba(0, 0, 0, 0) == 0);
    }

    #[test]
    fn round_trip() {
        // full and zero channels and the alpha survive encode + decode in every atom size
        for (text, rgba) in [("#fff", [255, 255, 255, 255]), ("#f00", [255, 0, 0, 255]), ("#0f0", [0, 255, 0, 255]),
                             ("#00f", [0, 0, 255, 255]), ("#0000", [0, 0, 0, 0]), ("#ff00ff00", [255, 0, 255, 0])] {
            assert_eq!(Color::to_rgba(Color::parse(text)), rgba, "{}", text);
        }
    }
}

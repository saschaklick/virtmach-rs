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

    /// Encodes channels given as atoms, 0 to 255, as an opaque color atom. Values outside of the range are cut to it.
    /// This is the rgb function of the surface interrupt.
    pub fn from_rgb(r: VMAtom, g: VMAtom, b: VMAtom) -> VMAtom {
        let channel = |v: VMAtom| (v as i64).clamp(0, 255) as u8;
        Self::from_rgba(channel(r), channel(g), channel(b), 255)
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

/// The default palette of VGA (mode 13h) as 8 bit RGB: 16 colors, 16 grays and 9 groups of 24 colors that
/// sweep the hues in three brightnesses and three saturations, then 8 blacks. Index n of BASIC drawing
/// statements is looked up here when compiling.
pub static VGA_PALETTE: [[u8; 3]; 256] = vga_palette();

const fn vga_palette() -> [[u8; 3]; 256] {
    // the DAC has 6 bits per channel, they are widened to 8
    const fn wide(v: u8) -> u8 { (v << 2) | (v >> 4) }
    const EGA: [[u8; 3]; 16] = [[0, 0, 0], [0, 0, 42], [0, 42, 0], [0, 42, 42], [42, 0, 0], [42, 0, 42], [42, 21, 0], [42, 42, 42],
        [21, 21, 21], [21, 21, 63], [21, 63, 21], [21, 63, 63], [63, 21, 21], [63, 21, 63], [63, 63, 21], [63, 63, 63]];
    const GRAY: [u8; 16] = [0, 5, 8, 11, 14, 17, 20, 24, 28, 32, 36, 40, 45, 50, 56, 63];
    // highest and lowest channel of each group of 24
    const GROUPS: [(u8, u8); 9] = [(63, 0), (63, 31), (63, 45), (28, 0), (28, 14), (28, 20), (16, 0), (16, 8), (16, 11)];

    let mut palette = [[0u8; 3]; 256];
    let mut i = 0;
    while i < 16 {
        palette[i] = EGA[i];
        palette[16 + i] = [GRAY[i]; 3];
        i += 1;
    }
    let mut group = 0;
    while group < 9 {
        let (high, low) = GROUPS[group];
        let range = high - low;
        let mut pos = 0u8;
        while pos < 24 {
            // one of the four steps from one end of the range to the other, rounded down at a half
            let (up, down) = {
                let k = (pos % 4 + if pos % 4 == 0 && pos > 0 { 4 } else { 0 }) as u8;
                let step = (k * range + 1) / 4;
                (low + step, high - step)
            };
            palette[32 + group * 24 + pos as usize] = match pos {
                0..=4 => [up, low, high],
                5..=8 => [high, low, down],
                9..=12 => [high, up, low],
                13..=16 => [down, high, low],
                17..=20 => [low, high, up],
                _ => [low, down, high]
            };
            pos += 1;
        }
        group += 1;
    }
    let mut i = 0;
    while i < 248 {
        palette[i] = [wide(palette[i][0]), wide(palette[i][1]), wide(palette[i][2])];
        i += 1;
    }
    palette
}

impl Color {
    /// The opaque color atom of an entry of the default VGA palette
    pub fn vga(index: u8) -> VMAtom {
        let [r, g, b] = VGA_PALETTE[index as usize];
        Self::from_rgba(r, g, b, 255)
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

#[cfg(test)]
mod rgb_tests {
    use super::*;

    #[test]
    fn vga_palette_matches_the_default_one() {
        let p = &VGA_PALETTE;
        // the 16 colors
        assert_eq!([p[0], p[1], p[4], p[6], p[7], p[8], p[12], p[15]],
            [[0, 0, 0], [0, 0, 170], [170, 0, 0], [170, 85, 0], [170, 170, 170], [85, 85, 85], [255, 85, 85], [255, 255, 255]]);
        // the grays
        assert_eq!([p[16], p[17], p[24], p[31]], [[0, 0, 0], [20, 20, 20], [113, 113, 113], [255, 255, 255]]);
        // the hues of the first group: blue, magenta, red, yellow, green, cyan, the one before blue
        assert_eq!([p[32], p[36], p[40], p[44], p[48], p[52], p[55]],
            [[0, 0, 255], [255, 0, 255], [255, 0, 0], [255, 255, 0], [0, 255, 0], [0, 255, 255], [0, 65, 255]]);
        // the steps between them are rounded like the DAC values of the real palette: 16, 31, 47
        assert_eq!([p[33], p[34], p[35]], [[65, 0, 255], [125, 0, 255], [190, 0, 255]]);
        // lighter, less saturated and darker groups, the last color of the last group, the 8 blacks
        assert_eq!([p[56], p[80], p[104], p[128], p[247]], [[125, 125, 255], [182, 182, 255], [0, 0, 113], [56, 56, 113], [44, 48, 65]]);
        assert!(p[248..].iter().all(|c| *c == [0, 0, 0]));
        assert_eq!(Color::to_rgba(Color::vga(15))[..3], [255, 255, 255]);
        assert!(Color::vga(0) < 0, "opaque black is a negative atom");
    }

    #[test]
    fn rgb_is_opaque_and_cut_to_the_channel_range() {
        assert_eq!(Color::from_rgb(100, 0, 50), Color::from_rgba(100, 0, 50, 255));
        // the largest and smallest atoms are cut to 0..255, whatever the size of an atom is
        let top = (VMAtom::MAX as i64).min(255) as u8;
        assert_eq!(Color::from_rgb(VMAtom::MAX, VMAtom::MIN, 7), Color::from_rgba(top, 0, 7, 255));
        assert!(Color::from_rgb(0, 0, 0) < 0, "alpha 1 is a negative atom");
        #[cfg(not(feature = "i8"))]
        assert_eq!(Color::to_rgba(Color::from_rgb(255, 255, 255)), [255, 255, 255, 255]);
    }
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

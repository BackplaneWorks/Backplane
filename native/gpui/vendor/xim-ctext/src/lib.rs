//! Encodes to COMPOUND_TEXT with a UTF-8 segment; decodes COMPOUND_TEXT's common sets (see `compound_text_to_utf8`)

#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[cfg(feature = "std")]
use std::io::{self, Write};

const UTF8_START: &[u8] = &[0x1B, 0x25, 0x47];
const UTF8_END: &[u8] = &[0x1B, 0x25, 0x40];

/// Wrapper for reduce allocation
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CText<'s> {
    utf8: &'s str,
}

impl<'s> fmt::Debug for CText<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.utf8)
    }
}

impl<'s> fmt::Display for CText<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.utf8)
    }
}

impl<'s> CText<'s> {
    pub const fn new(utf8: &'s str) -> Self {
        Self { utf8 }
    }

    pub const fn len(self) -> usize {
        self.utf8.len() + UTF8_START.len() + UTF8_END.len()
    }

    #[cfg(feature = "std")]
    pub fn write(self, mut out: impl Write) -> io::Result<usize> {
        let mut writed = 0;
        writed += out.write(UTF8_START)?;
        writed += out.write(self.utf8.as_bytes())?;
        writed += out.write(UTF8_END)?;
        Ok(writed)
    }
}

/// Encoding utf8 to COMPOUND_TEXT with utf8 escape
pub fn utf8_to_compound_text(text: &str) -> Vec<u8> {
    let mut ret = Vec::with_capacity(text.len() + 6);
    ret.extend_from_slice(UTF8_START);
    ret.extend_from_slice(text.as_bytes());
    ret.extend_from_slice(UTF8_END);
    ret
}

#[derive(Debug, Clone)]
pub enum DecodeError {
    InvalidEncoding,
    UnsupportedEncoding,
    Utf8Error(alloc::string::FromUtf8Error),
}

impl From<alloc::string::FromUtf8Error> for DecodeError {
    fn from(err: alloc::string::FromUtf8Error) -> Self {
        DecodeError::Utf8Error(err)
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncoding => write!(f, "Invalid compound text"),
            Self::UnsupportedEncoding => write!(f, "This encoding is not supported yet"),
            Self::Utf8Error(e) => write!(f, "Not a valid utf8 {}", e),
        }
    }
}

/// A character set designated to GL (0x21..=0x7E) or GR (0xA0..=0xFF).
#[derive(Clone, Copy)]
enum Set {
    /// ISO 8859-1's left half (ASCII): GL's initial set.
    Ascii,
    /// ISO 8859-1's right half: GR's initial set (byte 0xB4 is U+00B4).
    Latin1,
    /// JIS X 0201 Roman: ASCII with YEN SIGN at 0x5C and OVERLINE at 0x7E.
    JisRoman,
    /// JIS X 0201 Katakana (half width).
    Katakana,
    /// The right half of another 96-character ISO 8859 set, decoded by its single-byte encoding.
    Right96(&'static encoding_rs::Encoding),
    /// A 94^2 set (GB 2312, JIS X 0208, KS C 5601): two bytes, decoded as their EUC form (GB 2312 through GBK, which contains it).
    Double(&'static encoding_rs::Encoding),
}

impl Set {
    fn width(self) -> usize {
        match self {
            Set::Double(_) => 2,
            _ => 1,
        }
    }

    /// Decodes `bytes` (one character, the 7-bit value of each byte) into `out`.
    fn push(self, bytes: &[u8], out: &mut String) -> Result<(), DecodeError> {
        let c = bytes[0] & 0x7F;
        // a 94 or 94^2 set has no character at 0x20 / 0x7F (GR 0xA0 / 0xFF); every byte of a character is in 0x21..=0x7E once masked
        if !matches!(self, Set::Latin1 | Set::Right96(_)) && bytes.iter().any(|&b| !(0x21..=0x7E).contains(&(b & 0x7F))) {
            return Err(DecodeError::InvalidEncoding);
        }
        let ch = match self {
            Set::Ascii => c as char,
            Set::Latin1 => (c | 0x80) as char,
            Set::JisRoman => match c {
                0x5C => '\u{A5}',
                0x7E => '\u{203E}',
                c => c as char,
            },
            Set::Katakana => match c {
                0x21..=0x5F => {
                    core::char::from_u32(0xFF61 + (c - 0x21) as u32).ok_or(DecodeError::InvalidEncoding)?
                }
                _ => return Err(DecodeError::InvalidEncoding),
            },
            Set::Right96(enc) => {
                let byte = [c | 0x80];
                let s = enc
                    .decode_without_bom_handling_and_without_replacement(&byte)
                    .ok_or(DecodeError::InvalidEncoding)?;
                out.push_str(&s);
                return Ok(());
            }
            Set::Double(enc) => {
                let pair = [bytes[0] | 0x80, bytes[1] | 0x80];
                let s = enc
                    .decode_without_bom_handling_and_without_replacement(&pair)
                    .ok_or(DecodeError::InvalidEncoding)?;
                out.push_str(&s);
                return Ok(());
            }
        };
        out.push(ch);
        Ok(())
    }
}

/// The 94-character set named by an escape sequence's final byte (`ESC ( F`, `ESC ) F`).
fn set94(f: u8) -> Result<Set, DecodeError> {
    match f {
        b'B' => Ok(Set::Ascii),
        b'J' => Ok(Set::JisRoman),
        b'I' => Ok(Set::Katakana),
        _ => Err(DecodeError::UnsupportedEncoding),
    }
}

/// The 96-character set named by `ESC - F`: the right half of an ISO 8859 part.
fn set96(f: u8) -> Result<Set, DecodeError> {
    use encoding_rs::*;
    Ok(match f {
        b'A' => Set::Latin1,
        b'B' => Set::Right96(ISO_8859_2),
        b'C' => Set::Right96(ISO_8859_3),
        b'D' => Set::Right96(ISO_8859_4),
        b'F' => Set::Right96(ISO_8859_7),
        b'G' => Set::Right96(ISO_8859_6),
        b'H' => Set::Right96(ISO_8859_8),
        b'L' => Set::Right96(ISO_8859_5),
        // ISO 8859-9's right half is windows-1254's (encoding_rs maps the one to the other)
        b'M' => Set::Right96(WINDOWS_1254),
        b'V' => Set::Right96(ISO_8859_10),
        b'Y' => Set::Right96(ISO_8859_13),
        b'_' => Set::Right96(ISO_8859_14),
        b'b' => Set::Right96(ISO_8859_15),
        b'f' => Set::Right96(ISO_8859_16),
        // TIS-620 (ISO 8859-11), Thai: windows-874's right half
        b'T' => Set::Right96(WINDOWS_874),
        _ => return Err(DecodeError::UnsupportedEncoding),
    })
}

/// The 94^2 set named by `ESC $ ( F` / `ESC $ ) F`.
fn set94n(f: u8) -> Result<Set, DecodeError> {
    match f {
        b'A' => Ok(Set::Double(encoding_rs::GBK)),
        b'B' => Ok(Set::Double(encoding_rs::EUC_JP)),
        b'C' => Ok(Set::Double(encoding_rs::EUC_KR)),
        _ => Err(DecodeError::UnsupportedEncoding),
    }
}

/// Decodes COMPOUND_TEXT (X11 Compound Text Encoding) to UTF-8.
///
/// GL starts as ASCII and GR as ISO 8859-1's right half, so unescaped bytes 0xA0..=0xFF are Latin-1 (an IM in a C or Latin-1
/// locale sends the dead acute's accent as the single byte 0xB4). Handled: the designations of ISO 8859 right halves, JIS X 0201,
/// GB 2312, JIS X 0208 and KS C 5601 to GL or GR, the UTF-8 segments Xlib writes (`ESC % G` .. `ESC % @`), and direction
/// sequences (skipped). Never panics: anything else is an error.
pub fn compound_text_to_utf8(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut out = String::with_capacity(bytes.len());
    let mut gl = Set::Ascii;
    let mut gr = Set::Latin1;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            0x1B => {
                // ESC, intermediates 0x20..=0x2F, a final byte 0x30..=0x7E
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && (0x20..=0x2F).contains(&bytes[j]) {
                    j += 1;
                }
                let f = *bytes.get(j).ok_or(DecodeError::InvalidEncoding)?;
                if !(0x30..=0x7E).contains(&f) {
                    return Err(DecodeError::InvalidEncoding);
                }
                match &bytes[start..j] {
                    b"(" => gl = set94(f)?,
                    b")" if f != b'B' => gr = set94(f)?,
                    b"-" => gr = set96(f)?,
                    b"$(" => gl = set94n(f)?,
                    b"$)" => gr = set94n(f)?,
                    b"%" if f == b'G' => {
                        // UTF-8 up to ESC % @ (or the end of the text)
                        let rest = &bytes[j + 1..];
                        let end = rest
                            .windows(UTF8_END.len())
                            .position(|w| w == UTF8_END)
                            .unwrap_or(rest.len());
                        out.push_str(&String::from_utf8(rest[..end].to_vec())?);
                        j += end + if end < rest.len() { UTF8_END.len() } else { 0 };
                    }
                    // the end of a UTF-8 segment outside one: nothing to do
                    b"%" if f == b'@' => {}
                    _ => return Err(DecodeError::UnsupportedEncoding),
                }
                i = j + 1;
            }
            // CSI 1 ] / CSI 2 ] / CSI ]: a direction sequence, skipped (the text's order is kept as it is); any other CSI is an error
            0x9B => match bytes.get(i + 1..) {
                Some([b'1' | b'2', b']', ..]) => i += 3,
                Some([b']', ..]) => i += 2,
                _ => return Err(DecodeError::InvalidEncoding),
            },
            // C0 controls (HT, NL) and space are themselves whatever is designated
            0x00..=0x20 | 0x7F => {
                out.push(b as char);
                i += 1;
            }
            0x21..=0x7E | 0xA0..=0xFF => {
                let set = if b < 0x80 { gl } else { gr };
                let n = set.width();
                let ch = bytes.get(i..i + n).ok_or(DecodeError::InvalidEncoding)?;
                if n > 1 && ch.iter().any(|&c| (c < 0x80) != (b < 0x80)) {
                    return Err(DecodeError::InvalidEncoding);
                }
                set.push(ch, &mut out)?;
                i += n;
            }
            // other C1 controls are not allowed in compound text
            _ => return Err(DecodeError::InvalidEncoding),
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    #[test]
    fn korean() {
        const UTF8: &str = "가나다";
        const COMP: &[u8] = &[
            27, 37, 71, 234, 176, 128, 235, 130, 152, 235, 139, 164, 27, 37, 64,
        ];
        assert_eq!(crate::utf8_to_compound_text(UTF8), COMP);
        assert_eq!(crate::compound_text_to_utf8(COMP).unwrap(), UTF8);
    }

    #[test]
    fn latin1_right_half_is_the_default() {
        // ibus over XIM in a C locale: the dead acute's preedit, then the composed commit
        assert_eq!(crate::compound_text_to_utf8(&[180]).unwrap(), "\u{B4}");
        assert_eq!(crate::compound_text_to_utf8(&[0xE9]).unwrap(), "é");
        assert_eq!(crate::compound_text_to_utf8(b"caf\xE9 ok").unwrap(), "café ok");
        assert_eq!(crate::compound_text_to_utf8(&[0x1B, 0x2D, 0x41, 0xE9]).unwrap(), "é");
    }

    #[test]
    fn designations_and_segments() {
        // ESC - B: ISO 8859-2's right half (0xB1 is U+0105)
        assert_eq!(crate::compound_text_to_utf8(&[0x1B, 0x2D, 0x42, 0xB1]).unwrap(), "\u{105}");
        // Latin-1, a UTF-8 segment, then Latin-1 again
        let mut ct = vec![b'a', 0xE9];
        ct.extend_from_slice(&crate::utf8_to_compound_text("가"));
        ct.push(0xFC);
        assert_eq!(crate::compound_text_to_utf8(&ct).unwrap(), "aé가ü");
        // JIS X 0208 in GR, ASCII in GL
        assert_eq!(
            crate::compound_text_to_utf8(&[0x1B, 0x24, 0x29, 0x42, 0xC5, 0xEC, b'x']).unwrap(),
            "東x"
        );
        // half-width katakana in GR
        assert_eq!(crate::compound_text_to_utf8(&[0x1B, 0x29, 0x49, 0xB1]).unwrap(), "\u{FF71}");
        // a direction sequence is skipped
        assert_eq!(crate::compound_text_to_utf8(&[0x9B, b'1', b']', b'a', 0x9B, b']']).unwrap(), "a");
        // TIS-620 (0xA1 is U+0E01)
        assert_eq!(crate::compound_text_to_utf8(&[0x1B, 0x2D, 0x54, 0xA1]).unwrap(), "\u{E01}");
        // a UTF-8 segment without its end marker runs to the end of the text
        assert_eq!(crate::compound_text_to_utf8(&[0x1B, 0x25, 0x47, 0xC3, 0xA9]).unwrap(), "é");
    }

    #[test]
    fn bad_text_is_an_error_not_a_panic() {
        for bad in [
            &[0x1B][..],
            &[0x1B, 0x25][..],
            &[0x1B, 0x25, 0x47, 0xFF][..],
            &[0x1B, 0x24, 0x28, 0x42, 0x45][..],
            &[0x85][..],
            &[0x1B, 0x29, 0x49, 0xF0][..],
            &[0x1B, 0x25, 0x2F, 0x31, 0x80, 0x80][..],
            // a two-byte character whose trail is an escape, a space, a newline, or not a graphic byte
            &[0x1B, 0x24, 0x28, 0x41, 0x30, 0x1B, 0x28, 0x42][..],
            &[0x1B, 0x24, 0x28, 0x41, 0x30, 0x20][..],
            &[0x1B, 0x24, 0x28, 0x41, 0x30, 0x0A][..],
            &[0x1B, 0x24, 0x28, 0x43, 0x30, 0x20][..],
            &[0x1B, 0x24, 0x29, 0x41, 0xA0, 0xA1][..],
            // a 94 set in GR has nothing at 0xA0 / 0xFF; ASCII is not designated to GR
            &[0x1B, 0x29, 0x4A, 0xA0][..],
            &[0x1B, 0x29, 0x4A, 0xFF][..],
            &[0x1B, 0x29, 0x42, 0xC1][..],
            // a CSI that is not a direction sequence
            &[0x9B, 0xE9, 0xE9, b'a'][..],
            &[0x9B][..],
        ] {
            assert!(crate::compound_text_to_utf8(bad).is_err(), "{:?}", bad);
        }
    }

    #[test]
    fn iso_2011_jp() {
        const UTF8: &str = "東京";
        const COMP: &[u8] = &[27, 36, 40, 66, 69, 108, 53, 126];
        assert_eq!(crate::compound_text_to_utf8(COMP).unwrap(), UTF8);
    }
}

use std::{io::Write, mem};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct FixedLengthGraphemeString {
    pub string: String,
    pub max_grapheme_length: u16,
    len: u16,
}

impl Write for FixedLengthGraphemeString {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8(buf.to_vec()).unwrap();
        self.push_str(&s);
        Ok(s.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

impl FixedLengthGraphemeString {
    pub fn empty(max_grapheme_length: u16) -> FixedLengthGraphemeString {
        FixedLengthGraphemeString {
            string: String::new(),
            max_grapheme_length,
            len: 0,
        }
    }

    pub fn new<S: Into<String>>(s: S, max_grapheme_length: u16) -> FixedLengthGraphemeString {
        let mut fixed_length_grapheme_string = FixedLengthGraphemeString::empty(max_grapheme_length);
        fixed_length_grapheme_string.push_grapheme_str(s);
        fixed_length_grapheme_string
    }

    pub fn push_grapheme_str<S: Into<String>>(&mut self, s: S) {
        if self.len > self.max_grapheme_length {
            return;
        }

        let str = s.into();
        for grapheme in str.graphemes(true) {
            self.len += 1;
            if self.len > self.max_grapheme_length {
                break;
            }
            self.string.push_str(grapheme);
        }

        if self.len > self.max_grapheme_length {
            let mut len = 0;
            let mut str = String::new();
            mem::swap(&mut str, &mut self.string);

            for grapheme in str.graphemes(true) {
                if len + 3 >= self.max_grapheme_length {
                    break;
                }
                len += 1;
                self.string.push_str(grapheme);
            }
            self.string.push_str("...");
        }
    }

    pub fn push_str(&mut self, s: &str) { self.string.push_str(s); }
}

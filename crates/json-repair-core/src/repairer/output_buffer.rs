//! Output string builder with depth-0 position tracking.
//!
//! [`OutputBuffer`] accumulates the repaired JSON string and records a
//! bookmark (`last_depth0_pos`) at the last position where the bracket
//! depth was zero.  This bookmark is used by [`trim_suffix_junk`] to
//! strip trailing whitespace after the final closing bracket.

/// Accumulator for repaired JSON output.
///
/// `out` holds the growing output string.  `last_depth0_pos` tracks the
/// byte offset of the last emitted character at bracket-depth 0, used
/// for suffix-junk trimming.
pub(crate) struct OutputBuffer {
    pub out: String,
    pub last_depth0_pos: usize,
}

impl OutputBuffer {
    /// Creates a new `OutputBuffer` with capacity capped at 256 KiB.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.min(256 * 1024);
        OutputBuffer {
            out: String::with_capacity(cap),
            last_depth0_pos: 0,
        }
    }

    /// Appends a single character to the output.
    pub fn emit_char(&mut self, c: char) {
        self.out.push(c);
    }

    /// Appends a string slice to the output.
    pub fn emit_str(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Writes a `\uXXXX` escape sequence for the given Unicode code point,
    /// using lowercase hexadecimal digits.
    pub fn emit_unicode_escape(&mut self, code: u32) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.out.push_str("\\u");
        self.out
            .push(char::from(HEX[((code >> 12) & 0xF) as usize]));
        self.out.push(char::from(HEX[((code >> 8) & 0xF) as usize]));
        self.out.push(char::from(HEX[((code >> 4) & 0xF) as usize]));
        self.out.push(char::from(HEX[(code & 0xF) as usize]));
    }

    /// Removes a trailing `,` if present.
    ///
    /// Prevents output like `{"a":1,}`.
    pub fn trim_trailing_comma(&mut self) {
        if self.out.ends_with(',') {
            self.out.pop();
        }
    }

    /// Returns `true` if a comma separator is needed before the next element.
    ///
    /// A comma is needed when the output does not end with `,`, `{`, or
    /// `[`.  The caller is responsible for checking that at least one element
    /// has already been emitted.
    pub fn needs_comma_in_output(&self) -> bool {
        !matches!(self.out.as_bytes().last(), Some(b',' | b'{' | b'['))
    }

    /// Trims trailing whitespace after the last depth-0 position.
    ///
    /// Only removes when the tail is *all* whitespace.  Non-whitespace
    /// trailing junk is preserved.
    pub fn trim_suffix_junk(&mut self) {
        debug_assert!(
            self.last_depth0_pos <= self.out.len(),
            "trim_suffix_junk: last_depth0_pos exceeds output length"
        );
        if self.last_depth0_pos < self.out.len() {
            let tail = &self.out[self.last_depth0_pos..];
            if tail.trim().is_empty() {
                self.out.truncate(self.last_depth0_pos);
            }
        }
    }

    /// Takes the output buffer, replacing it with an empty string.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    /// Records the current output length as the depth-0 position.
    pub fn set_depth0_pos(&mut self) {
        self.last_depth0_pos = self.out.len();
    }

    /// Returns `true` if the output ends with character `c`.
    pub fn ends_with(&self, c: char) -> bool {
        self.out.ends_with(c)
    }
}

//! Read-only cursor over the input text stream.
//!
//! [`InputCursor`] wraps a `&str` and a byte position `i`, providing
//! character-level navigation without allocation or mutation of the
//! text.  All methods are pure position queries — the struct never
//! modifies the text itself.

use crate::preprocess::char_at;

/// Read-only cursor over the input text.
///
/// `text` is the full input; `i` is the current byte offset (always on a
/// UTF-8 character boundary).  Methods that advance `i` (`skip_ws`) are
/// the only mutation — the text itself is never altered.
#[derive(Debug)]
pub(crate) struct InputCursor<'a> {
    pub text: &'a str,
    pub i: usize,
}

impl<'a> InputCursor<'a> {
    /// Creates a new `InputCursor` positioned at byte 0.
    pub fn new(text: &'a str) -> Self {
        InputCursor { text, i: 0 }
    }

    /// Returns the character at the current cursor position, or `'\0'` if at EOF.
    pub fn cur(&self) -> char {
        self.char_at(self.i)
    }

    /// Returns the character at byte offset `pos`, or `'\0'` if `pos` is out
    /// of bounds.
    pub fn char_at(&self, pos: usize) -> char {
        if pos < self.text.len() {
            char_at(self.text, pos)
        } else {
            '\0'
        }
    }

    /// Checks whether the text starting at `self.i` begins with `s`.
    ///
    /// # Panics
    ///
    /// Panics when `s` contains non-ASCII characters — only ASCII patterns are
    /// supported.
    pub fn peek_is(&self, s: &str) -> bool {
        assert!(s.is_ascii(), "peek_is: non-ASCII pattern {s:?}");
        self.text[self.i..].starts_with(s)
    }

    /// Advances the cursor past ASCII whitespace.
    pub fn skip_ws(&mut self) {
        let bytes = self.text.as_bytes();
        let n = self.text.len();
        while self.i < n && bytes[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// Returns `pos` advanced past ASCII whitespace without modifying the
    /// cursor.
    pub fn skip_ws_at(&self, mut pos: usize) -> usize {
        let bytes = self.text.as_bytes();
        let n = self.text.len();
        while pos < n && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        pos
    }

    /// Returns `true` when the cursor has reached or passed the end of input.
    pub fn is_empty(&self) -> bool {
        self.i >= self.text.len()
    }
}

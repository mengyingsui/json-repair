//! Read-only cursor over the input text stream.
//!
//! [`InputCursor`] wraps a `&str` and a byte position `i`, providing
//! character-level navigation without allocation or mutation of the
//! text.  All methods are pure position queries — the struct never
//! modifies the text itself.

use crate::util::char_at;

/// Read-only cursor over the input text.
///
/// All field access goes through methods — the internal `text` and `i`
/// are never exposed directly, mirroring how `std::io::Cursor` wraps its
/// inner buffer.
#[derive(Debug)]
pub(crate) struct InputCursor<'a> {
    text: &'a str,
    i: usize,
}

impl<'a> InputCursor<'a> {
    /// Creates a new `InputCursor` positioned at byte 0.
    pub fn new(text: &'a str) -> Self {
        InputCursor { text, i: 0 }
    }

    /// Returns the full input text.
    pub fn text(&self) -> &str {
        self.text
    }

    /// Returns the input text as raw bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.text.as_bytes()
    }

    /// Returns the total byte length of the input text.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns the current byte offset.
    pub fn pos(&self) -> usize {
        self.i
    }

    /// Sets the cursor to byte offset `pos`.
    pub fn set_pos(&mut self, pos: usize) {
        self.i = pos;
    }

    /// Advances the cursor by `n` bytes.
    pub fn advance(&mut self, n: usize) {
        self.i += n;
    }

    /// Returns the character at the current cursor position, or `None` if at EOF.
    pub fn cur(&self) -> Option<char> {
        char_at(self.text, self.i)
    }

    /// Returns the character at byte offset `pos`, or `None` if `pos` is out
    /// of bounds or not at a valid UTF-8 char boundary.
    pub fn char_at(&self, pos: usize) -> Option<char> {
        char_at(self.text, pos)
    }

    /// Checks whether the text starting at `self.i` begins with `s`.
    ///
    /// Non-ASCII patterns are rejected at debug time and return `false` in
    /// release builds — only ASCII patterns are supported.
    pub fn peek_is(&self, s: &str) -> bool {
        debug_assert!(s.is_ascii(), "peek_is: non-ASCII pattern {s:?}");
        s.is_ascii() && self.text[self.i..].starts_with(s)
    }

    /// Advances the cursor past ASCII whitespace.
    ///
    /// Scans whitespace in 4-byte chunks for a modest throughput win on
    /// indented inputs, then handles the scalar tail byte-by-byte.
    pub fn skip_ws(&mut self) {
        let bytes = self.text.as_bytes();
        let n = self.text.len();
        while self.i + 4 <= n && is_ws_chunk(bytes, self.i) {
            self.i += 4;
        }
        while self.i < n && bytes[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// Returns `pos` advanced past ASCII whitespace without modifying the
    /// cursor.
    ///
    /// Like [`skip_ws`](Self::skip_ws), this scans 4-byte chunks before
    /// finishing the tail byte-by-byte.
    pub fn skip_ws_at(&self, mut pos: usize) -> usize {
        let bytes = self.text.as_bytes();
        let n = self.text.len();
        while pos + 4 <= n && is_ws_chunk(bytes, pos) {
            pos += 4;
        }
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

// Returns true when all four bytes starting at `start` are ASCII whitespace.
fn is_ws_chunk(bytes: &[u8], start: usize) -> bool {
    bytes[start].is_ascii_whitespace()
        && bytes[start + 1].is_ascii_whitespace()
        && bytes[start + 2].is_ascii_whitespace()
        && bytes[start + 3].is_ascii_whitespace()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cursor_at_zero() {
        let c = InputCursor::new("hello");
        assert_eq!(c.pos(), 0);
        assert_eq!(c.cur(), Some('h'));
        assert!(!c.is_empty());
    }

    #[test]
    fn cur_at_eof_returns_none() {
        let c = InputCursor::new("");
        assert_eq!(c.cur(), None);
        assert!(c.is_empty());
    }

    #[test]
    fn char_at_in_bounds() {
        let c = InputCursor::new("hello");
        assert_eq!(c.char_at(0), Some('h'));
        assert_eq!(c.char_at(4), Some('o'));
    }

    #[test]
    fn char_at_out_of_bounds_returns_none() {
        let c = InputCursor::new("hi");
        assert_eq!(c.char_at(2), None);
        assert_eq!(c.char_at(100), None);
    }

    #[test]
    fn char_at_multibyte_utf8() {
        // U+00A0 = 0xC2 0xA0, U+1F600 = 0xF0 0x9F 0x98 0x80
        let c = InputCursor::new("\u{00A0}\u{1F600}!");
        assert_eq!(c.char_at(0), Some('\u{00A0}'));
        assert_eq!(c.char_at(2), Some('\u{1F600}'));
        assert_eq!(c.char_at(6), Some('!'));
    }

    #[test]
    fn peek_is_matches_prefix() {
        let c = InputCursor::new("```json");
        assert!(c.peek_is("```"));
        assert!(c.peek_is("```json"));
        assert!(!c.peek_is("```yaml"));
    }

    #[test]
    fn peek_is_at_eof_returns_false() {
        let c = InputCursor::new("hi");
        assert!(!c.peek_is("hello"));
    }

    #[test]
    fn skip_ws_advances_past_ascii_whitespace() {
        let mut c = InputCursor::new("  \t\n  hello");
        c.skip_ws();
        assert_eq!(c.cur(), Some('h'));
        assert_eq!(c.pos(), 6);
    }

    #[test]
    fn skip_ws_no_whitespace_stays_put() {
        let mut c = InputCursor::new("hello");
        c.skip_ws();
        assert_eq!(c.pos(), 0);
    }

    #[test]
    fn skip_ws_does_not_skip_unicode_whitespace() {
        // JSON spec: only ASCII whitespace is significant.
        // U+00A0 must NOT be skipped by skip_ws.
        let mut c = InputCursor::new("\u{00A0}hello");
        c.skip_ws();
        assert_eq!(c.pos(), 0, "Unicode whitespace should not be skipped");
    }

    #[test]
    fn skip_ws_at_returns_new_position_without_mutating() {
        let c = InputCursor::new("  \t hello");
        let pos = c.skip_ws_at(0);
        assert_eq!(pos, 4);
        assert_eq!(c.pos(), 0, "skip_ws_at must not mutate cursor");
    }

    #[test]
    fn skip_ws_at_at_eof() {
        let c = InputCursor::new("   ");
        assert_eq!(c.skip_ws_at(0), 3);
    }

    #[test]
    fn is_empty_true_at_eof() {
        let mut c = InputCursor::new("hi");
        assert!(!c.is_empty());
        c.set_pos(2);
        assert!(c.is_empty());
        c.set_pos(100);
        assert!(c.is_empty());
    }
}

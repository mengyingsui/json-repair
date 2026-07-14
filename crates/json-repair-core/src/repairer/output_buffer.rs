//! Output string builder with depth-0 position tracking.
//!
//! [`OutputBuffer`] accumulates the repaired JSON string and records a
//! bookmark (`last_depth0_pos`) at the last position where the bracket
//! depth was zero.  This bookmark is used by [`trim_suffix_junk`] to
//! strip trailing whitespace after the final closing bracket.

/// Accumulator for repaired JSON output.
///
/// All mutation goes through methods — the internal `out` string is never
/// exposed directly, mirroring `String`'s own encapsulation of its
/// `Vec<u8>`.
pub(crate) struct OutputBuffer {
    out: String,
    last_depth0_pos: usize,
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
        self.out.push_str("\\u");
        self.out
            .push(char::from_digit((code >> 12) & 0xF, 16).expect("4-bit nibble is 0-15"));
        self.out
            .push(char::from_digit((code >> 8) & 0xF, 16).expect("4-bit nibble is 0-15"));
        self.out
            .push(char::from_digit((code >> 4) & 0xF, 16).expect("4-bit nibble is 0-15"));
        self.out
            .push(char::from_digit(code & 0xF, 16).expect("4-bit nibble is 0-15"));
    }

    /// Removes a trailing `,` if present.
    ///
    /// Prevents output like `{"a":1,}`.
    pub fn trim_trailing_comma(&mut self) {
        if self.out.ends_with(',') {
            self.out.pop();
        }
    }

    /// Removes the last character if it is trailing ASCII whitespace,
    /// repeating until a non-whitespace character or empty.
    ///
    /// Used by the bareword-split path to undo a prematurely emitted
    /// closing quote and any whitespace before it.
    pub fn trim_trailing_whitespace(&mut self) {
        while self.out.ends_with(|c: char| c.is_ascii_whitespace()) {
            self.out.pop();
        }
    }

    /// Removes and returns the last character, or `None` if empty.
    pub fn pop(&mut self) -> Option<char> {
        self.out.pop()
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

    /// Returns `true` if the output is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// Returns the spare capacity of the internal buffer.
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.out.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf() -> OutputBuffer {
        OutputBuffer::new(64)
    }

    #[test]
    fn emit_char_appends() {
        let mut b = buf();
        b.emit_char('a');
        b.emit_char('b');
        assert_eq!(b.take(), "ab");
    }

    #[test]
    fn emit_str_appends() {
        let mut b = buf();
        b.emit_str("hello");
        b.emit_str(" world");
        assert_eq!(b.take(), "hello world");
    }

    #[test]
    fn trim_trailing_comma_removes_one() {
        let mut b = buf();
        b.emit_str("[1,2,");
        b.trim_trailing_comma();
        assert_eq!(b.take(), "[1,2");
    }

    #[test]
    fn trim_trailing_comma_noop_if_absent() {
        let mut b = buf();
        b.emit_str("[1,2");
        b.trim_trailing_comma();
        assert_eq!(b.take(), "[1,2");
    }

    #[test]
    fn needs_comma_after_value() {
        let mut b = buf();
        b.emit_str("{\"a\":1");
        assert!(b.needs_comma_in_output());
    }

    #[test]
    fn needs_comma_after_colon_in_object() {
        // `needs_comma_in_output` returns true when the last byte is not
        // `,`/`{`/`[`.  After `:`, the function returns true, but callers
        // must additionally check `!ends_with(':')` before emitting a comma.
        let mut b = buf();
        b.emit_str("{\"a\":");
        assert!(b.needs_comma_in_output());
        assert!(b.ends_with(':'));
    }

    #[test]
    fn no_comma_needed_after_comma() {
        let mut b = buf();
        b.emit_str("{\"a\":1,");
        assert!(!b.needs_comma_in_output());
    }

    #[test]
    fn no_comma_needed_after_open_bracket() {
        let mut b = buf();
        b.emit_str("[");
        assert!(!b.needs_comma_in_output());
    }

    #[test]
    fn ends_with_check() {
        let mut b = buf();
        b.emit_str(r#""hello""#);
        assert!(b.ends_with('"'));
        assert!(!b.ends_with('x'));
    }

    #[test]
    fn take_empties_buffer() {
        let mut b = buf();
        b.emit_str("hello");
        let s = b.take();
        assert_eq!(s, "hello");
        assert!(b.is_empty());
    }

    #[test]
    fn set_depth0_pos_records_position() {
        let mut b = buf();
        b.emit_str("{\"a\":1}");
        b.set_depth0_pos();
        // last_depth0_pos is private; verify via trim_suffix_junk behavior
        b.emit_str("   ");
        b.trim_suffix_junk();
        assert_eq!(b.take(), "{\"a\":1}");
    }

    #[test]
    fn trim_suffix_junk_removes_whitespace_only_tail() {
        let mut b = buf();
        b.emit_str("{\"a\":1}");
        b.set_depth0_pos();
        b.emit_str("   \n\t");
        b.trim_suffix_junk();
        assert_eq!(b.take(), "{\"a\":1}");
    }

    #[test]
    fn trim_suffix_junk_preserves_non_whitespace_tail() {
        let mut b = buf();
        b.emit_str("{\"a\":1}");
        b.set_depth0_pos();
        b.emit_str("-lnd");
        b.trim_suffix_junk();
        assert_eq!(b.take(), "{\"a\":1}-lnd");
    }

    #[test]
    fn emit_unicode_escape_forms_correct_sequence() {
        let mut b = buf();
        b.emit_unicode_escape(0x41);
        assert_eq!(b.take(), "\\u0041");
        b.emit_unicode_escape(0xFF);
        assert_eq!(b.take(), "\\u00ff");
    }

    #[test]
    fn capacity_capped_at_256kib() {
        let b = OutputBuffer::new(10 * 1024 * 1024);
        assert!(b.capacity() <= 256 * 1024 + 16); // +16 for slack
    }

    #[test]
    fn capacity_not_capped_for_small_input() {
        let b = OutputBuffer::new(128);
        assert!(b.capacity() >= 128);
    }

    #[test]
    fn trim_trailing_whitespace_strips_ws() {
        let mut b = buf();
        b.emit_str("hello   \n\t");
        b.trim_trailing_whitespace();
        assert_eq!(b.take(), "hello");
    }

    #[test]
    fn pop_removes_last_char() {
        let mut b = buf();
        b.emit_str("abc");
        assert_eq!(b.pop(), Some('c'));
        assert_eq!(b.take(), "ab");
    }

    #[test]
    fn pop_empty_returns_none() {
        let mut b = buf();
        assert_eq!(b.pop(), None);
    }
}

//! Inline and block comment removal (`//`, `/* … */`, `#`, `--`).

use super::Repairer;

impl Repairer {
    /// Whether `ch` starts a recognized comment pattern (`//`, `/*`, `#`, `--`).
    #[inline]
    pub(super) fn is_comment_start(&self, ch: char) -> bool {
        ch == '/' || ch == '#' || (ch == '-' && self.peek_is("--"))
    }

    /// Skip a comment starting at `self.i` (`//`, `/* … */`, `#`, `--`).
    pub(super) fn skip_comment(&mut self) {
        if self.peek_is("//") {
            while self.i < self.n && self.cur() != '\n' {
                self.i += self.cur().len_utf8();
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else if self.peek_is("/*") {
            self.i += 2;
            while self.i + 1 < self.n {
                if self.text.as_bytes()[self.i..].starts_with(b"*/") {
                    self.i += 2;
                    return;
                }
                self.i += self.cur().len_utf8();
            }
        } else if self.cur() == '#' || self.peek_is("--") {
            while self.i < self.n && self.cur() != '\n' {
                self.i += self.cur().len_utf8();
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else {
            self.i += 1;
        }
    }
}

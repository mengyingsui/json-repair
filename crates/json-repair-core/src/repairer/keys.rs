//! Unquoted key and value parsing (quote insertion around bare words).

use super::Repairer;

impl Repairer {
    /// Parse an object key: quoted, single-quoted, or unquoted bare word.
    pub(super) fn parse_key(&mut self) {
        self.skip_ws();
        if self.i >= self.n {
            return;
        }
        let ch = self.cur();
        if ch == '"' {
            self.parse_string();
        } else if ch == '\'' {
            self.parse_single_quoted_string();
        } else {
            self.parse_unquoted_key();
        }
    }

    /// Emit chars as a bare word (wrapping in `"`) until a stop predicate
    /// returns `true`.  Shared by `parse_unquoted_key` and `parse_unquoted_value`.
    #[inline]
    fn emit_bare_word(&mut self, is_stop: impl Fn(char) -> bool) {
        while self.i < self.n {
            let ch = self.cur();
            if is_stop(ch) {
                break;
            }
            self.emit_unquoted_char(ch);
            self.i += ch.len_utf8();
        }
    }

    /// Parse an unquoted bare-word key, wrapping it in double quotes.
    pub(super) fn parse_unquoted_key(&mut self) {
        self.emit_char('"');
        self.emit_bare_word(|ch| {
            (ch as u32) < 128
                && matches!(
                    ch,
                    ' ' | '\t' | '\r' | '\n' | ':' | '{' | '}' | '[' | ']' | ',' | '"' | '\'' | '/'
                )
                || ch == '\u{200b}'
        });
        self.emit_char('"');
        if self.i < self.n && self.cur() == '"' {
            self.i += 1;
        }
    }

    /// Parse an unquoted bare-word value, wrapping it in double quotes.
    pub(super) fn parse_unquoted_value(&mut self) {
        self.emit_char('"');
        self.emit_bare_word(|ch| matches!(ch, ',' | '}' | ']'));
        self.emit_char('"');
        self.just_emitted_value = true;
    }
}

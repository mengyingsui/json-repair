//! String parsing with embedded-quote detection and escape handling.

use std::fmt::Write;

use super::{ParserState, Repairer};

/// Control characters (U+0000–U+001F) must be escaped in JSON strings.
const CONTROL_CHAR_MAX: u32 = 0x20;
/// Start of the UTF-16 surrogate range; lone surrogates are invalid in JSON.
const SURROGATE_LO: u32 = 0xD800;
/// End of the UTF-16 surrogate range.
const SURROGATE_HI: u32 = 0xDFFF;

/// Whether `ch` is one of the valid JSON escape characters after `\`.
#[inline]
fn is_valid_escape(ch: char) -> bool {
    matches!(ch, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't')
}

impl Repairer {
    /// Emit the escaped form of `ch` after a `\` was consumed.
    ///
    /// Valid JSON escapes are preserved; `\uXXXX` is validated (lone
    /// surrogates become `\ufffd`); control chars become `\uXXXX`; anything
    /// else is emitted as `\\` + the literal char.
    pub(super) fn emit_escape(&mut self, ch: char) {
        if is_valid_escape(ch) {
            self.emit_char('\\');
            self.emit_char(ch);
        } else if ch == 'u'
            && self.i + 5 <= self.n
            && self.text.as_bytes()[self.i + 1..self.i + 5]
                .iter()
                .all(|b| b.is_ascii_hexdigit())
        {
            let mut hex_val: u32 = 0;
            for k in 1..=4 {
                hex_val = (hex_val << 4) | self.char_at(self.i + k).to_digit(16).unwrap_or(0);
            }
            if (SURROGATE_LO..=SURROGATE_HI).contains(&hex_val) {
                let _ = write!(self.out, "\\ufffd");
                self.i += 4;
            } else {
                self.emit_char('\\');
                self.emit_char('u');
            }
        } else if (ch as u32) < CONTROL_CHAR_MAX {
            self.emit_unicode_escape(ch as u32);
        } else {
            self.emit_str("\\\\");
            self.emit_char(ch);
        }
    }

    /// Handle the `InStringEscaped` state for a single char.
    ///
    /// `single_quote_escape` controls whether `\'` is emitted literally (for
    /// single-quoted strings) or as a normal escape (for double/triple-quoted).
    #[inline]
    fn handle_escaped(&mut self, ch: char, single_quote_escape: bool) {
        if single_quote_escape && ch == '\'' {
            self.emit_char('\'');
        } else {
            self.emit_escape(ch);
        }
        self.state = ParserState::InString;
        self.i += ch.len_utf8();
    }

    /// Emit a single char from an unquoted string (key or value).
    ///
    /// Escapes `\` and `"` as JSON string escapes, control chars as
    /// `\uXXXX`, and passes everything else through.  This is the unquoted
    /// counterpart of `emit_string_body_char` — the difference is that in an
    /// unquoted context `\` is a literal backslash (not an escape introducer),
    /// so there is no `InStringEscaped` state.
    #[inline]
    pub(super) fn emit_unquoted_char(&mut self, ch: char) {
        match ch {
            '\\' => self.emit_str("\\\\"),
            '"' => self.emit_str("\\\""),
            c if (c as u32) < CONTROL_CHAR_MAX => {
                self.emit_unicode_escape(c as u32);
            }
            _ => self.emit_char(ch),
        }
    }

    /// Handle a body char that is NOT a string delimiter (`"` / `'`).
    ///
    /// Covers: backslash state transitions, `\n\r\t` escapes, `<0x20` control
    /// escapes, and plain passthrough.
    #[inline]
    fn emit_string_body_char(&mut self, ch: char, single_quote_escape: bool) {
        if self.state == ParserState::InString && ch == '\\' {
            self.state = ParserState::InStringEscaped;
            self.i += 1;
            return;
        }
        if self.state == ParserState::InStringEscaped {
            self.handle_escaped(ch, single_quote_escape);
            return;
        }
        match ch {
            '\n' => {
                self.out.push_str("\\n");
                self.i += 1;
            }
            '\r' => {
                self.out.push_str("\\r");
                self.i += 1;
            }
            '\t' => {
                self.out.push_str("\\t");
                self.i += 1;
            }
            c if (c as u32) < CONTROL_CHAR_MAX => {
                self.emit_unicode_escape(c as u32);
                self.i += 1;
            }
            _ => {
                self.emit_char(ch);
                self.i += ch.len_utf8();
            }
        }
    }

    /// Check whether the `"` at `self.i` is a real string terminator.
    ///
    /// May have side effects: caches the bareword lookahead position in
    /// `self.lookahead_pos` so `parse_string` can reuse it.
    ///
    /// Looks ahead past optional whitespace and returns `true` when the next
    /// char is one of:
    /// - `,` `}` `]` `\n` — structural punctuation (with sub-checks for `,`
    ///   and the embedded-quote guard, see below)
    /// - `"` — an immediately following quote (empty next value or `""…"""`)
    /// - `:` `{` `[` — but only when `expect_key` is set (object key context)
    /// - A bare word followed by `"` then `:` — unquoted key detection
    ///
    /// **Embedded-quote guard** (for `]`/`}` only): a `"` followed by brackets
    /// that cannot be a real container-closer is treated as an unescaped quote
    /// inside the string value, not a terminator.
    pub(super) fn check_closing_quote(&mut self) -> bool {
        let mut j = self.i + 1;
        while j < self.n && matches!(self.char_at(j), ' ' | '\t' | '\r') {
            j += 1;
        }
        if j >= self.n {
            return true;
        }
        let nc = self.char_at(j);
        if matches!(nc, ',' | '}' | ']' | '\n') {
            if nc == ',' && !self.expect_key {
                let k = self.skip_ws_at(j + 1);
                if k < self.n {
                    let after = self.char_at(k);
                    if !matches!(
                        after,
                        '"' | '{' | '[' | 't' | 'f' | 'n' | '-' | '}' | ']' | ','
                    ) && !after.is_ascii_digit()
                    {
                        return false;
                    }
                }
            }
            if (nc == ']' || nc == '}') && self.is_embedded_bracket_quote(j, nc) {
                return false;
            }
            return true;
        }
        if nc == '"' {
            return true;
        }
        if self.expect_key && matches!(nc, ':' | '{' | '[') {
            return true;
        }
        if nc.is_ascii_alphabetic() || nc == '_' {
            let mut k = j;
            loop {
                let ch = self.char_at(k);
                if !(ch.is_alphanumeric() || ch == '_') {
                    break;
                }
                k += ch.len_utf8();
            }
            k = self.skip_ws_at(k);
            if k < self.n && self.char_at(k) == '"' {
                self.lookahead_pos = Some(k);
                k += 1;
                k = self.skip_ws_at(k);
                if k < self.n && self.char_at(k) == ':' {
                    return true;
                }
            }
        }
        false
    }

    /// Check whether a `"` followed by bracket `nc` (`]` or `}`) at
    /// position `j` is an embedded quote inside the string value rather
    /// than a real string terminator.
    ///
    /// After the bracket run, a "structural" continuation is `,`, `}`,
    /// `]`, or EOF — anything else means bare content follows, so the
    /// bracket is a literal character.  Two sub-cases:
    ///
    /// * **Mismatched bracket** (not the innermost open container): it can
    ///   never be a real closer, so the quote is embedded.
    /// * **Matching bracket**: ambiguous — could be a real close + trailing
    ///   junk, or an embedded bracket.  Treated as embedded only when a
    ///   later real terminator (a `"` followed by `,`/`}`/`]`) exists AND
    ///   the current container's closer `nc` reappears after it.
    fn is_embedded_bracket_quote(&self, j: usize, nc: char) -> bool {
        let mut k = j;
        while k < self.n && matches!(self.char_at(k), ']' | '}') {
            k += 1;
        }
        k = self.skip_ws_at(k);
        let structural = k >= self.n || matches!(self.char_at(k), ',' | '}' | ']');
        if structural {
            return false;
        }
        let matches_top = self.brackets_last() == Some(nc);
        if !matches_top {
            return true;
        }
        let open_bracket = if nc == ']' { '[' } else { '{' };
        let mut p = k;
        while p < self.n {
            if self.char_at(p) == '"' {
                let q = self.skip_ws_at(p + 1);
                if q < self.n && matches!(self.char_at(q), ',' | '}' | ']') {
                    let need_colon = nc == '}' && self.char_at(q) == ',';
                    let mut r = q;
                    let mut in_str = false;
                    let mut depth: i32 = 1;
                    let mut saw_colon = false;
                    while r < self.n {
                        let rc = self.char_at(r);
                        if in_str {
                            if rc == '"' {
                                in_str = false;
                            }
                            r += rc.len_utf8();
                        } else if rc == '"' {
                            in_str = true;
                            r += 1;
                        } else if rc == nc {
                            depth -= 1;
                            if depth == 0 {
                                return !need_colon || saw_colon;
                            }
                            r += 1;
                        } else if rc == open_bracket {
                            depth += 1;
                            r += 1;
                        } else if rc == ':' {
                            saw_colon = true;
                            r += 1;
                        } else {
                            r += rc.len_utf8();
                        }
                    }
                    break;
                }
            }
            p += self.char_at(p).len_utf8();
        }
        false
    }

    /// Parse a double-quoted JSON string, handling embedded quotes and escapes.
    pub(super) fn parse_string(&mut self) {
        self.emit_char('"');
        self.state = ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.cur();
            if ch == '"' {
                if self.peek(1) == '"' {
                    self.emit_str("\\\"");
                    self.i += 1;
                    let mut j = self.i + 1;
                    while j < self.n && matches!(self.char_at(j), ' ' | '\t' | '\r') {
                        j += 1;
                    }
                    if j < self.n && matches!(self.char_at(j), ',' | '}' | ']' | ':' | '\n') {
                        continue;
                    } else {
                        self.i += 1;
                        if self.i < self.n && self.cur() == '"' {
                            self.emit_str("\\\"");
                            self.i += 1;
                        }
                        continue;
                    }
                }
                if self.check_closing_quote() {
                    self.emit_char('"');
                    self.state = ParserState::Normal;
                    let nc = self.peek(1);
                    if nc.is_ascii_alphabetic() || nc == '_' {
                        let k = self.lookahead_pos.take().unwrap_or_else(|| {
                            let mut k = self.i + 1;
                            loop {
                                let kc = self.char_at(k);
                                if !(kc.is_alphanumeric() || kc == '_') {
                                    break;
                                }
                                k += kc.len_utf8();
                            }
                            self.skip_ws_at(k)
                        });
                        if k < self.n && self.char_at(k) == '"' {
                            let _ = self.out.pop();
                            let trimmed = self
                                .out
                                .trim_end_matches(|c: char| c.is_ascii_whitespace())
                                .len();
                            if trimmed < self.out.len() {
                                self.out.truncate(trimmed);
                            }
                            self.trim_trailing_comma();
                            self.emit_char('"');
                            self.state = ParserState::Normal;
                            return;
                        }
                    }
                    self.i += 1;
                    debug_assert!(
                        self.out.ends_with('"'),
                        "parse_string: output missing closing quote"
                    );
                    if !self.out.ends_with('"') {
                        self.emit_char('"');
                    }
                    return;
                } else {
                    self.emit_str("\\\"");
                    self.i += 1;
                    continue;
                }
            }
            self.emit_string_body_char(ch, false);
            continue;
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
    }

    /// Parse a triple-quoted (`"""…"""`) string into a normal JSON string.
    pub(super) fn parse_triple_string(&mut self) {
        self.i += 3;
        self.emit_char('"');
        self.state = ParserState::InString;
        while self.i < self.n {
            if self.peek_is("\"\"\"") {
                let after = self.i + 3;
                if !(after < self.n && self.char_at(after) == '"') {
                    self.i += 3;
                    self.emit_char('"');
                    self.state = ParserState::Normal;
                    self.just_emitted_value = true;
                    return;
                }
            }
            let ch = self.cur();
            if ch == '"' {
                self.emit_str("\\\"");
                self.i += 1;
                continue;
            }
            self.emit_string_body_char(ch, false);
            continue;
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
        if !self.out.ends_with('"') {
            self.emit_char('"');
        }
    }

    /// Parse a single-quoted (`'…'`) string into a double-quoted JSON string.
    pub(super) fn parse_single_quoted_string(&mut self) {
        self.emit_char('"');
        self.state = ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.cur();
            if ch == '\'' {
                let mut j = self.i + 1;
                while j < self.n && matches!(self.char_at(j), ' ' | '\t' | '\r') {
                    j += 1;
                }
                if j >= self.n || matches!(self.char_at(j), ',' | '}' | ']' | ':' | '\n') {
                    self.emit_char('"');
                    self.state = ParserState::Normal;
                    self.i += 1;
                    self.just_emitted_value = true;
                    return;
                } else {
                    self.emit_char('\'');
                    self.i += 1;
                    continue;
                }
            }
            if ch == '"' {
                self.emit_str("\\\"");
                self.i += 1;
                continue;
            }
            self.emit_string_body_char(ch, true);
            continue;
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
        if !self.out.ends_with('"') {
            self.emit_char('"');
        }
    }
}

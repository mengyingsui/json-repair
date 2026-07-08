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

/// Whether the caller should continue the loop after `emit_string_body_char`.
///
/// `Delimiter` is never returned by `emit_string_body_char` — the caller
/// handles `"` / `'` before calling it — but keeping the variant makes the
/// state machine's intent explicit and exhaustive.
#[allow(dead_code)]
enum BodyAction {
    /// The char was fully handled; continue the loop.
    Handled,
    /// The char is a structural delimiter (`"` or `'`) that the caller must
    /// interpret.
    Delimiter,
}

impl Repairer {
    pub(super) fn emit_escape(&mut self, ch: char) {
        if is_valid_escape(ch) {
            self.emit_char('\\');
            self.emit_char(ch);
        } else if ch == 'u'
            && self.peek(1).is_ascii_hexdigit()
            && self.peek(2).is_ascii_hexdigit()
            && self.peek(3).is_ascii_hexdigit()
            && self.peek(4).is_ascii_hexdigit()
        {
            let mut hex_val: u32 = 0;
            for k in 1..=4 {
                hex_val = (hex_val << 4) | self.chars[self.i + k].to_digit(16).unwrap_or(0);
            }
            if (SURROGATE_LO..=SURROGATE_HI).contains(&hex_val) {
                let _ = write!(self.out, "\\ufffd");
                self.out_chars += 6;
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
        self.i += 1;
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
    fn emit_string_body_char(&mut self, ch: char, single_quote_escape: bool) -> BodyAction {
        if self.state == ParserState::InString && ch == '\\' {
            self.state = ParserState::InStringEscaped;
            self.i += 1;
            return BodyAction::Handled;
        }
        if self.state == ParserState::InStringEscaped {
            self.handle_escaped(ch, single_quote_escape);
            return BodyAction::Handled;
        }
        match ch {
            '\n' => {
                self.emit_str("\\n");
                self.i += 1;
                BodyAction::Handled
            }
            '\r' => {
                self.emit_str("\\r");
                self.i += 1;
                BodyAction::Handled
            }
            '\t' => {
                self.emit_str("\\t");
                self.i += 1;
                BodyAction::Handled
            }
            c if (c as u32) < CONTROL_CHAR_MAX => {
                self.emit_unicode_escape(c as u32);
                self.i += 1;
                BodyAction::Handled
            }
            _ => {
                self.emit_char(ch);
                self.i += 1;
                BodyAction::Handled
            }
        }
    }

    pub(super) fn is_closing_quote(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.n && matches!(self.chars[j], ' ' | '\t' | '\r') {
            j += 1;
        }
        if j >= self.n {
            return true;
        }
        let nc = self.chars[j];
        if matches!(nc, ',' | '}' | ']' | '\n') {
            if nc == ',' && !self.expect_key {
                let mut k = j + 1;
                while k < self.n && self.chars[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < self.n {
                    let after = self.chars[k];
                    if !matches!(
                        after,
                        '"' | '{' | '[' | 't' | 'f' | 'n' | '-' | '}' | ']' | ','
                    ) && !after.is_ascii_digit()
                    {
                        return false;
                    }
                }
            }
            // Embedded-quote guard for `]`/`}`: a `"` followed by a bracket that
            // cannot be a real container-closer is an unescaped quote inside the
            // string value, not a terminator.
            //
            // After the bracket run, a "structural" continuation is `,`, `}`,
            // `]`, or EOF -- anything else means bare content follows, so the
            // bracket is a literal character. Two sub-cases:
            //  * Mismatched bracket (not the innermost open container): it can
            //    never be a real closer, so the quote is embedded.
            //  * Matching bracket: ambiguous -- could be a real close + trailing
            //    junk, or an embedded bracket. Treat as embedded only when a
            //    later real terminator (a `"` followed by `,`/`}`/`]`) exists AND
            //    the current container's closer `nc` reappears after it; this
            //    rejects trailing junk like `["a","b"] "c"` while still repairing
            //    `["He said "]boom" loudly", "d"]`.
            if nc == ']' || nc == '}' {
                let mut k = j;
                while k < self.n && matches!(self.chars[k], ']' | '}') {
                    k += 1;
                }
                while k < self.n && self.chars[k].is_ascii_whitespace() {
                    k += 1;
                }
                let structural = k >= self.n || matches!(self.chars[k], ',' | '}' | ']');
                if !structural {
                    let matches_top = self.brackets.last().copied() == Some(nc);
                    if !matches_top {
                        return false;
                    }
                    let mut p = k;
                    let mut embedded = false;
                    while p < self.n {
                        if self.chars[p] == '"' {
                            let mut q = p + 1;
                            while q < self.n && self.chars[q].is_ascii_whitespace() {
                                q += 1;
                            }
                            if q < self.n && matches!(self.chars[q], ',' | '}' | ']') {
                                // Verify the current container genuinely closes
                                // after this terminator. Use a string-aware
                                // bracket balance so brackets inside quoted junk
                                // (e.g. the `}` in `"d}`) don't masquerade as a
                                // close. For objects whose terminator is followed
                                // by `,` (more elements expected), also require an
                                // out-of-string `:` (a key separator) before the
                                // close, so mimicking junk like
                                // `{"a":"b"} "c", "d"}` is rejected.
                                let open_bracket = if nc == ']' { '[' } else { '{' };
                                let need_colon = nc == '}' && self.chars[q] == ',';
                                let mut r = q;
                                let mut in_str = false;
                                let mut depth: i32 = 1;
                                let mut saw_colon = false;
                                while r < self.n {
                                    let rc = self.chars[r];
                                    if in_str {
                                        if rc == '"' {
                                            in_str = false;
                                        }
                                    } else if rc == '"' {
                                        in_str = true;
                                    } else if rc == nc {
                                        depth -= 1;
                                        if depth == 0 {
                                            if !need_colon || saw_colon {
                                                embedded = true;
                                            }
                                            break;
                                        }
                                    } else if rc == open_bracket {
                                        depth += 1;
                                    } else if rc == ':' {
                                        saw_colon = true;
                                    }
                                    r += 1;
                                }
                                break;
                            }
                        }
                        p += 1;
                    }
                    if embedded {
                        return false;
                    }
                }
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
            while k < self.n && (self.chars[k].is_alphanumeric() || self.chars[k] == '_') {
                k += 1;
            }
            while k < self.n && self.chars[k].is_ascii_whitespace() {
                k += 1;
            }
            if k < self.n && self.chars[k] == '"' {
                k += 1;
                while k < self.n && self.chars[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < self.n && self.chars[k] == ':' {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn parse_string(&mut self) {
        self.emit_char('"');
        self.state = ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            if ch == '"' {
                if self.peek(1) == '"' {
                    self.emit_str("\\\"");
                    self.i += 1;
                    let mut j = self.i + 1;
                    while j < self.n && matches!(self.chars[j], ' ' | '\t' | '\r') {
                        j += 1;
                    }
                    if j < self.n && matches!(self.chars[j], ',' | '}' | ']' | ':' | '\n') {
                        continue;
                    } else {
                        self.i += 1;
                        if self.i < self.n && self.chars[self.i] == '"' {
                            self.emit_str("\\\"");
                            self.i += 1;
                        }
                        continue;
                    }
                }
                if self.is_closing_quote() {
                    self.emit_char('"');
                    self.state = ParserState::Normal;
                    let nc = self.peek(1);
                    if nc.is_ascii_alphabetic() || nc == '_' {
                        let mut k = self.i + 1;
                        while k < self.n
                            && (self.chars[k].is_alphanumeric() || self.chars[k] == '_')
                        {
                            k += 1;
                        }
                        while k < self.n && self.chars[k].is_ascii_whitespace() {
                            k += 1;
                        }
                        if k < self.n && self.chars[k] == '"' {
                            let _ = self.out.pop();
                            self.out_chars -= 1;
                            while let Some(c) = self.out.pop() {
                                self.out_chars -= c.len_utf8();
                                if !c.is_ascii_whitespace() {
                                    self.out.push(c);
                                    self.out_chars += c.len_utf8();
                                    break;
                                }
                            }
                            self.trim_trailing_comma();
                            self.emit_char('"');
                            debug_assert!(
                                matches!(self.state, ParserState::Normal),
                                "parse_string: state != Normal after early-return"
                            );
                            return;
                        }
                    }
                    self.i += 1;
                    debug_assert!(
                        self.out.ends_with('"'),
                        "parse_string: output missing closing quote"
                    );
                    return;
                } else {
                    self.emit_str("\\\"");
                    self.i += 1;
                    continue;
                }
            }
            match self.emit_string_body_char(ch, false) {
                BodyAction::Delimiter => {}
                BodyAction::Handled => continue,
            }
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
    }

    pub(super) fn parse_triple_string(&mut self) {
        self.i += 3;
        self.emit_char('"');
        self.state = ParserState::InString;
        while self.i < self.n {
            if self.peek_is("\"\"\"") {
                let after = self.i + 3;
                if !(after < self.n && self.chars[after] == '"') {
                    self.i += 3;
                    self.emit_char('"');
                    self.state = ParserState::Normal;
                    self.just_emitted_value = true;
                    return;
                }
            }
            let ch = self.chars[self.i];
            if ch == '"' {
                self.emit_str("\\\"");
                self.i += 1;
                continue;
            }
            match self.emit_string_body_char(ch, false) {
                BodyAction::Delimiter => {}
                BodyAction::Handled => continue,
            }
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
        debug_assert!(
            self.out.ends_with('"'),
            "parse_triple_string: output missing closing quote"
        );
    }

    pub(super) fn parse_single_quoted_string(&mut self) {
        self.emit_char('"');
        self.state = ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            if ch == '\'' {
                let mut j = self.i + 1;
                while j < self.n && matches!(self.chars[j], ' ' | '\t' | '\r') {
                    j += 1;
                }
                if j >= self.n || matches!(self.chars[j], ',' | '}' | ']' | ':' | '\n') {
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
            match self.emit_string_body_char(ch, true) {
                BodyAction::Delimiter => {}
                BodyAction::Handled => continue,
            }
        }
        self.state = ParserState::Normal;
        self.emit_char('"');
        debug_assert!(
            self.out.ends_with('"'),
            "parse_single_quoted_string: output missing closing quote"
        );
    }
}

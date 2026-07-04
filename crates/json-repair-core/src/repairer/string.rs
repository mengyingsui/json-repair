use std::fmt::Write;

use super::Repairer;

const VALID_ESCAPES: &str = r#""\/bfnrt"#;

impl Repairer {
    pub(super) fn emit_escape(&mut self, ch: char) {
        if VALID_ESCAPES.contains(ch) {
            self.emit_char('\\');
            self.emit_char(ch);
        } else if ch == 'u' && self.peek(1).is_ascii_hexdigit() {
            self.emit_char('\\');
            self.emit_char('u');
        } else {
            self.emit_str("\\\\");
            self.emit_char(ch);
        }
    }

    pub(super) fn is_closing_quote(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.n
            && (self.chars[j] == ' ' || self.chars[j] == '\t' || self.chars[j] == '\r')
        {
            j += 1;
        }
        if j >= self.n {
            return true;
        }
        let nc = self.chars[j];
        const CLOSING_CHARS: &str = ",}]\n";
        if CLOSING_CHARS.contains(nc) {
            if nc == ',' && !self.expect_key {
                let mut k = j + 1;
                while k < self.n
                    && (self.chars[k] == ' '
                        || self.chars[k] == '\t'
                        || self.chars[k] == '\r'
                        || self.chars[k] == '\n')
                {
                    k += 1;
                }
                if k < self.n {
                    let after = self.chars[k];
                    if after != '"'
                        && after != '{'
                        && after != '['
                        && after != 't'
                        && after != 'f'
                        && after != 'n'
                        && after != '-'
                        && after != '}'
                        && after != ']'
                        && after != ','
                        && !after.is_ascii_digit()
                    {
                        return false;
                    }
                }
            }
            return true;
        }
        if nc == '"' {
            return true;
        }
        if self.expect_key && nc == ':' {
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
        self.state = crate::repairer::ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            match self.state {
                crate::repairer::ParserState::InStringEscaped => {
                    self.emit_escape(ch);
                    self.state = crate::repairer::ParserState::InString;
                    self.i += 1;
                    continue;
                }
                crate::repairer::ParserState::InString if ch == '\\' => {
                    self.state = crate::repairer::ParserState::InStringEscaped;
                    self.i += 1;
                    continue;
                }
                _ => {}
            }
            if ch == '"' {
                if self.peek(1) == '"' {
                    self.emit_str("\\\"");
                    self.i += 1;
                    let mut j = self.i + 1;
                    while j < self.n
                        && (self.chars[j] == ' '
                            || self.chars[j] == '\t'
                            || self.chars[j] == '\r')
                    {
                        j += 1;
                    }
                    if j < self.n && ",\u{7d}\u{5d}:\n".contains(self.chars[j]) {
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
                    self.state = crate::repairer::ParserState::Normal;
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
                            if self.out.ends_with(',') {
                                self.out.pop();
                                self.out_chars -= 1;
                            }
                            self.emit_char('"');
                            debug_assert!(
                                matches!(self.state, crate::repairer::ParserState::Normal),
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
            if ch == '\n' {
                self.emit_str("\\n");
                self.i += 1;
                continue;
            }
            if ch == '\r' {
                self.emit_str("\\r");
                self.i += 1;
                continue;
            }
            if ch == '\t' {
                self.emit_str("\\t");
                self.i += 1;
                continue;
            }
            if (ch as u32) < 0x20 {
                let _ = write!(self.out, "\\u{:04x}", ch as u32);
                self.out_chars += 6;
                self.i += 1;
                continue;
            }
            self.emit_char(ch);
            self.i += 1;
        }
        self.state = crate::repairer::ParserState::Normal;
        if !self.out.ends_with('"') {
            self.emit_char('"');
        }
        debug_assert!(
            self.out.ends_with('"'),
            "parse_string: output missing closing quote at eof"
        );
    }

    pub(super) fn parse_triple_string(&mut self) {
        self.i += 3;
        self.emit_char('"');
        self.state = crate::repairer::ParserState::InString;
        while self.i < self.n {
            if self.peek_is("\"\"\"") {
                let after = self.i + 3;
                if after < self.n && self.chars[after] == '"' {
                } else {
                    self.i += 3;
                    self.emit_char('"');
                    self.state = crate::repairer::ParserState::Normal;
                    self.just_emitted_value = true;
                    return;
                }
            }
            let ch = self.chars[self.i];
            match self.state {
                crate::repairer::ParserState::InStringEscaped => {
                    self.emit_escape(ch);
                    self.state = crate::repairer::ParserState::InString;
                    self.i += 1;
                    continue;
                }
                crate::repairer::ParserState::InString if ch == '\\' => {
                    self.state = crate::repairer::ParserState::InStringEscaped;
                    self.i += 1;
                    continue;
                }
                _ => {}
            }
            if ch == '"' {
                self.emit_str("\\\"");
                self.i += 1;
                continue;
            }
            if ch == '\n' {
                self.emit_str("\\n");
                self.i += 1;
                continue;
            }
            if ch == '\r' {
                self.emit_str("\\r");
                self.i += 1;
                continue;
            }
            if ch == '\t' {
                self.emit_str("\\t");
                self.i += 1;
                continue;
            }
            if (ch as u32) < 0x20 {
                let _ = write!(self.out, "\\u{:04x}", ch as u32);
                self.out_chars += 6;
                self.i += 1;
                continue;
            }
            self.emit_char(ch);
            self.i += 1;
        }
        self.state = crate::repairer::ParserState::Normal;
        self.emit_char('"');
        debug_assert!(
            self.out.ends_with('"'),
            "parse_triple_string: output missing closing quote"
        );
    }

    pub(super) fn parse_single_quoted_string(&mut self) {
        self.emit_char('"');
        self.state = crate::repairer::ParserState::InString;
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            match self.state {
                crate::repairer::ParserState::InStringEscaped => {
                    if ch == '\'' {
                        self.emit_char('\'');
                        self.state = crate::repairer::ParserState::InString;
                        self.i += 1;
                        continue;
                    }
                    self.emit_escape(ch);
                    self.state = crate::repairer::ParserState::InString;
                    self.i += 1;
                    continue;
                }
                crate::repairer::ParserState::InString if ch == '\\' => {
                    self.state = crate::repairer::ParserState::InStringEscaped;
                    self.i += 1;
                    continue;
                }
                _ => {}
            }
            if ch == '\'' {
                let mut j = self.i + 1;
                while j < self.n
                    && (self.chars[j] == ' ' || self.chars[j] == '\t' || self.chars[j] == '\r')
                {
                    j += 1;
                }
                if j >= self.n || ",\u{7d}\u{5d}:\n".contains(self.chars[j]) {
                    self.emit_char('"');
                    self.state = crate::repairer::ParserState::Normal;
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
            if ch == '\n' {
                self.emit_str("\\n");
                self.i += 1;
                continue;
            }
            if ch == '\r' {
                self.emit_str("\\r");
                self.i += 1;
                continue;
            }
            if ch == '\t' {
                self.emit_str("\\t");
                self.i += 1;
                continue;
            }
            if (ch as u32) < 0x20 {
                let _ = write!(self.out, "\\u{:04x}", ch as u32);
                self.out_chars += 6;
                self.i += 1;
                continue;
            }
            self.emit_char(ch);
            self.i += 1;
        }
        self.state = crate::repairer::ParserState::Normal;
        self.emit_char('"');
        debug_assert!(
            self.out.ends_with('"'),
            "parse_single_quoted_string: output missing closing quote"
        );
    }
}

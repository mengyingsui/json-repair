use super::Repairer;

impl Repairer {
    pub(super) fn parse_key(&mut self) {
        self.skip_ws();
        if self.i >= self.n {
            return;
        }
        let ch = self.chars[self.i];
        if ch == '"' {
            self.parse_string();
        } else if ch == '\'' {
            self.parse_single_quoted_string();
        } else {
            self.parse_unquoted_key();
        }
    }

    pub(super) fn parse_unquoted_key(&mut self) {
        self.emit_char('"');
        while self.i < self.n {
            let ch = self.chars[self.i];
            if matches!(
                ch,
                ' ' | '\t'
                    | '\r'
                    | '\n'
                    | ':'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | ','
                    | '"'
                    | '\''
                    | '/'
                    | '\u{200b}'
            ) {
                break;
            }
            self.emit_unquoted_char(ch);
            self.i += 1;
        }
        self.emit_char('"');
        if self.i < self.n && self.chars[self.i] == '"' {
            self.i += 1;
        }
    }

    pub(super) fn parse_unquoted_value(&mut self) {
        self.emit_char('"');
        while self.i < self.n {
            let ch = self.chars[self.i];
            if matches!(ch, ',' | '}' | ']') {
                break;
            }
            self.emit_unquoted_char(ch);
            self.i += 1;
        }
        self.emit_char('"');
        self.just_emitted_value = true;
    }
}

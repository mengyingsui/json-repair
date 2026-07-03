use std::fmt::Write;

const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 8192;
const MAX_PARSE_DEPTH: usize = 500;
const VALID_ESCAPES: &str = r#""\/bfnrt"#;

pub(crate) struct Repairer {
    chars: Vec<char>,
    n: usize,
    i: usize,
    out: String,
    brackets: Vec<char>,
    expect_key: bool,
    just_emitted_value: bool,
    out_chars: usize,
    last_depth0_pos: usize,
    depth: usize,
}

impl Repairer {
    pub(crate) fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        Repairer {
            chars,
            n,
            i: 0,
            out: String::with_capacity(n),
            brackets: Vec::new(),
            expect_key: false,
            just_emitted_value: false,
            out_chars: 0,
            last_depth0_pos: 0,
            depth: 0,
        }
    }

    fn peek(&self, offset: usize) -> char {
        let pos = self.i + offset;
        if pos < self.n { self.chars[pos] } else { '\0' }
    }

    fn peek_str(&self, len: usize) -> String {
        let end = (self.i + len).min(self.n);
        self.chars[self.i..end].iter().collect()
    }

    fn emit_char(&mut self, c: char) {
        self.out.push(c);
        self.out_chars += c.len_utf8();
    }

    fn skip_ws(&mut self) {
        while self.i < self.n && self.chars[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn close_brackets(&mut self) {
        while let Some(b) = self.brackets.pop() {
            self.emit_char(b);
        }
        self.last_depth0_pos = self.out_chars;
    }

    fn emit_escape(&mut self, ch: char) {
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

    fn emit_str(&mut self, s: &str) {
        self.out.push_str(s);
        self.out_chars += s.len();
    }

    fn is_closing_quote(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.n && (self.chars[j] == ' ' || self.chars[j] == '\t' || self.chars[j] == '\r')
        {
            j += 1;
        }
        if j >= self.n {
            return true;
        }
        let nc = self.chars[j];
        if ",\u{7d}\u{5d}:\n".contains(nc) {
            return true;
        }
        if nc == '"' {
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
            }
            while k < self.n && self.chars[k].is_ascii_whitespace() {
                k += 1;
            }
            if k < self.n && self.chars[k] == ':' {
                return true;
            }
        }
        false
    }

    fn skip_prefix_junk(&mut self) {
        let mut start = 0;
        while start < self.n && self.chars[start].is_ascii_whitespace() {
            start += 1;
        }
        if start + 2 < self.n
            && self.chars[start] == '`'
            && self.chars[start + 1] == '`'
            && self.chars[start + 2] == '`'
        {
            start += 3;
            while start < self.n && self.chars[start] != '\n' {
                start += 1;
            }
            if start < self.n {
                start += 1;
            }
        }
        let mut text_chars: Vec<char> = self.chars[start..].to_vec();
        let text_n = text_chars.len();
        let saved = self.i;
        let mut unbraced_start: isize = -1;
        self.i = 0;
        loop {
            if self.i >= text_n {
                break;
            }
            let ch = text_chars[self.i];
            if ch == '{' || ch == '[' {
                if unbraced_start != -1 {
                    let wrapped: String = text_chars[unbraced_start as usize..].iter().collect();
                    text_chars = format!("{{{wrapped}").chars().collect();
                    self.chars = text_chars;
                    self.n = self.chars.len();
                    self.i = 0;
                    return;
                }
                break;
            }
            if ch == '"' {
                let str_start = self.i;
                self.i += 1;
                while self.i < text_n {
                    let c = text_chars[self.i];
                    if c == '\\' {
                        self.i += 2;
                    } else if c == '"' {
                        self.i += 1;
                        break;
                    } else {
                        self.i += 1;
                    }
                }
                let mut j = self.i;
                while j < text_n && text_chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < text_n && text_chars[j] == ':' && unbraced_start == -1 {
                    unbraced_start = str_start as isize;
                }
            } else {
                self.i += 1;
            }
        }
        if self.i >= text_n {
            self.i = saved;
        } else {
            self.chars = text_chars;
            self.n = self.chars.len();
        }
    }

    fn skip_suffix_junk(&mut self) {
        if self.last_depth0_pos < self.out.len() {
            let tail = &self.out[self.last_depth0_pos..];
            if tail.trim().is_empty() {
                self.out.truncate(self.last_depth0_pos);
            }
        }
    }

    fn parse_string(&mut self) {
        self.emit_char('"');
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            if ch == '\\' {
                self.i += 1;
                if self.i < self.n {
                    self.emit_escape(self.chars[self.i]);
                    self.i += 1;
                } else {
                    self.emit_str("\\\\");
                }
                continue;
            }
            if ch == '"' {
                if self.peek(1) == '"' {
                    self.emit_str("\\\"");
                    self.i += 1;
                    let mut j = self.i + 1;
                    while j < self.n
                        && (self.chars[j] == ' ' || self.chars[j] == '\t' || self.chars[j] == '\r')
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
                            return;
                        }
                    }
                    self.i += 1;
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
        if !self.out.ends_with('"') {
            self.emit_char('"');
        }
    }

    fn parse_triple_string(&mut self) {
        self.i += 3;
        self.emit_char('"');
        while self.i < self.n {
            if self.peek_str(3) == "\"\"\"" {
                let after = self.i + 3;
                if after < self.n && self.chars[after] == '"' {
                    // pass
                } else {
                    self.i += 3;
                    self.emit_char('"');
                    self.just_emitted_value = true;
                    return;
                }
            }
            let ch = self.chars[self.i];
            if ch == '\\' {
                self.i += 1;
                if self.i < self.n {
                    self.emit_escape(self.chars[self.i]);
                    self.i += 1;
                } else {
                    self.emit_str("\\\\");
                }
                continue;
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
        self.emit_char('"');
    }

    fn parse_single_quoted_string(&mut self) {
        self.emit_char('"');
        self.i += 1;
        while self.i < self.n {
            let ch = self.chars[self.i];
            if ch == '\\' {
                if self.peek(1) == '\'' {
                    self.emit_char('\'');
                    self.i += 2;
                    continue;
                }
                self.i += 1;
                if self.i < self.n {
                    self.emit_escape(self.chars[self.i]);
                    self.i += 1;
                } else {
                    self.emit_str("\\\\");
                }
                continue;
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
        self.emit_char('"');
    }

    fn parse_value(&mut self) {
        self.depth += 1;
        if self.depth > MAX_PARSE_DEPTH {
            self.emit_str("null");
            self.depth -= 1;
            return;
        }
        self.parse_value_inner();
        self.depth -= 1;
    }

    fn parse_value_inner(&mut self) {
        self.skip_ws();
        if self.i >= self.n {
            self.emit_str("null");
            return;
        }
        let ch = self.chars[self.i];
        match ch {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => {
                if self.peek_str(3) == "\"\"\"" {
                    let rest: String = self.chars[self.i + 3..].iter().collect();
                    if rest.contains("\"\"\"") {
                        self.parse_triple_string();
                        return;
                    }
                }
                self.parse_string();
            }
            '\'' => self.parse_single_quoted_string(),
            't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U' => self.parse_literal(),
            '-' => {
                if self.peek_str(2) == "--" {
                    self.skip_comment();
                    self.parse_value();
                } else {
                    self.parse_number();
                }
            }
            '.' | '0'..='9' => self.parse_number(),
            '/' | '#' => {
                self.skip_comment();
                self.parse_value();
            }
            '}' | ']' | ',' => {
                self.emit_str("null");
            }
            _ => {
                if self.expect_key && (ch.is_ascii_alphabetic() || ch == '_') {
                    self.parse_unquoted_key();
                } else if ch.is_ascii_alphabetic() || ch == '_' {
                    self.parse_unquoted_value();
                } else {
                    self.i += 1;
                    self.parse_value();
                }
            }
        }
    }

    fn parse_object(&mut self) {
        self.emit_char('{');
        self.brackets.push('}');
        self.i += 1;
        let prev_expect = self.expect_key;
        self.expect_key = true;
        let mut first = true;
        loop {
            self.skip_ws();
            if self.i >= self.n {
                break;
            }
            let ch = self.chars[self.i];
            if ch == '{' && self.expect_key {
                self.i += 1;
                continue;
            }
            if ch == ':' && self.expect_key {
                self.i += 1;
                continue;
            }
            if ch == '}' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char('}');
                self.brackets.pop();
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.i += 1;
                self.expect_key = prev_expect;
                self.just_emitted_value = true;
                return;
            }
            if ch == ',' {
                if !first && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                self.expect_key = true;
                continue;
            }
            if ch == '/' || ch == '#' || (ch == '-' && self.peek_str(2) == "--") {
                self.skip_comment();
                continue;
            }
            if ch == '"' && self.just_emitted_value {
                let mut j = self.i + 1;
                while j < self.n && self.chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= self.n || "},\u{5d}:".contains(self.chars[j]) {
                    self.i += 1;
                    continue;
                }
            }
            if ch == ']' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char('}');
                self.brackets.pop();
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.expect_key = prev_expect;
                return;
            }
            if self.expect_key {
                if !first
                    && ch != '"'
                    && ch != '_'
                    && ch != '/'
                    && ch != '\''
                    && !ch.is_ascii_alphabetic()
                {
                    break;
                }
                if ch.is_ascii_alphabetic() {
                    let mut j = self.i + 1;
                    while j < self.n && (self.chars[j].is_alphanumeric() || self.chars[j] == '_') {
                        j += 1;
                    }
                    while j < self.n
                        && (self.chars[j] == ' ' || self.chars[j] == '\t' || self.chars[j] == '\r')
                    {
                        j += 1;
                    }
                    if j >= self.n || !",\":".contains(self.chars[j]) {
                        break;
                    }
                }
                if !first
                    && self.just_emitted_value
                    && !self.out.ends_with(',')
                    && !self.out.ends_with('{')
                    && !self.out.ends_with('[')
                {
                    self.emit_char(',');
                }
                self.parse_key();
                self.skip_ws();
                if self.i < self.n && self.chars[self.i] == ':' {
                    self.emit_char(':');
                    self.i += 1;
                } else if self.i < self.n && self.chars[self.i] != ':' {
                    self.emit_char(':');
                }
                self.expect_key = false;
                self.parse_value();
                self.expect_key = true;
                self.just_emitted_value = true;
            } else {
                if !first
                    && ch != '"'
                    && ch != '{'
                    && ch != '['
                    && ch != '\''
                    && ch != 't'
                    && ch != 'f'
                    && ch != 'n'
                    && ch != 'T'
                    && ch != 'F'
                    && ch != 'N'
                    && ch != 'i'
                    && ch != 'I'
                    && ch != 'u'
                    && ch != 'U'
                    && ch != '-'
                    && ch != '.'
                    && !ch.is_ascii_digit()
                {
                    break;
                }
                if !first
                    && self.just_emitted_value
                    && !self.out.ends_with(',')
                    && !self.out.ends_with('{')
                    && !self.out.ends_with('[')
                    && ch != '}'
                    && ch != ']'
                    && ch != ','
                {
                    self.emit_char(',');
                }
                self.parse_value();
                self.just_emitted_value = true;
            }
            first = false;
        }
        self.expect_key = prev_expect;
    }

    fn parse_array(&mut self) {
        self.emit_char('[');
        self.brackets.push(']');
        self.i += 1;
        let mut first = true;
        loop {
            self.skip_ws();
            if self.i >= self.n {
                break;
            }
            let ch = self.chars[self.i];
            if ch == ']' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char(']');
                self.brackets.pop();
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.i += 1;
                self.just_emitted_value = true;
                return;
            }
            if ch == '}' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char(']');
                self.brackets.pop();
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.i += 1;
                self.just_emitted_value = true;
                return;
            }
            if ch == ',' {
                if !first && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                continue;
            }
            if ch == '/' || ch == '#' || (ch == '-' && self.peek_str(2) == "--") {
                self.skip_comment();
                continue;
            }
            if !first
                && self.just_emitted_value
                && !self.out.ends_with(',')
                && !self.out.ends_with('[')
                && !self.out.ends_with(':')
                && ch != ']'
            {
                self.emit_char(',');
            }
            self.parse_value();
            self.just_emitted_value = true;
            first = false;
        }
    }

    fn parse_key(&mut self) {
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

    fn parse_unquoted_key(&mut self) {
        self.emit_char('"');
        while self.i < self.n {
            let ch = self.chars[self.i];
            if " \t\r\n:{}[],\"'/\u{200b}".contains(ch) {
                break;
            }
            self.emit_char(ch);
            self.i += 1;
        }
        self.emit_char('"');
        if self.i < self.n && self.chars[self.i] == '"' {
            self.i += 1;
        }
    }

    fn parse_unquoted_value(&mut self) {
        self.emit_char('"');
        while self.i < self.n {
            let ch = self.chars[self.i];
            if ",\u{7d}\u{5d}".contains(ch) {
                break;
            }
            if ch == '\\' {
                self.emit_str("\\\\");
            } else if ch == '"' {
                self.emit_str("\\\"");
            } else if (ch as u32) < 0x20 {
                let _ = write!(self.out, "\\u{:04x}", ch as u32);
                self.out_chars += 6;
            } else {
                self.emit_char(ch);
            }
            self.i += 1;
        }
        self.emit_char('"');
        self.just_emitted_value = true;
    }

    fn parse_literal(&mut self) {
        let end = (self.i + 9).min(self.n);
        let lower: String = self.chars[self.i..end]
            .iter()
            .collect::<String>()
            .to_lowercase();
        if lower.starts_with("true") {
            self.emit_str("true");
            self.i += 4;
        } else if lower.starts_with("false") {
            self.emit_str("false");
            self.i += 5;
        } else if lower.starts_with("null") || lower.starts_with("none") {
            self.emit_str("null");
            self.i += 4;
        } else if lower.starts_with("undefined") {
            self.emit_str("null");
            self.i += 9;
        } else if lower.starts_with("nan") {
            self.emit_str("null");
            self.i += 3;
        } else if lower.starts_with("infinity") || lower.starts_with("+infinity") {
            self.emit_str("null");
            self.i += 8;
        } else if lower.starts_with("-infinity") {
            self.emit_str("null");
            self.i += 9;
        } else {
            self.parse_unquoted_value();
            return;
        }
        self.just_emitted_value = true;
    }

    fn parse_number(&mut self) {
        let start = self.i;
        while self.i < self.n && "-0123456789.eE+".contains(self.chars[self.i]) {
            self.i += 1;
        }
        let num_str: String = self.chars[start..self.i].iter().collect();
        let num_str = if num_str.starts_with("+.") {
            format!("0{}", &num_str[1..])
        } else if let Some(stripped) = num_str.strip_prefix('+') {
            stripped.to_string()
        } else if num_str.starts_with("-.") {
            format!("-0{}", &num_str[1..])
        } else if num_str.starts_with('.') {
            format!("0{}", num_str)
        } else {
            num_str
        };
        let num_str = if num_str.ends_with('.') {
            format!("{}0", num_str)
        } else {
            num_str
        };
        if num_str.parse::<f64>().is_ok() {
            self.emit_str(&num_str);
        } else {
            self.emit_char('0');
        }
        self.just_emitted_value = true;
    }

    fn skip_comment(&mut self) {
        if self.peek_str(2) == "//" {
            while self.i < self.n && self.chars[self.i] != '\n' {
                self.i += 1;
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else if self.peek_str(2) == "/*" {
            self.i += 2;
            while self.i + 1 < self.n {
                if self.chars[self.i] == '*' && self.chars[self.i + 1] == '/' {
                    self.i += 2;
                    return;
                }
                self.i += 1;
            }
        } else if self.chars[self.i] == '#' || self.peek_str(2) == "--" {
            while self.i < self.n && self.chars[self.i] != '\n' {
                self.i += 1;
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else {
            self.i += 1;
        }
    }

    fn is_implicit_object_sequence(&self) -> bool {
        if self.i >= self.n || self.chars[self.i] != '{' {
            return false;
        }
        let remaining = self.n - self.i;
        if remaining < IMPLICIT_SEQUENCE_MIN_LENGTH {
            return false;
        }
        let mut j = self.i;
        let mut count = 0;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut esc = false;
        while j + 1 < self.n {
            let ch = self.chars[j];
            if esc {
                esc = false;
                j += 1;
                continue;
            }
            if ch == '\\' {
                esc = true;
                j += 1;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                j += 1;
                continue;
            }
            if in_string {
                j += 1;
                continue;
            }
            if ch == '{' || ch == '[' {
                depth += 1;
                j += 1;
                continue;
            }
            if ch == '}' || ch == ']' {
                depth = depth.saturating_sub(1);
            }
            if ch == '}' && depth == 0 {
                let mut k = j + 1;
                if k < self.n && self.chars[k] == ',' {
                    k += 1;
                }
                while k < self.n && self.chars[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < self.n && self.chars[k] == '{' {
                    count += 1;
                    if count >= 3 {
                        return true;
                    }
                    j = k;
                    continue;
                }
            }
            j += 1;
        }
        false
    }

    fn parse_implicit_array(&mut self) {
        self.emit_char('[');
        let mut first = true;
        loop {
            self.skip_ws();
            if self.i >= self.n {
                break;
            }
            if self.chars[self.i] != '{' {
                break;
            }
            if !first {
                self.emit_char(',');
            }
            self.parse_value();
            first = false;
            self.skip_ws();
            if self.i < self.n && self.chars[self.i] == ',' {
                self.i += 1;
            }
        }
        if !first && self.out.ends_with(',') {
            self.out.pop();
            self.out_chars -= 1;
        }
        self.emit_char(']');
    }

    pub(crate) fn repair(&mut self) -> String {
        self.skip_prefix_junk();
        if self.i >= self.n {
            return String::new();
        }
        if self.is_implicit_object_sequence() {
            self.parse_implicit_array();
        } else {
            self.parse_value();
        }
        self.close_brackets();
        self.skip_suffix_junk();
        std::mem::take(&mut self.out)
    }
}

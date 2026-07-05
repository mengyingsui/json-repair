use super::Repairer;

const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 8192;

impl Repairer {
    pub(super) fn skip_prefix_junk(&mut self) {
        let mut start = 0;
        while start < self.n && self.chars[start].is_ascii_whitespace() {
            start += 1;
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
            if ch == '`'
                && self.i + 2 < text_n
                && text_chars[self.i + 1] == '`'
                && text_chars[self.i + 2] == '`'
            {
                self.i += 3;
                let lang_start = self.i;
                while self.i < text_n && text_chars[self.i] != '\n' {
                    self.i += 1;
                }
                let lang: String = text_chars[lang_start..self.i].iter().collect();
                let lang_trimmed = lang.trim();
                let is_json_fence = lang_trimmed.is_empty() || lang_trimmed == "json";
                if self.i < text_n {
                    self.i += 1;
                }
                if !is_json_fence {
                    let mut code_depth = 1u32;
                    while self.i < text_n && code_depth > 0 {
                        if self.i + 2 < text_n
                            && text_chars[self.i] == '`'
                            && text_chars[self.i + 1] == '`'
                            && text_chars[self.i + 2] == '`'
                        {
                            self.i += 3;
                            code_depth -= 1;
                        } else {
                            self.i += 1;
                        }
                    }
                }
                continue;
            }
            if ch == '{' || ch == '[' {
                if ch == '[' {
                    let mut depth = 1i32;
                    let mut j = self.i + 1;
                    let mut is_metatag = j < text_n;
                    while j < text_n && depth > 0 {
                        match text_chars[j] {
                            '[' => depth += 1,
                            ']' => depth -= 1,
                            '{' | '"' => is_metatag = false,
                            _ => {}
                        }
                        j += 1;
                    }
                    if depth == 0 && is_metatag && j - self.i <= 64 {
                        let inner: String = text_chars[self.i + 1..j - 1].iter().collect();
                        if inner
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        {
                            self.i = j;
                            continue;
                        }
                    }
                    if j < text_n && text_chars[j] == '(' {
                        let mut link_depth = 1i32;
                        let mut k = j + 1;
                        while k < text_n && link_depth > 0 {
                            if text_chars[k] == '(' {
                                link_depth += 1;
                            }
                            if text_chars[k] == ')' {
                                link_depth -= 1;
                            }
                            k += 1;
                        }
                        self.i = k;
                        continue;
                    }
                }
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
            debug_assert!(
                self.i < self.n && (self.chars[self.i] == '{' || self.chars[self.i] == '['),
                "skip_prefix_junk: position does not point at JSON container start"
            );
        }
    }

    pub(super) fn skip_suffix_junk(&mut self) {
        debug_assert!(
            self.last_depth0_pos <= self.out.len(),
            "skip_suffix_junk: last_depth0_pos exceeds output length"
        );
        if self.last_depth0_pos < self.out.len() {
            let tail = &self.out[self.last_depth0_pos..];
            if tail.trim().is_empty() {
                self.out.truncate(self.last_depth0_pos);
            }
        }
    }

    pub(super) fn is_implicit_object_sequence(&self) -> bool {
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
}

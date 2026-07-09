//! Suffix junk, implicit object sequences, and trailing-comma trimming.

use super::Repairer;

/// Minimum input length to even consider the implicit-object-sequence fast path.
const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 8192;
/// Maximum length of a `[TEXT_*]`-style metatag to recognise and skip.
const METATAG_MAX_LEN: usize = 64;

impl Repairer {
    /// Skip non-JSON text before the first `{` or `[`.
    ///
    /// Handles markdown code fences, `[TEXT_*]`-style metatags, markdown
    /// link parens, and unbraced `"key": value` patterns.
    ///
    /// On return, `self.i` points at the first `{` or `[` of the JSON body
    /// (or at the original position if no JSON container was found).
    /// For unbraced input, `self.chars` is rewritten to prepend `{` so the
    /// parser treats the bare key as the first object member.
    pub(super) fn skip_prefix_junk(&mut self) {
        let mut start = 0;
        while start < self.n && self.chars[start].is_ascii_whitespace() {
            start += 1;
        }
        let mut i = start;
        let mut unbraced_start: Option<usize> = None;

        while i < self.n {
            let ch = self.chars[i];
            if ch == '`' && i + 2 < self.n && self.chars[i + 1] == '`' && self.chars[i + 2] == '`' {
                i += 3;
                let lang_start = i;
                while i < self.n && self.chars[i] != '\n' {
                    i += 1;
                }
                let lang: String = self.chars[lang_start..i].iter().collect();
                let lang_trimmed = lang.trim();
                let is_json_fence = lang_trimmed.is_empty() || lang_trimmed == "json";
                if i < self.n {
                    i += 1;
                }
                if !is_json_fence {
                    while i < self.n {
                        if i + 2 < self.n
                            && self.chars[i] == '`'
                            && self.chars[i + 1] == '`'
                            && self.chars[i + 2] == '`'
                        {
                            i += 3;
                            break;
                        }
                        i += 1;
                    }
                }
                continue;
            }
            if ch == '{' || ch == '[' {
                if ch == '[' {
                    let mut depth = 1i32;
                    let mut j = i + 1;
                    let mut is_metatag = j < self.n;
                    while j < self.n && depth > 0 {
                        match self.chars[j] {
                            '[' => depth += 1,
                            ']' => depth -= 1,
                            '{' | '"' => is_metatag = false,
                            _ => {}
                        }
                        j += 1;
                    }
                    if depth == 0 && is_metatag && j - i <= METATAG_MAX_LEN {
                        let inner: String = self.chars[i + 1..j - 1].iter().collect();
                        if inner
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        {
                            i = j;
                            continue;
                        }
                    }
                    if j < self.n && self.chars[j] == '(' {
                        let mut k = j + 1;
                        let mut link_depth = 1i32;
                        while k < self.n && link_depth > 0 {
                            if self.chars[k] == '(' {
                                link_depth += 1;
                            }
                            if self.chars[k] == ')' {
                                link_depth -= 1;
                            }
                            k += 1;
                        }
                        i = k;
                        continue;
                    }
                }
                if let Some(start_pos) = unbraced_start {
                    let wrapped: String = self.chars[start_pos..].iter().collect();
                    self.chars = format!("{{{wrapped}").chars().collect();
                    self.n = self.chars.len();
                    self.i = 0;
                    return;
                }
                break;
            }
            if ch == '"' {
                let str_start = i;
                i += 1;
                while i < self.n {
                    let c = self.chars[i];
                    if c == '\\' {
                        i += 2;
                    } else if c == '"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                let mut j = i;
                while j < self.n && self.chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < self.n && self.chars[j] == ':' && unbraced_start.is_none() {
                    unbraced_start = Some(str_start);
                }
            } else {
                i += 1;
            }
        }
        if i >= self.n {
            self.i = 0;
        } else {
            self.i = i;
            debug_assert!(
                self.i < self.n && (self.chars[self.i] == '{' || self.chars[self.i] == '['),
                "skip_prefix_junk: position does not point at JSON container start"
            );
        }
    }

    /// Trim trailing whitespace/junk after the last depth-0 position in `out`.
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

    /// Detect a comma-separated sequence of top-level objects (implicit array).
    ///
    /// Returns `true` when the input starts with `{`, is long enough, and
    /// contains at least 3 consecutive `{…}` objects separated by optional
    /// commas — the repairer then wraps them in `[…]`.
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

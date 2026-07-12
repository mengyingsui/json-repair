//! Suffix junk, implicit object sequences, and trailing-comma trimming.

use super::Repairer;

/// Minimum input length to even consider the implicit-object-sequence fast path.
const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 8192;
/// Maximum length of a `[TEXT_*]`-style metatag to recognize and skip.
const METATAG_MAX_LEN: usize = 64;
/// Maximum number of bytes to scan for implicit object sequences before giving up.
const IMPLICIT_SEQUENCE_MAX_SCAN: usize = 65536;
/// Minimum number of consecutive `{…}` objects to treat as an implicit array.
const IMPLICIT_SEQUENCE_MIN_COUNT: usize = 3;

impl Repairer {
    /// Skip non-JSON text before the first `{` or `[`.
    ///
    /// Handles Markdown code fences, `[TEXT_*]`-style metatags, Markdown
    /// link parens, and unbraced `"key": value` patterns.
    ///
    /// On return, `self.i` points at the first `{` or `[` of the JSON body
    /// (or at the original position if no JSON container was found).
    /// For unbraced input, `self.text` is rewritten to prepend `{` so the
    /// parser treats the bare key as the first object member.
    pub(super) fn skip_prefix_junk(&mut self) {
        let start = self.skip_ws_at(0);
        let mut i = start;
        let mut unbraced_start: Option<usize> = None;

        while i < self.n {
            let ch = self.char_at(i);
            if i + 2 < self.n && self.text.as_bytes()[i..].starts_with(b"```") {
                i += 3;
                let lang_start = i;
                while i < self.n && self.char_at(i) != '\n' {
                    i += self.char_at(i).len_utf8();
                }
                let lang = &self.text[lang_start..i];
                let lang_trimmed = lang.trim();
                let is_json_fence = lang_trimmed.is_empty() || lang_trimmed == "json";
                if i < self.n {
                    i += 1;
                }
                if !is_json_fence {
                    while i < self.n {
                        if i + 2 < self.n && self.text.as_bytes()[i..].starts_with(b"```") {
                            i += 3;
                            break;
                        }
                        i += self.char_at(i).len_utf8();
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
                        let jc = self.char_at(j);
                        match jc {
                            '[' => depth += 1,
                            ']' => depth -= 1,
                            '{' | '"' => is_metatag = false,
                            _ => {}
                        }
                        j += jc.len_utf8();
                    }
                    if depth == 0 && is_metatag && j - i <= METATAG_MAX_LEN {
                        let inner = &self.text[i + 1..j - 1];
                        if inner
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                        {
                            i = j;
                            continue;
                        }
                    }
                    if j < self.n && self.char_at(j) == '(' {
                        let mut k = j + 1;
                        let mut link_depth = 1i32;
                        while k < self.n && link_depth > 0 {
                            let kc = self.char_at(k);
                            if kc == '(' {
                                link_depth += 1;
                            }
                            if kc == ')' {
                                link_depth -= 1;
                            }
                            k += kc.len_utf8();
                        }
                        i = k;
                        continue;
                    }
                }
                if let Some(start_pos) = unbraced_start {
                    self.text.insert(start_pos, '{');
                    self.text.push('}');
                    self.n = self.text.len();
                    self.i = start_pos;
                    return;
                }
                break;
            }
            if ch == '"' {
                let str_start = i;
                i += 1;
                while i < self.n {
                    let c = self.char_at(i);
                    if c == '\\' {
                        i += 1;
                        if i < self.n {
                            i += self.char_at(i).len_utf8();
                        }
                    } else if c == '"' {
                        i += 1;
                        break;
                    } else {
                        i += c.len_utf8();
                    }
                }
                let j = self.skip_ws_at(i);
                if j < self.n && self.char_at(j) == ':' && unbraced_start.is_none() {
                    unbraced_start = Some(str_start);
                }
            } else {
                i += ch.len_utf8();
            }
        }
        if i >= self.n {
            self.i = 0;
        } else {
            self.i = i;
            debug_assert!(
                self.i < self.n && (self.cur() == '{' || self.cur() == '['),
                "skip_prefix_junk: position does not point at JSON container start"
            );
        }
    }

    /// Trim trailing whitespace after the last depth-0 position in `out`.
    ///
    /// Only trims when the tail is *all* whitespace (detected via
    /// `tail.trim().is_empty()`).  Non-whitespace trailing junk is left
    /// in place.
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
        if self.i >= self.n || self.cur() != '{' {
            return false;
        }
        let remaining = self.n - self.i;
        if remaining < IMPLICIT_SEQUENCE_MIN_LENGTH {
            return false;
        }
        let scan_end = self.n.min(self.i + IMPLICIT_SEQUENCE_MAX_SCAN);
        let mut j = self.i;
        let mut count = 0;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut esc = false;
        while j + 1 < scan_end {
            let ch = self.char_at(j);
            if esc {
                esc = false;
                j += ch.len_utf8();
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
                j += ch.len_utf8();
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
                if k < self.n && self.char_at(k) == ',' {
                    k += 1;
                }
                k = self.skip_ws_at(k);
                if k < self.n && self.char_at(k) == '{' {
                    count += 1;
                    if count >= IMPLICIT_SEQUENCE_MIN_COUNT {
                        return true;
                    }
                    j = k;
                    continue;
                }
            }
            j += ch.len_utf8();
        }
        false
    }
}

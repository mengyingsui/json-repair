//! Suffix junk, implicit object sequences, and trailing-comma trimming.

use super::Repairer;

/// Minimum input length (bytes) to consider the implicit-object-sequence path.
const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 128;
/// Maximum length of a `[TEXT_*]`-style metatag to recognize and skip.
const METATAG_MAX_LEN: usize = 128;
/// Minimum number of consecutive `{…}` objects to treat as an implicit array.
const IMPLICIT_SEQUENCE_MIN_COUNT: usize = 2;

/// Byte length of a UTF-8 character from its lead byte.
#[inline]
fn utf8_char_len(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

impl Repairer {
    /// Try to skip a Markdown code fence at `pos`.  Returns `Some(new_pos)` on match.
    fn try_skip_code_fence(&self, pos: usize) -> Option<usize> {
        if pos + 2 >= self.n || !self.text.as_bytes()[pos..].starts_with(b"```") {
            return None;
        }
        let mut i = pos + 3;
        let lang_start = i;
        while i < self.n && self.char_at(i) != '\n' {
            i += self.char_at(i).len_utf8();
        }
        let is_json = matches!(self.text[lang_start..i].trim(), "" | "json");
        if i < self.n {
            i += 1;
        }
        if !is_json {
            while i < self.n {
                if i + 2 < self.n && self.text.as_bytes()[i..].starts_with(b"```") {
                    i += 3;
                    break;
                }
                i += self.char_at(i).len_utf8();
            }
        }
        Some(i)
    }

    /// Try to skip a `[TEXT_*]` metatag or `[label](url)` Markdown link at
    /// `bracket_pos` (which must point at `[`).  Returns `Some(new_pos)` on
    /// match.
    fn try_skip_metatag_or_link(&self, bracket_pos: usize) -> Option<usize> {
        debug_assert_eq!(self.char_at(bracket_pos), '[');
        let mut depth = 1i32;
        let mut j = bracket_pos + 1;
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
        if depth == 0 && is_metatag && j - bracket_pos <= METATAG_MAX_LEN {
            let inner = &self.text[bracket_pos + 1..j - 1];
            if !inner.is_empty()
                && inner
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Some(j);
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
            return Some(k);
        }
        None
    }

    /// Scan a `"…"` string starting at `pos`.  Returns the position just
    /// after the closing `"` (or `self.n` for an unterminated string).
    fn scan_string(&self, pos: usize) -> usize {
        debug_assert_eq!(self.char_at(pos), '"');
        let bytes = self.text.as_bytes();
        let mut i = pos + 1;
        while i < self.n {
            let b = bytes[i];
            if b == b'\\' {
                i += 1;
                if i < self.n {
                    let nb = bytes[i];
                    i += if nb.is_ascii() { 1 } else { utf8_char_len(nb) };
                }
            } else if b == b'"' {
                return i + 1;
            } else {
                i += if b.is_ascii() { 1 } else { utf8_char_len(b) };
            }
        }
        i
    }

    /// Skip non-JSON text before the first `{` or `[` and handle
    /// unbraced-key input.
    ///
    /// Handles Markdown code fences, `[TEXT_*]`-style metatags, Markdown
    /// link parens, and unbraced `"key": value` patterns.
    ///
    /// On return, `self.i` points at the first `{` or `[` of the JSON body
    /// (or at the original position if no JSON container was found).
    /// For unbraced input, `self.text` is rewritten to prepend `{` so the
    /// parser treats the bare key as the first object member.
    pub(super) fn normalize_preamble(&mut self) {
        let start = self.skip_ws_at(0);
        let mut i = start;
        let mut unbraced_start: Option<usize> = None;

        while i < self.n {
            if let Some(new_i) = self.try_skip_code_fence(i) {
                i = new_i;
                continue;
            }

            let ch = self.char_at(i);
            if ch == '{' || ch == '[' {
                if ch == '[' {
                    if let Some(new_i) = self.try_skip_metatag_or_link(i) {
                        i = new_i;
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
                let end = self.scan_string(i);
                let j = self.skip_ws_at(end);
                if j < self.n && self.char_at(j) == ':' && unbraced_start.is_none() {
                    unbraced_start = Some(i);
                }
                i = end;
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
                "normalize_preamble: position does not point at JSON container start"
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
    /// Returns `true` when the input starts with `{` and contains at least
    /// 2 consecutive `{…}` objects separated by optional commas — the
    /// repairer then wraps them in `[…]`.
    pub(super) fn is_implicit_object_sequence(&self) -> bool {
        if self.i >= self.n || self.cur() != '{' {
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
            let ch = self.char_at(j);
            if esc {
                esc = false;
                j += ch.len_utf8();
                continue;
            }
            if ch == '\\' {
                esc = true;
                j += ch.len_utf8();
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                j += ch.len_utf8();
                continue;
            }
            if in_string {
                j += ch.len_utf8();
                continue;
            }
            if ch == '{' || ch == '[' {
                depth += 1;
                j += ch.len_utf8();
                continue;
            }
            if ch == '}' || ch == ']' {
                depth = depth.saturating_sub(1);
            }
            if ch == '}' && depth == 0 {
                let mut k = self.skip_ws_at(j + 1);
                if k < self.n && self.char_at(k) == ',' {
                    k = self.skip_ws_at(k + 1);
                }
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

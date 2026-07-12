//! Number parsing and normalization (leading zeros, leading dot, trailing dot).

use super::Repairer;

impl Repairer {
    /// Parse a number from the current position and emit its normalized form.
    ///
    /// Handles leading `+`/`.`, trailing `.`, and strips leading zeros to
    /// conform to JSON number syntax.  Sets `self.error` if non-numeric
    /// characters are encountered.
    pub(super) fn parse_number(&mut self) {
        let start = self.i;
        while self.i < self.n
            && matches!(
                self.char_at(self.i),
                '-' | '0'..='9' | '.' | 'e' | 'E' | '+'
            )
        {
            self.i += 1;
        }
        if self.i < self.n && self.char_at(self.i).is_ascii_alphabetic() {
            self.error = Some(crate::error::JsonRepairError {
                message: "number contains non-numeric characters".into(),
                position: Some(self.i),
            });
            return;
        }
        let raw = &self.text[start..self.i];
        let mut num_str = String::with_capacity(raw.len() + 2);
        if raw.starts_with("+.") {
            num_str.push('0');
            num_str.push_str(&raw[1..]);
        } else if let Some(stripped) = raw.strip_prefix('+') {
            num_str.push_str(stripped);
        } else if raw.starts_with("-.") {
            num_str.push('-');
            num_str.push('0');
            num_str.push_str(&raw[1..]);
        } else if raw.starts_with('.') {
            num_str.push('0');
            num_str.push_str(raw);
        } else {
            num_str.push_str(raw);
        }
        if num_str.ends_with('.') {
            num_str.push('0');
        }
        normalize_leading_zeros_inplace(&mut num_str);
        if validate_number(&num_str) {
            self.emit_str(&num_str);
        } else {
            self.emit_char('0');
        }
        self.just_emitted_value = true;
        debug_assert!(
            self.error.is_none(),
            "parse_number: error set but parse continued"
        );
    }
}

/// Quick check: does `s` have more than one `.` or `e`/`E`?
#[inline]
fn has_excessive_separators(s: &str) -> bool {
    let mut dot = 0u8;
    let mut exp = 0u8;
    for &b in s.as_bytes() {
        match b {
            b'.' => dot += 1,
            b'e' | b'E' => exp += 1,
            _ => {}
        }
        if dot > 1 || exp > 1 {
            return true;
        }
    }
    false
}

/// Validate a number string is valid JSON and not suspiciously long.
#[cfg(feature = "serde-validate")]
fn validate_number(s: &str) -> bool {
    if has_excessive_separators(s) {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

/// Validate a number string without serde_json — accept any f64-parseable string.
#[cfg(not(feature = "serde-validate"))]
fn validate_number(s: &str) -> bool {
    if has_excessive_separators(s) {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Strip leading zeros from the integer part of a number string, in-place.
///
/// JSON (RFC 8259) forbids leading zeros in numbers.  `f64::parse()` accepts
/// them, so the repairer's `parse_number` would emit e.g. `"000"` which is
/// invalid JSON.  This helper normalizes those away while preserving the
/// numeric value.
///
/// ```ignore
/// let mut s = String::from("000");   normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "0");
/// let mut s = String::from("-001");  normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "-1");
/// let mut s = String::from("00.5");  normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "0.5");
/// let mut s = String::from("0");     normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "0");
/// let mut s = String::from("0.5");   normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "0.5");
/// let mut s = String::from("123");   normalize_leading_zeros_inplace(&mut s); assert_eq!(s, "123");
/// ```
fn normalize_leading_zeros_inplace(s: &mut String) {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return;
    }
    let start = if bytes[0] == b'-' || bytes[0] == b'+' {
        1
    } else {
        0
    };
    let int_end = s[start..]
        .find(['.', 'e', 'E'])
        .map(|pos| start + pos)
        .unwrap_or(s.len());
    let int_len = int_end - start;
    if int_len <= 1 || bytes[start] != b'0' {
        return;
    }
    let mut zeros = 0;
    while start + zeros < int_end && bytes[start + zeros] == b'0' {
        zeros += 1;
    }
    let keep = if zeros == int_len { 1 } else { 0 };
    let remove_from = start + keep;
    let remove_to = start + zeros;
    if remove_to > remove_from {
        s.drain(remove_from..remove_to);
    }
}

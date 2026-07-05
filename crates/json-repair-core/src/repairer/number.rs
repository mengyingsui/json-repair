use super::Repairer;

impl Repairer {
    pub(super) fn parse_number(&mut self) {
        let start = self.i;
        while self.i < self.n && "-0123456789.eE+".contains(self.chars[self.i]) {
            self.i += 1;
        }
        if self.i < self.n && self.chars[self.i].is_ascii_alphabetic() {
            self.error = Some(crate::error::JsonRepairError {
                message: "number contains non-numeric characters".into(),
                position: Some(start),
            });
            return;
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
        let num_str = normalize_number_leading_zeros(&num_str);
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

/// Validate a number string is valid JSON and not suspiciously long.
#[cfg(feature = "serde-validate")]
fn validate_number(s: &str) -> bool {
    if s.matches('.').count() > 1 || s.matches('e').count() + s.matches('E').count() > 1 {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

/// Validate a number string without serde_json — accept any f64-parseable string.
#[cfg(not(feature = "serde-validate"))]
fn validate_number(s: &str) -> bool {
    if s.matches('.').count() > 1 || s.matches('e').count() + s.matches('E').count() > 1 {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Strip leading zeros from the integer part of a number string.
///
/// JSON (RFC 8259) forbids leading zeros in numbers.  `f64::parse()` accepts
/// them, so the repairer's `parse_number` would emit e.g. `"000"` which is
/// invalid JSON.  This helper normalises those away while preserving the
/// numeric value.
///
/// ```ignore
/// assert_eq!(normalize_number_leading_zeros("000"),   "0");
/// assert_eq!(normalize_number_leading_zeros("-001"),  "-1");
/// assert_eq!(normalize_number_leading_zeros("00.5"),  "0.5");
/// assert_eq!(normalize_number_leading_zeros("0"),     "0");    // unchanged
/// assert_eq!(normalize_number_leading_zeros("0.5"),   "0.5");  // unchanged
/// assert_eq!(normalize_number_leading_zeros("123"),   "123");  // unchanged
/// ```
fn normalize_number_leading_zeros(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let start = if s.starts_with('-') || s.starts_with('+') {
        1
    } else {
        0
    };
    let int_end = s[start..]
        .find(['.', 'e', 'E'])
        .map(|pos| start + pos)
        .unwrap_or(s.len());

    let int_part = &s[start..int_end];
    if int_part.len() > 1 && int_part.starts_with('0') {
        let stripped = int_part.trim_start_matches('0');
        let normalized = if stripped.is_empty() { "0" } else { stripped };
        let sign_prefix = if start > 0 { &s[..start] } else { "" };
        let rest = &s[int_end..];
        format!("{}{}{}", sign_prefix, normalized, rest)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_leading_zeros() {
        assert_eq!(normalize_number_leading_zeros("000"), "0");
        assert_eq!(normalize_number_leading_zeros("-001"), "-1");
        assert_eq!(normalize_number_leading_zeros("00.5"), "0.5");
        assert_eq!(normalize_number_leading_zeros("0"), "0");
        assert_eq!(normalize_number_leading_zeros("0.5"), "0.5");
        assert_eq!(normalize_number_leading_zeros("123"), "123");
    }
}

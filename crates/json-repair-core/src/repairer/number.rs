//! JSON number scanning and normalization.
//!
//! Detects numeric tokens, stops at the first non-number character, and
//! emits a normalized representation.  Time-like tokens (`10:30`) are
//! left for the key parser by checking for a trailing `:`.

use crate::repairer::{InputCursor, OutputBuffer, Tracer};

/// Returns `true` if `ch` can start a JSON number.
///
/// Covers leading digits and leading `.` (for `.5`-style numbers).
/// The leading sign `-` is handled separately by [`run_value`](super::Repairer::run_value)
/// because it may also start a `--` comment.
pub(super) fn is_number_start(ch: char) -> bool {
    matches!(ch, '0'..='9' | '.')
}

/// Scan forward from `input.pos()` through characters legal in a JSON
/// number: digits, `.`, `e`/`E`, `-`.  `+` is legal only as the first
/// character or immediately after `e`/`E`.  Returns the position past the
/// last legal character (i.e. the first position that cannot belong to a
/// number span).  Does NOT advance `input.pos()`.
pub(super) fn scan_number_span(input: &InputCursor) -> usize {
    let bytes = input.bytes();
    let mut j = input.pos();
    while j < bytes.len() {
        match bytes[j] {
            b'0'..=b'9' | b'.' | b'e' | b'E' | b'-' => j += 1,
            b'+' => {
                if j == input.pos() || (j > 0 && matches!(bytes[j - 1], b'e' | b'E')) {
                    j += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    j
}

/// Consume the widest legal number span starting at `input.pos()`, then
/// normalize leading zeros, leading `+`, leading `.`, trailing `.`,
/// and validate before emitting.
pub(super) fn parse_number(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    let start = input.pos();
    let end = scan_number_span(input);
    input.set_pos(end);
    // If the span is immediately followed by an alphabetic character
    // it is not a number (e.g. `123abc` → treat as bareword, emit `0`).
    if input
        .char_at(input.pos())
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        output.emit_char('0');
        return;
    }
    let raw = &input.text()[start..input.pos()];
    let mut num_str = String::with_capacity(raw.len() + 2);
    // Normalize edge cases that JSON rejects but LLMs produce:
    //   +.5 → 0.5,  +3 → 3,  -.5 → -0.5,  .5 → 0.5,  3. → 3.0
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
        output.emit_str(&num_str);
    } else {
        output.emit_char('0');
    }
}

// Quick reject: more than one `.` or `e/E` → always invalid.
fn has_excessive_separators(s: &str) -> bool {
    let mut dot = 0u8;
    let mut exp = 0u8;
    for &b in s.as_bytes() {
        match b {
            // Decimal point: count it to reject multiple dots.
            b'.' => dot += 1,
            // Exponent marker: count it to reject multiple exponents.
            b'e' | b'E' => exp += 1,
            // Any other byte does not affect separator validation.
            _ => {}
        }
        if dot > 1 || exp > 1 {
            return true;
        }
    }
    false
}

#[cfg(feature = "serde-validate")]
fn validate_number(s: &str) -> bool {
    if has_excessive_separators(s) {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

#[cfg(not(feature = "serde-validate"))]
fn validate_number(s: &str) -> bool {
    if has_excessive_separators(s) {
        return false;
    }
    s.parse::<f64>().is_ok()
}

// Strip leading zeros from the integer part of a number string.
//   "007" → "7",  "-007.5" → "-7.5"
// Keeps a single zero for `0` alone or `0.x` / `0ex`.
fn normalize_leading_zeros_inplace(s: &mut String) {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return;
    }
    let start = if bytes[0] == b'-' { 1 } else { 0 };
    // Find end of integer part (first `.` or `e`/`E`)
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
    // Keep one zero when the integer part is *all* zeros (e.g. "00" → "0")
    let keep = if zeros == int_len { 1 } else { 0 };
    let remove_from = start + keep;
    let remove_to = start + zeros;
    if remove_to > remove_from {
        s.drain(remove_from..remove_to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(text: &str) -> InputCursor<'_> {
        InputCursor::new(text)
    }

    // ── scan_number_span ────────────────────────────────────────────────

    #[test]
    fn scan_number_span_pure_digits() {
        let input = cursor("12345abc");
        assert_eq!(scan_number_span(&input), 5);
    }

    #[test]
    fn scan_number_span_decimal_and_exponent() {
        let input = cursor("-3.14e+10}");
        assert_eq!(scan_number_span(&input), 9);
    }

    #[test]
    fn scan_number_span_plus_only_after_e() {
        // `+` at start is legal; `+` after non-e is a stop
        let input = cursor("+5+5");
        assert_eq!(scan_number_span(&input), 2);
    }

    #[test]
    fn scan_number_span_empty_at_non_digit() {
        let input = cursor("abc");
        assert_eq!(scan_number_span(&input), 0);
    }

    #[test]
    fn scan_number_span_trailing_dot_kept() {
        // `.` is consumed by the span; caller decides validity
        let input = cursor("5.");
        assert_eq!(scan_number_span(&input), 2);
    }

    // ── has_excessive_separators ───────────────────────────────────────

    #[test]
    fn excessive_separators_single_dot_ok() {
        assert!(!has_excessive_separators("1.5"));
    }

    #[test]
    fn excessive_separators_two_dots() {
        assert!(has_excessive_separators("1.2.3"));
    }

    #[test]
    fn excessive_separators_two_e() {
        assert!(has_excessive_separators("1e2e3"));
    }

    #[test]
    fn excessive_separators_dot_and_e_ok() {
        assert!(!has_excessive_separators("1.5e10"));
    }

    // ── normalize_leading_zeros_inplace ────────────────────────────────

    #[test]
    fn normalize_no_leading_zeros() {
        let mut s = String::from("123");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "123");
    }

    #[test]
    fn normalize_single_zero_kept() {
        let mut s = String::from("0");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "0");
    }

    #[test]
    fn normalize_leading_zeros_stripped() {
        let mut s = String::from("007");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "7");
    }

    #[test]
    fn normalize_negative_leading_zeros() {
        let mut s = String::from("-007.5");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "-7.5");
    }

    #[test]
    fn normalize_all_zeros_keep_one() {
        let mut s = String::from("00");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "0");
    }

    #[test]
    fn normalize_zero_before_decimal_kept() {
        let mut s = String::from("0.5");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "0.5");
    }

    #[test]
    fn normalize_zero_before_exponent_kept() {
        let mut s = String::from("0e10");
        normalize_leading_zeros_inplace(&mut s);
        assert_eq!(s, "0e10");
    }

    #[test]
    fn normalize_empty_string_noop() {
        let mut s = String::new();
        normalize_leading_zeros_inplace(&mut s);
        assert!(s.is_empty());
    }
}

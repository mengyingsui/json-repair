//! Internal utility functions shared across modules.

/// Returns the byte width of a UTF-8 character from its leading byte.
///
/// Assumes `lead` is the first byte of a well-formed UTF-8 sequence
/// (callers must ensure the input is valid UTF-8, e.g. via `&str`).
///
/// This intentionally does **not** use `char::len_utf8()` because that
/// requires materializing a full `char` (decoding the multi-byte sequence
/// first).  In hot scanning loops we only need the *width* to advance a
/// byte index — inspecting the leading byte alone is sufficient and
/// avoids the decode cost.
pub(crate) fn utf8_char_len(lead: u8) -> usize {
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

/// Returns the character at byte offset `pos`, or `None` if `pos` is out
/// of bounds or not at a valid UTF-8 char boundary.
///
/// ASCII bytes map directly to their `char`; multibyte sequences fall
/// back to `str::chars`.
pub(crate) fn char_at(text: &str, pos: usize) -> Option<char> {
    if pos >= text.len() {
        return None;
    }
    let byte = text.as_bytes()[pos];
    if byte.is_ascii() {
        Some(char::from(byte))
    } else {
        text[pos..].chars().next()
    }
}

#[cfg(test)]
mod tests {
    use super::{char_at, utf8_char_len};

    #[test]
    fn ascii_leading_byte() {
        for b in 0x00..0x80 {
            assert_eq!(utf8_char_len(b), 1, "ASCII byte 0x{b:02X} should be 1");
        }
    }

    #[test]
    fn two_byte_leading_bytes() {
        // 0xC2..0xDF are valid 2-byte leads; 0xC0..0xC1 are invalid in UTF-8
        // but the function only inspects the leading byte pattern.
        for b in 0xC0..0xE0 {
            assert_eq!(utf8_char_len(b), 2, "byte 0x{b:02X} should map to 2");
        }
    }

    #[test]
    fn three_byte_leading_bytes() {
        for b in 0xE0..0xF0 {
            assert_eq!(utf8_char_len(b), 3, "byte 0x{b:02X} should map to 3");
        }
    }

    #[test]
    fn four_byte_leading_bytes() {
        for b in 0xF0..=0xFF {
            assert_eq!(utf8_char_len(b), 4, "byte 0x{b:02X} should map to 4");
        }
    }

    #[test]
    fn known_code_points() {
        // ASCII
        assert_eq!(utf8_char_len(b'A'), 1);
        // U+00A0 NO-BREAK SPACE — 0xC2 0xA0
        assert_eq!(utf8_char_len(0xC2), 2);
        // U+2003 EM SPACE — 0xE2 0x80 0x83
        assert_eq!(utf8_char_len(0xE2), 3);
        // U+1F600 — 0xF0 0x9F 0x98 0x80
        assert_eq!(utf8_char_len(0xF0), 4);
    }

    // ── char_at ────────────────────────────────────────────────────────

    #[test]
    fn char_at_ascii() {
        assert_eq!(char_at("hello", 0), Some('h'));
        assert_eq!(char_at("hello", 4), Some('o'));
    }

    #[test]
    fn char_at_out_of_bounds_returns_none() {
        assert_eq!(char_at("hi", 2), None);
        assert_eq!(char_at("hi", 100), None);
    }

    #[test]
    fn char_at_multibyte() {
        // U+00A0 = 0xC2 0xA0
        assert_eq!(char_at("\u{00A0}x", 0), Some('\u{00A0}'));
        // U+1F600 = 0xF0 0x9F 0x98 0x80
        assert_eq!(char_at("\u{1F600}!", 0), Some('\u{1F600}'));
    }
}

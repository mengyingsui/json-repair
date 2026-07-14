use crate::repairer::InputCursor;

/// Returns `true` when `ch` starts a comment (`//`, `/*`, `#`, `--`).
pub(super) fn is_comment_start(input: &InputCursor, ch: char) -> bool {
    ch == '/' || ch == '#' || (ch == '-' && input.peek_is("--"))
}

/// Skip past a comment sequence at the cursor position.
/// Handles `//`, `/* ... */`, `#`, and `--` style comments.
pub(super) fn skip_comment(input: &mut InputCursor) {
    // C++-style // — skip to newline or EOF
    if input.peek_is("//") {
        while input.pos() < input.len() && input.cur() != '\n' {
            input.advance(input.cur().len_utf8());
        }
        if input.pos() < input.len() {
            input.advance(1);
        }
    // C-style /* ... */ — scan for */
    } else if input.peek_is("/*") {
        input.advance(2);
        while input.pos() + 1 < input.len() {
            if input.bytes()[input.pos()..].starts_with(b"*/") {
                input.advance(2);
                return;
            }
            input.advance(input.cur().len_utf8());
        }
        // Unterminated /* accepted — consume the rest of the input
        input.set_pos(input.len());
        // Shell/SQL style # or -- line comment
    } else if input.cur() == '#' || input.peek_is("--") {
        while input.pos() < input.len() && input.cur() != '\n' {
            input.advance(input.cur().len_utf8());
        }
        if input.pos() < input.len() {
            input.advance(1);
        }
    } else {
        // Not actually a comment — `/` was the start of a regex-like token
        input.advance(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(text: &str) -> InputCursor<'_> {
        InputCursor::new(text)
    }

    // ── is_comment_start ───────────────────────────────────────────────

    #[test]
    fn comment_start_slash_slash() {
        let c = cursor("// comment");
        assert!(is_comment_start(&c, '/'));
    }

    #[test]
    fn comment_start_slash_star() {
        let c = cursor("/* block */");
        assert!(is_comment_start(&c, '/'));
    }

    #[test]
    fn comment_start_hash() {
        let c = cursor("# comment");
        assert!(is_comment_start(&c, '#'));
    }

    #[test]
    fn comment_start_dash_dash() {
        let c = cursor("-- comment");
        assert!(is_comment_start(&c, '-'));
    }

    #[test]
    fn not_comment_start_single_dash() {
        let c = cursor("-3.14");
        assert!(!is_comment_start(&c, '-'));
    }

    #[test]
    fn not_comment_start_letter() {
        let c = cursor("hello");
        assert!(!is_comment_start(&c, 'h'));
    }

    // ── skip_comment ───────────────────────────────────────────────────

    #[test]
    fn skip_line_comment_to_newline() {
        let mut c = cursor("// hello\n{\"a\":1}");
        skip_comment(&mut c);
        assert_eq!(c.cur(), '{');
    }

    #[test]
    fn skip_line_comment_at_eof() {
        let mut c = cursor("// hello");
        skip_comment(&mut c);
        assert!(c.is_empty());
    }

    #[test]
    fn skip_hash_comment() {
        let mut c = cursor("# hello\n42");
        skip_comment(&mut c);
        assert_eq!(c.cur(), '4');
    }

    #[test]
    fn skip_dash_dash_comment() {
        let mut c = cursor("-- hello\n42");
        skip_comment(&mut c);
        assert_eq!(c.cur(), '4');
    }

    #[test]
    fn skip_block_comment() {
        let mut c = cursor("/* block */42");
        skip_comment(&mut c);
        assert_eq!(c.cur(), '4');
    }

    #[test]
    fn skip_unterminated_block_comment() {
        let mut c = cursor("/* unterminated...");
        skip_comment(&mut c);
        assert!(c.is_empty());
    }

    #[test]
    fn skip_block_comment_with_inner_stars() {
        let mut c = cursor("/* a * b * c */x");
        skip_comment(&mut c);
        assert_eq!(c.cur(), 'x');
    }

    #[test]
    fn skip_comment_lone_slash_advances_one() {
        // `/` not followed by `/` or `*` — single slash token
        let mut c = cursor("/regex/");
        skip_comment(&mut c);
        assert_eq!(c.cur(), 'r');
    }
}

//! Comment skipping for C++-style, C-style, shell-style and SQL-style comments.
//!
//! Comments are consumed during structural parsing so they do not appear
//! in the repaired JSON output.  This module only decides whether a
//! character starts a comment and advances the cursor past it.

use crate::repairer::{InputCursor, Tracer};
use crate::util::utf8_char_len;

/// Returns `true` when `ch` starts a comment (`//`, `/*`, `#`, `--`).
pub(super) fn is_comment_start(input: &InputCursor, ch: char) -> bool {
    ch == '/' || ch == '#' || (ch == '-' && input.peek_is("--"))
}

/// Skip past a comment sequence at the cursor position.
/// Handles `//`, `/* ... */`, `#`, and `--` style comments.
pub(super) fn skip_comment(input: &mut InputCursor, tracer: &mut Tracer) {
    let _ = tracer;
    // C++-style // — skip to newline or EOF
    if input.peek_is("//") {
        #[cfg(feature = "tracing")]
        let pos = input.pos();
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::CommentSkipped {
                style: crate::trace::CommentStyle::Line,
                start: pos,
            }
        );
        while let Some(ch) = input.cur() {
            if ch == '\n' {
                break;
            }
            input.advance(ch.len_utf8());
        }
        if input.pos() < input.len() {
            input.advance(1);
        }
    // C-style /* ... */ — scan for */
    } else if input.peek_is("/*") {
        #[cfg(feature = "tracing")]
        let pos = input.pos();
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::CommentSkipped {
                style: crate::trace::CommentStyle::Block,
                start: pos,
            }
        );
        input.advance(2);
        while input.pos() + 1 < input.len() {
            if input.bytes()[input.pos()..].starts_with(b"*/") {
                input.advance(2);
                return;
            }
            // Loop condition guarantees pos < len, so the leading byte is in bounds.
            input.advance(utf8_char_len(input.bytes()[input.pos()]));
        }
        // Unterminated /* accepted — consume the rest of the input
        input.set_pos(input.len());
        // Shell/SQL style # line comment
    } else if input.cur() == Some('#') {
        #[cfg(feature = "tracing")]
        let pos = input.pos();
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::CommentSkipped {
                style: crate::trace::CommentStyle::Hash,
                start: pos,
            }
        );
        while let Some(ch) = input.cur() {
            if ch == '\n' {
                break;
            }
            input.advance(ch.len_utf8());
        }
        if input.pos() < input.len() {
            input.advance(1);
        }
    // SQL style -- line comment
    } else if input.peek_is("--") {
        #[cfg(feature = "tracing")]
        let pos = input.pos();
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::CommentSkipped {
                style: crate::trace::CommentStyle::DashDash,
                start: pos,
            }
        );
        while let Some(ch) = input.cur() {
            if ch == '\n' {
                break;
            }
            input.advance(ch.len_utf8());
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
#[allow(clippy::let_unit_value, clippy::unused_unit)]
mod tests {
    use super::*;

    fn cursor(text: &str) -> InputCursor<'_> {
        InputCursor::new(text)
    }

    fn tracer() -> Tracer<'static> {
        #[cfg(feature = "tracing")]
        {
            None
        }
        #[cfg(not(feature = "tracing"))]
        {
            ()
        }
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
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('{'));
    }

    #[test]
    fn skip_line_comment_at_eof() {
        let mut c = cursor("// hello");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert!(c.is_empty());
    }

    #[test]
    fn skip_hash_comment() {
        let mut c = cursor("# hello\n42");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('4'));
    }

    #[test]
    fn skip_dash_dash_comment() {
        let mut c = cursor("-- hello\n42");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('4'));
    }

    #[test]
    fn skip_block_comment() {
        let mut c = cursor("/* block */42");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('4'));
    }

    #[test]
    fn skip_unterminated_block_comment() {
        let mut c = cursor("/* unterminated...");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert!(c.is_empty());
    }

    #[test]
    fn skip_block_comment_with_inner_stars() {
        let mut c = cursor("/* a * b * c */x");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('x'));
    }

    #[test]
    fn skip_comment_lone_slash_advances_one() {
        // `/` not followed by `/` or `*` — single slash token
        let mut c = cursor("/regex/");
        let mut t = tracer();
        skip_comment(&mut c, &mut t);
        assert_eq!(c.cur(), Some('r'));
    }
}

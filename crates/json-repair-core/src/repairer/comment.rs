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
        while input.i < input.text.len() && input.cur() != '\n' {
            input.i += input.cur().len_utf8();
        }
        if input.i < input.text.len() {
            input.i += 1;
        }
    // C-style /* ... */ — scan for */
    } else if input.peek_is("/*") {
        input.i += 2;
        while input.i + 1 < input.text.len() {
            if input.text.as_bytes()[input.i..].starts_with(b"*/") {
                input.i += 2;
                return;
            }
            input.i += input.cur().len_utf8();
        }
        // Unterminated /* accepted — no error, just consume
        // Shell/SQL style # or -- line comment
    } else if input.cur() == '#' || input.peek_is("--") {
        while input.i < input.text.len() && input.cur() != '\n' {
            input.i += input.cur().len_utf8();
        }
        if input.i < input.text.len() {
            input.i += 1;
        }
    } else {
        // Not actually a comment — `/` was the start of a regex-like token
        input.i += 1;
    }
}

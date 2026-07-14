use super::{InputCursor, OutputBuffer, keys};

// Canonical JSON tokens (left column) and their aliases (right column):
//   "none", "undefined", "nan", "infinity", … → null
//   "yes" → true,  "no" → false
//   "nil", "nullptr" → null
//   "+infinity", "-infinity" → null
const LIT_TRUE: &str = "true";
const LIT_FALSE: &str = "false";
const LIT_NULL: &str = "null";
const LIT_NONE: &str = "none";
const LIT_UNDEFINED: &str = "undefined";
const LIT_NAN: &str = "nan";
const LIT_INFINITY: &str = "infinity";
const LIT_POS_INF: &str = "+infinity";
const LIT_NEG_INF: &str = "-infinity";
const LIT_YES: &str = "yes";
const LIT_NO: &str = "no";
const LIT_NIL: &str = "nil";
const LIT_NULLPTR: &str = "nullptr";

// ASCII case-insensitive match at cursor position.
// Used against LLM output which may capitalize or mix case.
fn match_lit(input: &InputCursor, pat: &str) -> bool {
    let plen = pat.len();
    if input.pos() + plen > input.len() {
        return false;
    }
    input.bytes()[input.pos()..input.pos() + plen]
        .iter()
        .zip(pat.bytes())
        .all(|(&a, b)| a.eq_ignore_ascii_case(&b))
}

// Try every known literal pattern; fall back to generic unquoted value.
pub(super) fn parse_literal(input: &mut InputCursor, output: &mut OutputBuffer) {
    const ENTRIES: &[(&str, &str)] = &[
        (LIT_TRUE, LIT_TRUE),
        (LIT_FALSE, LIT_FALSE),
        (LIT_NULL, LIT_NULL),
        (LIT_NONE, LIT_NULL),
        (LIT_UNDEFINED, LIT_NULL),
        (LIT_NAN, LIT_NULL),
        (LIT_INFINITY, LIT_NULL),
        (LIT_YES, LIT_TRUE),
        (LIT_NO, LIT_FALSE),
        (LIT_NIL, LIT_NULL),
        (LIT_NULLPTR, LIT_NULL),
        (LIT_POS_INF, LIT_NULL),
        (LIT_NEG_INF, LIT_NULL),
    ];
    for &(pat, emit) in ENTRIES {
        if match_lit(input, pat) {
            output.emit_str(emit);
            input.advance(pat.len());
            return;
        }
    }
    // Not a recognizable literal — treat as an unquoted string value.
    keys::parse_unquoted_value(input, output);
}

/// Returns `true` if `ch` can start a JSON literal token (`true`, `false`,
/// `null`, or their aliases like `Infinity`, `Undefined`, `NaN`, …).
///
/// Used by [`run_value`](super::Repairer::run_value) to dispatch to
/// [`parse_literal`] without hard-coding a 10-character match arm.
pub(super) fn is_literal_start(ch: char) -> bool {
    matches!(
        ch,
        't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U'
    )
}

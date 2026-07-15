//! Unquoted literal parsing (`true`, `false`, `null` and common aliases).
//!
//! Handles LLM-emitted variants such as `True`, `None`, `Undefined`,
//! `NaN`, `Infinity`, `+Infinity` and `-Infinity`, normalizing them to
//! canonical JSON literals.

use super::{InputCursor, OutputBuffer, Tracer, keys};

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
pub(super) fn parse_literal(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    const ENTRIES: &[(&str, &str, &str)] = &[
        (LIT_TRUE, LIT_TRUE, "llm_literal_to_json"),
        (LIT_FALSE, LIT_FALSE, "llm_literal_to_json"),
        (LIT_NULL, LIT_NULL, "llm_literal_to_json"),
        (LIT_NONE, LIT_NULL, "llm_literal_to_json"),
        (LIT_UNDEFINED, LIT_NULL, "llm_literal_to_json"),
        (LIT_NAN, LIT_NULL, "nan_to_null"),
        (LIT_INFINITY, LIT_NULL, "infinity_to_null"),
        (LIT_YES, LIT_TRUE, "llm_literal_to_json"),
        (LIT_NO, LIT_FALSE, "llm_literal_to_json"),
        (LIT_NIL, LIT_NULL, "llm_literal_to_json"),
        (LIT_NULLPTR, LIT_NULL, "llm_literal_to_json"),
        (LIT_POS_INF, LIT_NULL, "infinity_to_null"),
        (LIT_NEG_INF, LIT_NULL, "infinity_to_null"),
    ];
    for &(pat, emit, kind) in ENTRIES {
        if match_lit(input, pat) {
            emit_trace!(tracer, crate::trace::TraceEvent::ValueNormalized { kind });
            let _ = kind;
            output.emit_str(emit);
            input.advance(pat.len());
            return;
        }
    }
    // Not a recognizable literal — treat as an unquoted string value.
    keys::parse_unquoted_value(input, output, tracer);
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

/// Attempt to parse a signed Infinity literal (`+Infinity` / `-Infinity`,
/// case-insensitive) at the current cursor position.
///
/// If matched, the cursor is advanced past the literal and `null` is emitted
/// to `output`. Returns `true` on success.
///
/// Used by [`run_value`](super::Repairer::run_value) for `+` and `-`
/// dispatch, which otherwise treat those characters as numbers or comments.
pub(super) fn try_parse_signed_infinity(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    sign: char,
    tracer: &mut Tracer,
) -> bool {
    let _ = tracer;
    let pat = match sign {
        '+' => LIT_POS_INF,
        '-' => LIT_NEG_INF,
        _ => return false,
    };
    if match_lit(input, pat) {
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::ValueNormalized {
                kind: "infinity_to_null",
            }
        );
        input.advance(pat.len());
        output.emit_str("null");
        true
    } else {
        false
    }
}

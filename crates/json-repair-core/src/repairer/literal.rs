//! Bareword literal parsing (`true`, `false`, `null`, `Infinity`, `NaN`).

use super::Repairer;

/// Literal string patterns recognised by `parse_literal`.
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

impl Repairer {
    /// Case-insensitive prefix match against a pattern, starting at `self.i`.
    /// Returns `true` if the next characters (case-insensitively) equal `pat`.
    #[inline]
    fn match_lit(&self, pat: &str) -> bool {
        let plen = pat.len();
        if self.i + plen > self.n {
            return false;
        }
        self.text.as_bytes()[self.i..self.i + plen]
            .iter()
            .zip(pat.bytes())
            .all(|(&a, b)| a.eq_ignore_ascii_case(&b))
    }

    /// Parse a bareword literal (`true`/`false`/`null`/`none`/`undefined`/
    /// `NaN`/`Infinity`), emitting the JSON equivalent.  Falls back to
    /// `parse_unquoted_value` if no literal matches.
    pub(super) fn parse_literal(&mut self) {
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
            if self.match_lit(pat) {
                self.out.push_str(emit);
                self.i += pat.len();
                return;
            }
        }
        self.parse_unquoted_value();
    }
}

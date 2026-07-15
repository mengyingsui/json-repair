//! JSON pretty-printer for already-valid JSON strings.
//!
//! [`format_json`] performs a lightweight post-repair formatting pass,
//! similar to Python's `json.dumps(..., indent=N)`. It does not repair
//! malformed input — callers should run [`repair_json`](crate::repair_json)
//! first when the input might be invalid.

use crate::error::{JsonRepairError, JsonRepairErrorKind};

/// Pretty-print a valid JSON string with configurable indentation.
///
/// `indent` is the number of spaces per nesting level. Passing `0`
/// still emits newlines but no leading indentation.
///
/// # Errors
///
/// Returns [`JsonRepairError`] if the input is not structurally valid JSON
/// (e.g. unclosed string or unbalanced brackets).
///
/// # Example
///
/// ```
/// use json_repair_core::format_json;
///
/// let compact = r#"{"a":1,"b":[1,2]}"#;
/// let pretty = format_json(compact, 2).unwrap();
/// assert!(pretty.contains("\n  \"a\": 1"));
/// ```
pub fn format_json(text: &str, indent: usize) -> Result<String, JsonRepairError> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let mut out = String::with_capacity(text.len() * 2);
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    // Track container nesting to detect unbalanced input.
    let mut bracket_stack: Vec<char> = Vec::new();

    for ch in text.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '{' | '[' => {
                out.push(ch);
                bracket_stack.push(ch);
                depth += 1;
                out.push('\n');
                push_indent(&mut out, depth, indent);
            }
            '}' | ']' => {
                let opener = match ch {
                    '}' => '{',
                    _ => '[',
                };
                if bracket_stack.pop() != Some(opener) {
                    return Err(JsonRepairError::new(JsonRepairErrorKind::InvalidJson));
                }
                depth = depth.saturating_sub(1);
                out.push('\n');
                push_indent(&mut out, depth, indent);
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                out.push('\n');
                push_indent(&mut out, depth, indent);
            }
            ':' => {
                out.push(ch);
                out.push(' ');
            }
            '"' => {
                out.push(ch);
                in_string = true;
            }
            c if c.is_whitespace() => {
                // Drop original whitespace; our own formatting inserts it.
                continue;
            }
            _ => out.push(ch),
        }
    }

    if in_string || !bracket_stack.is_empty() {
        return Err(JsonRepairError::new(JsonRepairErrorKind::InvalidJson));
    }

    Ok(out)
}

// Append `depth * indent` spaces to `out`.
fn push_indent(out: &mut String, depth: usize, indent: usize) {
    let spaces = depth.saturating_mul(indent);
    out.reserve(spaces);
    for _ in 0..spaces {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_empty_returns_empty() {
        assert_eq!(format_json("", 2).unwrap(), "");
        assert_eq!(format_json("   ", 2).unwrap(), "");
    }

    #[test]
    fn format_indent_zero_still_newlines() {
        let input = r#"{"a":1,"b":2}"#;
        let expected = "{\n\"a\": 1,\n\"b\": 2\n}";
        assert_eq!(format_json(input, 0).unwrap(), expected);
    }

    #[test]
    fn format_pretty_object() {
        let input = r#"{"a":1,"b":[1,2]}"#;
        let out = format_json(input, 2).unwrap();
        assert_eq!(out, "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2\n  ]\n}");
    }

    #[test]
    fn format_pretty_nested() {
        let input = r#"{"a":{"b":1}}"#;
        let out = format_json(input, 2).unwrap();
        assert_eq!(out, "{\n  \"a\": {\n    \"b\": 1\n  }\n}");
    }

    #[test]
    fn format_preserves_string_content() {
        let input = r#"{"msg":"hello, world"}"#;
        let out = format_json(input, 2).unwrap();
        assert!(out.contains("\"msg\""));
        assert!(out.contains("\"hello, world\""));
    }

    #[test]
    fn format_preserves_escaped_quotes() {
        let input = r#"{"a":"say \"hi\""}"#;
        let out = format_json(input, 2).unwrap();
        assert!(out.contains("\"say \\\"hi\\\"\""));
    }

    #[test]
    fn format_rejects_unclosed_string() {
        let input = r#"{"a":"unclosed}"#;
        assert!(format_json(input, 2).is_err());
    }

    #[test]
    fn format_rejects_unbalanced_brackets() {
        let input = r#"{"a":1]"#;
        assert!(format_json(input, 2).is_err());
    }
}

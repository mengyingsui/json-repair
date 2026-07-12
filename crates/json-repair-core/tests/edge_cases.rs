mod helpers;

use helpers::roundtrip;
use serde::Deserialize;

#[test]
fn test_empty_input() {
    assert_eq!(json_repair_core::repair_json("").unwrap(), "");
    assert_eq!(json_repair_core::repair_json("   ").unwrap(), "");
}

#[test]
fn test_very_long_string() {
    let long_text = "A".repeat(10000);
    let input = format!("{{\"key\": \"{long_text}\"}}");
    let result = roundtrip(&input);
    assert_eq!(result, serde_json::json!({"key": long_text}));
}

#[test]
fn test_backslash_before_newline_in_string() {
    // Fuzz crash: {vho: "r*\<LF>"}  — backslash followed by raw newline
    // emit_escape('\n') must escape it, not emit a raw newline.
    let input = "{\"vho\": \"r*\\\n\"}";
    let result = json_repair_core::repair_json(input);
    assert!(result.is_ok(), "repair should succeed: {:?}", result.err());
    let result = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let obj = parsed.as_object().unwrap();
    assert_eq!(obj["vho"], "r*\n", "value should be preserved");
}

#[test]
fn test_multiple_exponent_markers_in_number() {
    // Fuzz crash: long string with multiple E groups like 12333...EEE...33...EEE...33
    // parse_number must reject multi-E numbers and emit safe fallback.
    let mut input = String::from("1");
    input.push_str(&"3".repeat(40));
    input.push_str(&"E".repeat(30));
    input.push_str(&"3".repeat(20));
    input.push_str(&"E".repeat(30));
    input.push_str(&"3".repeat(10));
    let result = json_repair_core::repair_json(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    assert_eq!(parsed, 0, "multi-E number should fall back to 0");
}

#[test]
fn test_invalid_unicode_escape_in_string() {
    // Fuzz crash: \u followed by non-hex chars like \uehu produces \uehu
    // which is invalid JSON (\u requires exactly 4 hex digits).
    // emit_escape must validate all 4 hex digits before emitting \u.
    let input = "{\"key\": \"\\uehu\"}";
    let result = json_repair_core::repair_json(input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    let obj = parsed.as_object().unwrap();
    assert_eq!(
        obj["key"], "\\uehu",
        "invalid \\u escape should be preserved as literal text"
    );
}

#[test]
fn test_long_number_with_trailing_dash() {
    // Fuzz crash: 18888...888- has trailing `-` with >100 chars
    let mut input = String::from("1");
    input.push_str(&"8".repeat(100));
    input.push('-');
    let result = json_repair_core::repair_json(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    assert_eq!(parsed, 0, "trailing-dash number should fall back to 0");
}

#[test]
fn test_long_number_with_dash_dot_in_middle() {
    // Fuzz crash: 222...-.226... has `-.` in middle with >100 chars
    let mut input = "222".to_string();
    input.push_str(&"2".repeat(100));
    input.push_str("-.33333333333333333333");
    let result = json_repair_core::repair_json(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    assert_eq!(parsed, 0, "dash-dot number should fall back to 0");
}

#[test]
fn test_trailing_comma_after_value() {
    // When a comma in the input is followed by a char that's not `}` and not
    // a valid key start, object_loop breaks (line 107) without stripping the
    // comma. close_brackets must still strip it.
    let input = String::from("{\"a\": 1,\x01}");
    let result = json_repair_core::repair_json(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    let obj = parsed.as_object().unwrap();
    assert_eq!(obj["a"], 1, "value should be preserved");
}

#[test]
fn test_huge_pure_digit_number_not_emitted() {
    // Fuzz crash: 400-digit number overflows f64, must not be emitted as-is.
    // validate_number must reject it (serde_json rejects numbers > f64::MAX).
    let input = "9".repeat(400);
    let result = json_repair_core::repair_json(&input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    assert_eq!(parsed, 0, "huge number should fall back to 0");
}

#[test]
fn test_surrogate_escape_in_string_key_replaced() {
    // Fuzz crash: \ude22 in a JSON string escape produces a lone surrogate
    // which serde_json rejects. emit_escape must replace surrogates with \ufffd.
    let input = "{\"\\u0001\\u0000\\u0000e\\ude22stest\": null}";
    let result = json_repair_core::repair_json(input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|e| panic!("invalid JSON: {}\n---\n{}\n---", e, result));
    let obj = parsed.as_object().unwrap();
    let mut expected_key = String::new();
    expected_key.push('\u{0001}');
    expected_key.push('\u{0000}');
    expected_key.push('\u{0000}');
    expected_key.push('e');
    expected_key.push('\u{fffd}');
    expected_key.push_str("stest");
    assert_eq!(obj[&expected_key], serde_json::Value::Null);
}

#[test]
fn test_deeply_nested_128_brackets() {
    // Fuzz crash: [[[...[]...]]] with exactly 128 [ produces output at
    // serde_json's recursion boundary. repair_json must not panic from
    // debug_assert! (mod.rs) or stack overflow.
    let input = format!("{}{}", "[".repeat(128), "]".repeat(128));
    let result = json_repair_core::repair_json(&input);
    assert!(
        result.is_ok(),
        "128-depth nesting should repair: {:?}",
        result.err()
    );
    let repaired = result.unwrap();
    use serde::Deserialize;
    let mut de = serde_json::Deserializer::from_str(&repaired);
    de.disable_recursion_limit();
    let parsed = serde_json::Value::deserialize(&mut de);
    assert!(
        parsed.is_ok(),
        "128-depth output must be valid JSON: {}",
        parsed.unwrap_err()
    );
}

#[test]
fn test_deeply_nested_brackets() {
    // serde_json default recursion limit (128) would reject 400-deep output,
    // so we use disable_recursion_limit() for verification.
    let input = format!("{}{}", "[".repeat(400), "]".repeat(400));
    let result = json_repair_core::repair_json(&input);
    assert!(result.is_ok(), "deeply nested brackets should repair");
    let repaired = result.unwrap();
    let mut de = serde_json::Deserializer::from_str(&repaired);
    de.disable_recursion_limit();
    let parsed = serde_json::Value::deserialize(&mut de);
    assert!(
        parsed.is_ok(),
        "must be valid JSON: {}",
        parsed.unwrap_err()
    );
}

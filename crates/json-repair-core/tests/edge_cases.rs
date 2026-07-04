mod helpers;

use helpers::roundtrip;

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
fn test_control_chars_in_unquoted_key() {
    // Crash input from fuzzer: {\0\0\0\x1a}
    // Null bytes should not be emitted raw into JSON output
    let mut input = String::from('{');
    input.push('\0');
    input.push('\0');
    input.push('\0');
    input.push('\u{1a}');
    input.push('}');
    let result = json_repair_core::repair_json(&input).unwrap();
    // Result must be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // Key should be unquoted, repaired, with control chars escaped as \uXXXX
    let key = parsed.as_object().unwrap().keys().next().unwrap();
    assert_eq!(key, "\0\0\0\u{1a}", "key content should be preserved");
}

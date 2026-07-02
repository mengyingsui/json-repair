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

mod helpers;

use helpers::roundtrip;

#[test]
fn test_metatag_before_json() {
    let result = roundtrip("[TEXT_START]\n{\"a\": 1}");
    assert_eq!(result, serde_json::json!({"a": 1}));
}

#[test]
fn test_metatag_before_array() {
    let result = roundtrip("[TEXT_START] some metadata [{\"a\": 1, \"b\": 2}]");
    assert_eq!(result, serde_json::json!([{"a": 1, "b": 2}]));
}

#[test]
fn test_multiple_metatags() {
    let result = roundtrip("[TAG_A][TAG_B][TAG_C]{\"result\": true}");
    assert_eq!(result, serde_json::json!({"result": true}));
}

#[test]
fn test_single_char_tag() {
    let result = roundtrip("[X]{\"a\": 1}");
    assert_eq!(result, serde_json::json!({"a": 1}));
}

#[test]
fn test_text_code_fence_skipped() {
    let result = roundtrip("[TEXT_START]\n```text\nsome text\n```\n[TEXT_END]\n{\"hello\": \"world\"}");
    assert_eq!(result, serde_json::json!({"hello": "world"}));
}

#[test]
fn test_json_code_fence_parsed() {
    let result = roundtrip("```json\n{\"a\": 1}\n```");
    assert_eq!(result, serde_json::json!({"a": 1}));
}

#[test]
fn test_text_then_json_fence() {
    let result = roundtrip("[TEXT_START]\n```text\nstuff\n```\n[TEXT_END]\n```json\n{\"a\": 1}\n```");
    assert_eq!(result, serde_json::json!({"a": 1}));
}

#[test]
fn test_real_array_not_skipped() {
    let result = roundtrip("[1, 2, 3]");
    assert_eq!(result, serde_json::json!([1, 2, 3]));
}

#[test]
fn test_real_array_strings_not_skipped() {
    let result = roundtrip("[\"hello\", \"world\"]");
    assert_eq!(result, serde_json::json!(["hello", "world"]));
}

#[test]
fn test_metatag_and_text_before_json() {
    let result = roundtrip("prefix text then [TEXT_START] tags [TEXT_END] before {\"key\": \"value\"}");
    assert_eq!(result, serde_json::json!({"key": "value"}));
}

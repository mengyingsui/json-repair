mod helpers;

use helpers::roundtrip;

// ── adjacent objects ─────────────────────────────────────────────────

#[test]
fn test_adjacent_objects_wrapped() {
    let big_obj = format!(r#"{{"key": "{}", "num": 42}}"#, "a".repeat(500));
    let text = big_obj.repeat(20);
    let repaired = json_repair_core::repair_json(&text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 20);
}

#[test]
fn test_adjacent_objects_mixed_commas() {
    let big_obj = format!(r#"{{"key": "{}", "num": 42}}"#, "a".repeat(500));
    let mut parts: Vec<String> = (0..7).map(|_| big_obj.clone()).collect();
    parts.push(format!(",{}", big_obj));
    parts.extend((0..12).map(|_| big_obj.clone()));
    let text = parts.join("");
    let repaired = json_repair_core::repair_json(&text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 20);
}

// ── complex scenarios ────────────────────────────────────────────────

#[test]
fn test_llm_response_with_quotes() {
    let text = "{\n  \"response\": \"The user said \"hello\" and I replied \"hi there\"\",\n  \"sentiment\": \"positive\"\n}";
    let result = roundtrip(text);
    assert_eq!(
        result["response"],
        "The user said \"hello\" and I replied \"hi there\""
    );
    assert_eq!(result["sentiment"], "positive");
}

#[test]
fn test_llm_json_with_code() {
    let text = "{\"code\": \"\"\"def greet(name):\\n    print(f\"Hello, {name}\")\n\"\"\"}";
    let result = roundtrip(text);
    let code = result["code"].as_str().unwrap();
    assert!(code.contains("def greet(name):"));
    assert!(code.contains("Hello, {name}"));
}

// ── implicit array ───────────────────────────────────────────────────

#[test]
fn test_comma_separated_objects() {
    let obj = r#"{"a": 1}"#;
    let text = (0..20).map(|_| obj).collect::<Vec<_>>().join(",\n");
    let repaired = json_repair_core::repair_json(&text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    assert!(result.is_object(), "expected dict, got {result:?}");
}

#[test]
fn test_two_objects_only() {
    let text = "{\"x\": \"hello\"},\n{\"y\": \"world\"}";
    let repaired = json_repair_core::repair_json(text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    assert!(result.is_object(), "expected dict, got {result:?}");
}

#[test]
fn test_not_triggered_for_single_object() {
    let result = roundtrip("{\"key\": \"value with }, { pattern inside\"}");
    assert_eq!(
        result,
        serde_json::json!({"key": "value with }, { pattern inside"})
    );
}

#[test]
fn test_small_block_not_wrapped() {
    let result = roundtrip("{\"a\": \"}, {\"}");
    assert_eq!(result, serde_json::json!({"a": "}, {"}));
}

#[test]
fn test_large_implicit_array() {
    let big_obj = format!(r#"{{"key": "{}", "num": 42}}"#, "a".repeat(350));
    let text = (0..25)
        .map(|_| big_obj.as_str())
        .collect::<Vec<_>>()
        .join(",\n");
    let repaired = json_repair_core::repair_json(&text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 25);
}

// ── misordered brackets ──────────────────────────────────────────────

#[test]
fn test_bracket_instead_of_brace() {
    let result =
        roundtrip("{\"actions\": [{\"text\": \"a\", \"verb\": \"b\", \"object\": \"c\"]}]}");
    assert_eq!(
        result,
        serde_json::json!({"actions": [{"text": "a", "verb": "b", "object": "c"}]})
    );
}

#[test]
fn test_mixed_objects_last_broken() {
    let result = roundtrip("{\"arr\": [{\"x\": 1}, {\"y\": 2}, {\"z\": 3]}");
    assert_eq!(
        result,
        serde_json::json!({"arr": [{"x": 1}, {"y": 2}, {"z": 3}]})
    );
}

#[test]
fn test_swapped_brackets() {
    let result = roundtrip("{\"data\": [{\"id\": 1, \"val\": \"test\"]}}");
    assert_eq!(
        result,
        serde_json::json!({"data": [{"id": 1, "val": "test"}]})
    );
}

#[test]
fn test_deeply_nested() {
    let result = roundtrip("{\"a\": {\"b\": [{\"c\": 1, \"d\": 2]}}");
    assert_eq!(result, serde_json::json!({"a": {"b": [{"c": 1, "d": 2}]}}));
}

#[test]
fn test_extra_brackets_in_input() {
    let result = roundtrip("{\"items\": [{\"name\": \"x\", \"value\": \"\"]}]}");
    assert_eq!(
        result,
        serde_json::json!({"items": [{"name": "x", "value": ""}]})
    );
}

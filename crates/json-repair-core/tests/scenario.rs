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
    assert!(result.is_array(), "expected array, got {result:?}");
    assert_eq!(result.as_array().unwrap().len(), 20);
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

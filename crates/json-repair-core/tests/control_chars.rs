#[test]
fn test_literal_carriage_return() {
    let result = json_repair_core::repair_json("{\"text\": \"line1\r\nline2\"}").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let text = parsed["text"].as_str().unwrap();
    assert!(
        text == "line1\r\nline2" || text == "line1\nline2",
        "unexpected text: {text:?}"
    );
}

#![cfg(not(miri))]

const INPUT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/cases/unterminated_string.jsonl"
));

#[test]
fn test_repair_missing_closing_quote() {
    let obj: serde_json::Value = serde_json::from_str(INPUT.trim()).unwrap();
    let text = obj["input"].as_str().unwrap();
    let repaired = json_repair_core::repair_json(text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    let segments = result["attributes"].as_array().unwrap();
    assert_eq!(segments.len(), 8);
    assert_eq!(segments[1]["entity"], "\u{94c1}\u{7532}\u{8230}");
    assert!(
        segments[1]["text"].as_str().unwrap().ends_with('\u{2019}'),
        "expected text to end with smart quote"
    );
}

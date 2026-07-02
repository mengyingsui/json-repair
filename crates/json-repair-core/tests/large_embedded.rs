const INPUT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/cases/embedded_quotes_large.jsonl"
));

#[test]
fn test_repair_multi_segment() {
    let obj: serde_json::Value = serde_json::from_str(INPUT.trim()).unwrap();
    let text = obj["input"].as_str().unwrap();
    let repaired = json_repair_core::repair_json(text).unwrap();
    let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    let segments = result["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 5);
    assert!(segments[0]["text"].as_str().unwrap().contains("阿尔杰"));
    assert_eq!(segments[0]["process"], "讨论聚会的意义和潜在合作");
    assert_eq!(
        segments[segments.len() - 1]["process"],
        "确认聚会地点并讨论塔罗牌的性质"
    );
}

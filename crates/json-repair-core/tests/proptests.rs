use std::path::Path;

use proptest::collection::{hash_map, vec};
use proptest::prelude::*;
use proptest::sample::select;

fn json_scalar() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        (-1_000_000i64..1_000_000).prop_map(|n| serde_json::Value::Number(n.into())),
        "[a-zA-Z0-9 ]{0,20}".prop_map(serde_json::Value::String),
    ]
}

fn json_value() -> impl Strategy<Value = serde_json::Value> {
    let leaf = json_scalar();
    leaf.prop_recursive(4, 20, 5, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..5).prop_map(serde_json::Value::Array),
            hash_map("[a-z]{1,8}", inner, 0..5)
                .prop_map(|m| serde_json::Value::Object(serde_json::Map::from_iter(m))),
        ]
    })
}

fn broken_inputs_static() -> &'static [&'static str] {
    use std::sync::LazyLock;
    static INPUTS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/cases/broken_patterns.jsonl");
        let content = std::fs::read_to_string(path).unwrap();
        let mut inputs = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(s) = obj.get("input").and_then(|v| v.as_str()) {
                    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
                    inputs.push(leaked);
                }
            }
        }
        inputs
    });
    INPUTS.as_slice()
}

fn broken_input_strategy() -> impl Strategy<Value = String> {
    select(broken_inputs_static()).prop_map(|s| s.to_string())
}

proptest! {
    #[test]
    fn valid_json_passthrough(value in json_value()) {
        let text = serde_json::to_string(&value).unwrap();
        let repaired = json_repair_core::repair_json(&text).unwrap();
        let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        prop_assert_eq!(&result, &value);
    }

    #[test]
    fn string_content_preserved(content in "[a-zA-Z0-9 .,!?]{1,100}") {
        let text = format!(r#"{{"key": "{content}"}}"#);
        let repaired = json_repair_core::repair_json(&text).unwrap();
        let result: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        prop_assert_eq!(result["key"].as_str(), Some(content.as_str()));
    }

    #[test]
    fn repair_is_idempotent(input in broken_input_strategy()) {
        let first = match json_repair_core::repair_json(&input) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        if first.is_empty() {
            return Ok(());
        }
        let second = json_repair_core::repair_json(&first).unwrap();
        prop_assert_eq!(&first, &second);
    }

    #[test]
    fn broken_produces_valid_json(input in broken_input_strategy()) {
        let repaired = match json_repair_core::repair_json(&input) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        if !repaired.is_empty() {
            let _: serde_json::Value = serde_json::from_str(&repaired)
                .expect("repaired output must be valid JSON");
        }
    }
}

/// ── Number-corruption edge cases ──────────────────────────────────────────
fn numeric_corruption_input() -> impl Strategy<Value = String> {
    prop_oneof![
        // "number + junk" patterns — the core "123abc" class of bug
        (any::<u64>(), "[a-z_]{1,10}").prop_map(|(n, suffix)| format!("{n}{suffix}")),
        (any::<i64>(), "[a-z]{1,5}").prop_map(|(n, suffix)| format!("{n}{suffix}")),
        (r"[0-9]{1,3}\.[0-9]{1,3}", "[a-zA-Z]{1,5}")
            .prop_map(|(n, suffix)| format!("{n}{suffix}")),
        // multiple decimal points
        r"[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
        // trailing dot
        r"[0-9]{1,5}\.",
        r"\.[0-9]{1,5}",
        // leading zeros
        r"0[0-9]{2,6}",
        r"-0[0-9]{1,5}",
        // hex-like (0x...)
        r"0[xX][0-9a-fA-F]{1,8}",
        // multiple signs
        r"[+-]{2,3}[0-9]{1,5}",
        r"--[0-9]{1,5}",
        // malformed scientific
        r"[0-9]{1,3}e[+-]?[0-9]{0,3}[a-z]{0,3}",
        r"[0-9]{1,3}e[0-9]{1,2}\.[0-9]{1,2}",
        r"[0-9]{1,2}e[0-9]{1,2}e[0-9]{1,2}",
        // sign in wrong position
        r"[0-9]{2,4}-[0-9]{2,4}",
        // embedded spaces
        r"[0-9]{2} [0-9]{2}",
        r"[0-9]{1,3}\. [0-9]{1,3}",
        r"[0-9]{1,3}\s+\.[0-9]{1,3}",
        // lone operators
        Just("+".to_string()),
        Just("-".to_string()),
        Just(".".to_string()),
        Just("e".to_string()),
    ]
}

proptest! {
    #[test]
    fn numeric_corruption_rejected_or_fixed(input in numeric_corruption_input()) {
        // Some number-corrupted inputs may be repairable, others must error.
        // Neither path should panic or produce invalid JSON.
        if let Ok(repaired) = json_repair_core::repair_json(&input) {
            if !repaired.is_empty() {
                let _: serde_json::Value = serde_json::from_str(&repaired)
                    .expect("repaired number-corrupted input must be valid JSON");
            }
        }
    }

    #[test]
    fn numeric_corruption_no_panic(input in numeric_corruption_input()) {
        // The absolute minimum guarantee: no panic.
        let _ = json_repair_core::repair_json(&input);
    }

    #[test]
    fn corrupt_number_in_json_value(
        raw_key in "[a-z]{1,6}",
        corrupt_num in numeric_corruption_input(),
    ) {
        let input = format!(r#"{{"{raw_key}": {corrupt_num}}}"#);
        let _ = json_repair_core::repair_json(&input);
    }

    #[test]
    fn corrupt_number_in_array(corrupt_num in numeric_corruption_input()) {
        let input = format!("[{corrupt_num}]");
        let _ = json_repair_core::repair_json(&input);
    }
}

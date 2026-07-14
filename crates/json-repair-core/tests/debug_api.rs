//! Tests for the `repair_json_debug` entry point.
//!
//! In debug builds this wrapper adds an idempotence assertion; in release
//! builds it is identical to `repair_json`.  These tests verify the public
//! contract regardless of build profile.

use json_repair_core::{repair_json, repair_json_debug};

#[test]
fn debug_wrapper_matches_repair_json() {
    let inputs = [
        r#"{'key': 'value'}"#,
        r#"{"a": 1, "b": "two"}"#,
        r#"{"missing": "close brace""#,
        r#"[1, 2, 3,]"#,
        r#"{"key": "value with "embedded" quotes"}"#,
        "",
        "   ",
        "true",
        "null",
        "42",
    ];
    for input in &inputs {
        let plain = repair_json(input);
        let debug = repair_json_debug(input);
        assert_eq!(
            plain.is_ok(),
            debug.is_ok(),
            "ok-ness mismatch for input {input:?}"
        );
        if let (Ok(p), Ok(d)) = (plain.as_ref(), debug.as_ref()) {
            assert_eq!(p, d, "result mismatch for input {input:?}");
        }
    }
}

#[test]
fn debug_wrapper_handles_broken_patterns() {
    // A representative sample of malformed JSON that must repair successfully
    // and (in debug builds) pass the idempotence check without panicking.
    let broken = [
        r#"{"a": 1, "b": 2,}"#,
        r#"{'single': 'quotes'}"#,
        r#"{key: "unquoted key"}"#,
        r#"["unclosed array"#,
        r#"{"""triple""": """value"""}"#,
        r#"{"a" "b"}"#,
        r#"[1 2 3]"#,
    ];
    for input in &broken {
        let result = repair_json_debug(input)
            .unwrap_or_else(|e| panic!("repair_json_debug failed for {input:?}: {e}"));
        // The idempotence assertion inside repair_json_debug already guards
        // the fixed-point property; here we additionally validate the output
        // parses as JSON.
        assert!(
            serde_json::from_str::<serde_json::Value>(&result).is_ok(),
            "debug wrapper output is not valid JSON: {result:?} (input: {input:?})"
        );
    }
}

#[test]
fn debug_wrapper_empty_input() {
    assert_eq!(repair_json_debug("").unwrap(), "");
}

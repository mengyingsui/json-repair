#![cfg(not(miri))]

mod helpers;

use helpers::collect_cases;

const CASES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/cases");

#[test]
fn test_all_jsonl_cases() {
    let dir = std::path::Path::new(CASES_DIR);
    let cases = collect_cases(dir);
    if cases.is_empty() {
        panic!("no JSONL cases found in {CASES_DIR}");
    }
    let mut failures = Vec::new();
    for (cat, input, idx, expected) in &cases {
        match json_repair_core::repair_json(input) {
            Ok(repaired) => match serde_json::from_str::<serde_json::Value>(&repaired) {
                Ok(actual) => {
                    if let Some(expected) = expected {
                        if &actual != expected {
                            failures.push(format!(
                                "{cat}[{idx}]: expected {expected}, got {actual}\n  input:    {input:?}\n  repaired: {repaired:?}"
                            ));
                        }
                    }
                }
                Err(e) => {
                    failures.push(format!(
                        "{cat}[{idx}]: repaired JSON invalid: {e}\n  input:    {input:?}\n  repaired: {repaired:?}"
                    ));
                }
            },
            Err(e) => {
                failures.push(format!(
                    "{cat}[{idx}]: repair_json failed: {e}\n  input: {input:?}"
                ));
            }
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} / {} cases failed:\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n---\n")
        );
    }
}

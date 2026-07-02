mod helpers;

use helpers::collect_inputs;

const CASES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/cases");

#[test]
fn test_broken_patterns_validity_and_idempotence() {
    let dir = std::path::Path::new(CASES_DIR);
    let inputs = collect_inputs(dir, "broken_patterns");
    let mut failures = Vec::new();
    for input in &inputs {
        let repaired = match json_repair_core::repair_json(input) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("repair_json failed: {e}\n  input: {input:?}"));
                continue;
            }
        };
        if let Err(e) = serde_json::from_str::<serde_json::Value>(&repaired) {
            failures.push(format!(
                "repaired JSON invalid: {e}\n  input:    {input:?}\n  repaired: {repaired:?}"
            ));
            continue;
        }
        let second = match json_repair_core::repair_json(&repaired) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("second repair failed: {e}\n  first:  {repaired:?}"));
                continue;
            }
        };
        if repaired != second {
            failures.push(format!(
                "not idempotent:\n  first:  {repaired:?}\n  second: {second:?}\n  input:  {input:?}"
            ));
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} / {} broken patterns failed:\n{}",
            failures.len(),
            inputs.len(),
            failures.join("\n---\n")
        );
    }
}

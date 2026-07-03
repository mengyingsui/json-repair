#![allow(dead_code)]

use std::path::Path;
use std::fs;

pub fn roundtrip(text: &str) -> serde_json::Value {
    let repaired = json_repair_core::repair_json(text).unwrap();
    serde_json::from_str(&repaired).unwrap()
}

pub fn collect_cases(dir: &Path) -> Vec<(String, String, usize, Option<serde_json::Value>)> {
    let mut cases = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            let ext = p.extension().is_some_and(|ext| ext == "jsonl");
            let is_bench = p.file_stem().and_then(|s| s.to_str()) == Some("bench_data");
            ext && !is_bench
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let cat = entry.path().file_stem().unwrap().to_str().unwrap().to_string();
        let content = fs::read_to_string(entry.path()).unwrap();
        for (idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let obj: serde_json::Value = serde_json::from_str(line).unwrap();
            let input = obj["input"].as_str().unwrap().to_string();
            let expected = obj.get("expected").cloned();
            cases.push((cat.clone(), input, idx, expected));
        }
    }
    cases
}

pub fn collect_inputs(dir: &Path, name: &str) -> Vec<String> {
    let path = dir.join(format!("{name}.jsonl"));
    let content = fs::read_to_string(&path).unwrap();
    let mut inputs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let obj: serde_json::Value = serde_json::from_str(line).unwrap();
        inputs.push(obj["input"].as_str().unwrap().to_string());
    }
    inputs
}

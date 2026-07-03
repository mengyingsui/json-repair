use std::fs;
use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
struct BenchEntry {
    label: String,
    input: String,
    expected_valid: bool,
}

fn load_entries() -> Vec<BenchEntry> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cases/bench_data.jsonl");
    let content = fs::read_to_string(&path).unwrap();
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn bench_repair(c: &mut Criterion) {
    let entries = load_entries();
    let mut group = c.benchmark_group("json_repair");

    for entry in &entries {
        group.bench_with_input(
            BenchmarkId::new(&entry.label, entry.input.len()),
            &entry.input,
            |b, input| {
                b.iter(|| {
                    let repaired = json_repair_core::repair_json(black_box(input)).unwrap();
                    let _ = serde_json::from_str::<serde_json::Value>(&repaired);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_repair);
criterion_main!(benches);

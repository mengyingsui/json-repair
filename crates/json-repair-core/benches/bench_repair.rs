use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_repair_json(c: &mut Criterion) {
    let input = r#"{
        "response": "The user said \"hello\" and I replied \"hi there\"",
        "sentiment": "positive",
        "code": "def greet(name):\n    print(f\"Hello, {name}\")"
    }"#;
    c.bench_function("repair_json", |b| {
        b.iter(|| json_repair_core::repair_json(black_box(input)))
    });
}

fn bench_long_input(c: &mut Criterion) {
    let long = "A".repeat(10000);
    let input = format!("{{\"key\": \"{long}\"}}");
    c.bench_function("repair_json_long", |b| {
        b.iter(|| json_repair_core::repair_json(black_box(&input)))
    });
}

criterion_group!(benches, bench_repair_json, bench_long_input);
criterion_main!(benches);

# Rust Benchmarks

The Rust and Python benchmarks both read from the **shared data file**
[`tests/cases/bench_data.jsonl`](../../tests/cases/bench_data.jsonl), which
contains 19 test cases (11 fixable + 8 unfixable) of varying sizes and
complexity. This ensures identical input data for fair cross-language
comparison.

Benchmarks are dynamically generated — each line in the JSONL file becomes
a named criterion benchmark (e.g. `passthrough_valid/48`,
`unfixable_semicolons_large/2980`).

Each iteration measures: **repair + validation** (`repair_json()` followed
by `serde_json::from_str()`), which captures the full cost of detecting
whether the input can be repaired into valid JSON.

## Run

```bash
cargo bench -p json-repair-core

# Filter by pattern
cargo bench -p json-repair-core --bench bench_repair "semicolons"

# HTML report (requires gnuplot or plotters)
cargo bench -p json-repair-core --features html_reports
```

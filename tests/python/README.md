# Performance Benchmarks

This directory contains only performance benchmarks. All functional/fuzz
testing is handled by the Rust test suite (`cargo test -p json-repair-core`).

## File

| File                  | Description                                                                                               |
|-----------------------|-----------------------------------------------------------------------------------------------------------|
| `test_performance.py` | Micro-benchmarks via `pytest-benchmark` — runs every entry from `bench_data.jsonl` through `repair_json`. |
| `__init__.py`         | Package init.                                                                                             |

## Run

```bash
# Performance benchmarks only
uv run pytest tests/python/test_performance.py --benchmark-only

# Benchmark histogram
uv run pytest tests/python/test_performance.py --benchmark-histogram
```

Python functional tests have been removed. All correctness/fuzz testing lives
in the Rust crate at `crates/json-repair-core/tests/`.

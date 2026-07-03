# Python Tests

## Files

| File | Description |
|------|-------------|
| `test_repair.py` | Parametrized JSONL tests — reads `tests/cases/*.jsonl` and verifies every repair case. |
| `test_hypothesis.py` | Property-based fuzzing — valid JSON passthrough, broken→valid idempotence. |
| `test_performance.py` | Micro-benchmarks via `pytest-benchmark` (18 scenarios across 5 classes). |
| `test_adjacent_objects.py` | Adjacent-object wrapping (`{...},{...}` → `[{...},{...}]`). |
| `test_complex_scenarios.py` | Realistic LLM JSON with embedded quotes, code blocks. |
| `test_control_characters.py` | Literal `\r` in string values. |
| `test_edge_cases.py` | Empty input, very long string passthrough. |
| `test_implicit_array.py` | Implicit sequence detection (≥8KB, ≥3 objects). |
| `test_large_embedded_quotes.py` | Multi-segment repair of large strings with quotes. |
| `test_misordered_brackets.py` | Swapped `]`/`}` and extra bracket handling. |
| `test_return_object.py` | `return_object=True` path (JSON load after repair). |
| `test_unterminated_string.py` | Missing closing quote repair. |
| `_helpers.py` | Shared assertion helpers for all test files. |

## Run

```bash
# All tests
uv run pytest tests/python/ -v

# Performance benchmarks only
uv run pytest tests/python/test_performance.py --benchmark-only

# Benchmark histogram
uv run pytest tests/python/test_performance.py --benchmark-histogram
```

The Rust extension is built automatically by `uv sync`. No Python fallback exists.

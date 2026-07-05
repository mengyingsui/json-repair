# Rust Integration Tests

## Files

| File | Description |
|------|-------------|
| `jsonl_cases.rs` | Scans `tests/cases/*.jsonl` and tests every case with `expected` (mirrors Python `test_repair.py`). |
| `broken_patterns.rs` | Parametrized validity + idempotence checks on hand-crafted broken inputs. |
| `scenario.rs` | Realistic scenarios: LLM output, implicit arrays, bracket swaps, etc. (14 tests). |
| `preprocess.rs` | Tests `fix_colon_in_key` and `fix_mixed_quotes` pre-processors. |
| `edge_cases.rs` | Empty input, very long string passthrough, fuzzer crash regression tests (multi-E, invalid `\u`, control chars, surrogate escape, backslash-at-EOF, deeply nested brackets). |
| `control_chars.rs` | Literal `\r` carriage return in string content. |
| `large_embedded.rs` | Multi-segment repair with many embedded unescaped quotes. |
| `unterminated_string.rs` | Missing closing quote at end of input. |
| `helpers.rs` | Shared utilities (`assert_valid_json`, `assert_repair_eq`, etc.). |

## Run

```bash
# All tests
cargo test -p json-repair-core

# Single file
cargo test -p json-repair-core --test scenario

# With output
cargo test -p json-repair-core -- --nocapture
```

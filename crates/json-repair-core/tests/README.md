 # Rust Integration Tests

## Files

| File                            | Description                                                                                                                                                                  |
|---------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `jsonl_cases.rs`                | Scans `tests/cases/*.jsonl`. Lines with `expected` assert exact output; lines without `expected` verify output is valid JSON.                                                |
| `broken_patterns.rs`            | Parametrized validity + idempotence checks on hand-crafted broken inputs.                                                                                                    |
| `scenario.rs`                   | Realistic scenarios: LLM output, implicit arrays, bracket swaps, etc. (5 tests).                                                                                             |
| *(covered by `jsonl_cases.rs`)* | The fused single-pass preprocessor is exercised by JSONL test cases (`mixed_quotes`, `colon_in_key`).                                                                        |
| `edge_cases.rs`                 | Empty input, very long string passthrough, fuzzer crash regression tests (multi-E, invalid `\u`, control chars, surrogate escape, backslash-at-EOF, deeply nested brackets). |
| `large_embedded.rs`             | Multi-segment repair with many embedded unescaped quotes.                                                                                                                    |
| `unterminated_string.rs`        | Missing closing quote at end of input.                                                                                                                                       |
| `corpus_scan.rs`                | Scans `fuzz/corpus/repair/` and validates every seed produces balanced JSON output.                                                                                          |
| `proptests.rs`                  | Property-based random tests (proptest): valid JSON passthrough, idempotence, string content preservation, number corruption.                                                 |
| `helpers.rs`                    | Shared utilities (`roundtrip`, `collect_cases`, etc.).                                                                                                                       |

## Run

```bash
# All tests
cargo test -p json-repair-core

# Single file
cargo test -p json-repair-core --test scenario

# With output
cargo test -p json-repair-core -- --nocapture
```

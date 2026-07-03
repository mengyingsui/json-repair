# Changelog — json-repair-core

## v0.1.2 (2026-07-03)

### Security
- **Depth violation → `Err`** — exceeding `MAX_PARSE_DEPTH=512` now returns
  `Err(JsonRepairError)` instead of silently emitting `null`.
- **Numeric corruption detection** — non-numeric characters immediately
  following a number token now trigger `Err` instead of silent split.
- **Explicit `ParserState` enum** — escape handling in `parse_string`,
  `parse_single_quoted_string`, and `parse_triple_string` now uses a formal
  state machine (`Normal` / `InString` / `InStringEscaped`).
- **Fuzz testing** — `cargo-fuzz` target for random-input robustness.

### Added
- `ParserState` enum with three variants.
- `error: Option<JsonRepairError>` field on `Repairer`, enabling error
  propagation from sub-methods without cascading signature changes.
- Fuzz target at `fuzz/fuzz_targets/repair.rs`.

### Changed
- `Repairer::repair()` return type: `String` → `Result<String, JsonRepairError>`.
- Main loop now checks `self.error` before each frame pop — errors from
  sub-methods abort the parse immediately.
- All three string-parsers use `match self.state` with explicit transitions.

## v0.1.1 (2026-07-03)

### Security
- **Recursion depth limit** (`MAX_PARSE_DEPTH=500`) — prevents stack overflow on
  deeply nested input. `parse_value` now wraps itself with a depth counter;
  exceeding the limit emits `null` and returns.

### Fixed
- `out_chars` byte tracking in `parse_string` — `-= 1` → `-= c.len_utf8()` for
  correct non-ASCII suffix-junk cleanup.
- `skip_prefix_junk` redundant String allocations — rewritten to use index-based
  operations on `Vec<char>`, eliminating intermediate `String` copies.
- `peek_str` per-call allocation — replaced with `peek_is(&str)`, zero-allocation
  pattern matching.
- `parse_number` slow `f64::parse` on very long digit sequences (>100 chars now
  skips the parse and emits the number directly).

### Added
- Rustdoc documentation for all public API items.
- `clippy.toml` (MSRV = 1.85).
- `deny.toml` (cargo-deny license/advisory checks).
- Workspace-level metadata (`Cargo.toml` now uses `workspace.package`).

## v0.1.0 (2026-07-03)

### Added
- Initial release. Single-pass Rust state machine for repairing malformed JSON
  from LLM outputs.
- Full test suite: 24 integration tests, criterion benchmarks.
- `repair_json()` entry point, `JsonRepairError` type.
- Pre-processing: `fix_colon_in_key`, `fix_mixed_quotes`.
- See Python package v0.3.0 for full feature list.

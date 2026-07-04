# Changelog — json-repair-core

## v0.1.6 (2026-07-04)

### Changed
- Cargo.lock tracked for reproducible CI builds.

### Fixed
- **Trailing-comma at EOF** — fuzzer-discovered: `array_loop` and
  `object_loop` now strip trailing commas before breaking at end-of-input
  (e.g. `[12,\n` → `[12]`, not `[12,]`).

### Security
- No new security features; all guarantees from v0.1.4+ preserved.

## v0.1.5 (2026-07-04)

### Changed
- **Module system** — single `repairer.rs` (1216 lines) split into 7 submodules:
  `string`, `number`, `literal`, `keys`, `structure`, `comment`, `junk`.
- Pre-processing (`fix_colon_in_key`, `fix_mixed_quotes`) moved to `preprocess.rs`.

### Added
- `serde-validate` Cargo feature (enabled by default) — makes `serde_json`
  optional.  Disable with `--no-default-features` to remove the dependency.
- Debug-only assertions (`debug_assert!`) throughout the codebase — byte counter
  sync, bracket balance, output valid-JSON checks, string closure invariants.
- `repair_json_debug` public function — wraps `repair_json` with extra
  idempotence and output validation checks (compiles away in release builds).

## v0.1.4 (2026-07-04)

### Security
- **Number normalisation** — leading zeros in numbers are stripped
  (`"000"` → `"0"`, `"-001"` → `"-1"`) so emitted JSON conforms to RFC 8259
  (which forbids leading zeros). Previously `f64::parse()` accepted
  non-conformant numbers which were emitted verbatim.

### Changed
- `parse_number` now validates via `serde_json::from_str` instead of
  `f64::parse()` — correctly rejects non-JSON-conformant numbers that
  `f64` would silently accept.
- `is_closing_quote` — `:` after an unquoted key is now treated as part of
  the key-value pair when the parser expects a key. Comma-before-value
  detection tightened to check more reserved characters (`true`, `false`,
  `null`, `-`, digits) before declaring the quote as closing.

### Fixed
- Leading-zero numbers (`000`, `-001`, `00.5`) now emit valid JSON.

### Added
- Numeric-corruption property tests (4 new proptest functions) covering
  number+junk suffix, multiple decimal points, hex-like, multiple signs,
  malformed scientific notation, embedded spaces, and lone operators.

## v0.1.3 (2026-07-04)

### Added
- 10 new Rust integration tests in `tests/prefix_junk.rs` for metadata tag
  skipping, code fence handling, and link-pattern skipping.
- `CLOSING_CHARS` const in `is_closing_quote` for readability.

### Changed
- `fix_colon_in_key` and `fix_mixed_quotes` return `Cow<'_, str>` — zero
  allocation when input has no matching patterns.

### Fixed
- `skip_prefix_junk` now detects `[TEXT_*]` metadata tags, skips non-JSON
  code fences, preserves JSON inside ` ```json ``` ` fences.
- `peek_is` uses `s.chars().count()` instead of `s.len()` for correct
  non-ASCII character offset calculation.
- `skip_prefix_junk` link-depth loop variable mutability.

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

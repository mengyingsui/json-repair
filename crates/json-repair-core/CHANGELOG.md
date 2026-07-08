# Changelog — json-repair-core

## v0.1.8 (2026-07-08)

### Changed
- **Triplicated string-loop extraction** (`string.rs`) — `emit_string_body_char`,
  `handle_escaped`, `BodyAction` enum extracted from the identical character
  loops in `parse_string`, `parse_triple_string`, `parse_single_quoted_string`.
- **Escape-logic deduplication** (`keys.rs` + `string.rs`) — `emit_unquoted_char`
  unifies escape branches across key and value unquoted-string parsing.
- **`object_loop` splitting** (`structure.rs`) — `is_value_start`,
  `is_key_start`, `looks_like_key` extracted from nested condition block.
- **`trim_trailing_comma` helper** (`repairer/mod.rs`) — eliminates 6+
  repetitions of `if out.ends_with(',')` across the codebase.
- **`emit_unicode_escape` helper** (`repairer/mod.rs` + `string.rs`) —
  centralises `write!` + byte-counter pattern.
- **Constants for magic numbers** (`string.rs`) — `CONTROL_CHAR_MAX` (0x20),
  `SURROGATE_LO` (0xD800), `SURROGATE_HI` (0xDFFF), `METATAG_MAX_LEN` (64).
- **`skip_prefix_junk` Vec clone eliminated** (`junk.rs`) — common path no
  longer clones the full `self.chars` vector.
- **`peek_is` `chars().count()` → `s.len()`** (`repairer/mod.rs`) — ASCII
  patterns now use byte-length comparison with `debug_assert!`.
- **Preprocess `Cow::Borrowed` fast-path** (`preprocess.rs`) — zero allocation
  when input has no colon-in-key or mixed-quote patterns.

### Performance
- **`parse_literal`** (`literal.rs`) — `collect::<String>().to_lowercase()` dual
  heap allocation replaced with per-char case-insensitive `match_lit()`.
- **`emit_escape` hex parsing** (`string.rs`) — `collect<String>()` +
  `from_str_radix` replaced with `to_digit(16)`-based bit-shift accumulation.
- **`.contains()` → `matches!()`** on all hot paths — compiler generates
  jump tables instead of loop-searches.
- **39–82% speedup** on corrupted benchmarks vs v0.1.7.

## v0.1.7 (2026-07-05)

### Added
- **Fuzzer crash regression tests** — JSONL entries and Rust tests for 7
  crash patterns: invalid `\u`, multi-E, control chars, trailing comma,
  misordered brackets, surrogate escape, backslash-at-EOF, deeply nested
  brackets (128 & 400 levels).
- `test_deeply_nested_128_brackets` — reproduces exact fuzzer stack overflow.
- `test_backslash_at_eof_in_string` — reproduces `debug_assert!` false positive.

### Changed
- **`is_output_balanced`** (`mod.rs:255`) — `debug_assert!` replaced with
  `Err(JsonRepairError)`. Bracket imbalance returns a proper error instead
  of panicking in debug builds.
- **serde_json validation** (`mod.rs:261-274`) — `de.disable_recursion_limit()`
  removed; `bracket_depth <= 100` guard skips validation on deeply nested
  output (prevents ASAN `stack-overflow` and `STATUS_STACK_BUFFER_OVERRUN`).
- **`emit_escape`** (`string.rs:12-31`) — `\uXXXX` in surrogate range
  (0xD800–0xDFFF) now emits `\ufffd` instead of the raw surrogate.
- **`validate_number`** — removed length-bypass; all numbers validated via
  `serde_json` for consistent RFC 8259 conformance.
- **`structure.rs`** — `object_loop`/`array_loop` bracket handlers `match`
  on bracket stack before appending.

### Fixed
- **Multi-E** — `12.34E567E+0E12` no longer emits invalid JSON.
- **Invalid `\u` escape** — non-hex after `\u` emits `\\u` literal.
- **Control chars in unquoted values** — `\x00` inside bare-word values
  properly `\uXXXX`-escaped.
- **Surrogate escapes** — `\uD800`–`\uDFFF` emitted as `\ufffd`.
- **Backslash at EOF in string** — `\"` at output end no longer triggers
  `debug_assert!` false positive.
- **serde_json stack overflow** — 128-level bracket nesting guarded at 100.
- **Bracket-misorder in arrays** — `]` before `}` correctly closes object first.

### Security
- **Stack overflow guard** — serde_json recursion limit bypass removed;
  deeply nested output (>100 brackets) skips validation instead of crashing.
- **Surrogate sanitisation** — raw surrogate code points no longer emitted.
- All guarantees from v0.1.4+ preserved.

## v0.1.6 (2026-07-04)

### Changed
- Cargo.lock tracked for reproducible CI builds.
- Python binding (`json-repair-python`) updated for pyo3 0.29 —
  `py.allow_threads()` replaced with `py.detach()`.

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

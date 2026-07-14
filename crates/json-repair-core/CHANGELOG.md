# Changelog — json-repair-core

## v0.3.0 (2026-07-14)

### Changed
- **Focused sub-struct composition** (P2) — sub-modules `comment.rs`, `junk.rs`,
  `number.rs`, `string.rs`, `keys.rs`, `literal.rs` converted from `impl Repairer`
  to free functions with explicit parameters. `ParserState` moved into `string.rs`
  as a private enum.
- **`Repairer`** (`repairer.rs`) — `state: ParserState` field removed. Struct
  now holds only `input`, `output`, `brackets`.
- **`emit_unicode_escape`** (`output_buffer.rs`) — replaced 4×
  `char::from_digit().unwrap()` with zero-cost hex lookup table.
- **`BracketStack`** (`bracket_stack.rs`) — removed hard-coded `520` initial
  capacity; uses `Vec::new()`.
- **`parse_string`** (`string.rs`) — NUL key-split path now also calls
  `try_split_bareword_after_value` for bareword recovery.

### Fixed
- **`unescaped_quotes[11]`** — `check_closing_quote` `nc == ':'` branch
  extended backward scan for `{`/`[` to handle nested embedded JSON keys.
- **`edge_cases[64]`** — `_fix_expected.py` corrected to emit `\u0000` as
  6-char literal, not raw NUL byte.

## v0.2.1 (2026-07-13)

### Performance
- **Eliminated `is_output_balanced` output scan** (`repairer.rs`) — replaced
  the O(n) post-repair bracket balance scan with an O(1) `bracket_depth != 0`
  check. `Repairer` now tracks bracket depth live via `brackets_push`/`brackets_pop`,
  eliminating the only output traversal in `repair()`.
- **Implicit array `[` lives on bracket stack** (`structure.rs`) — implicit
  array opening bracket is now pushed via `brackets_push(']')` and closed by
  `close_brackets()` in LIFO order, instead of emitting `]` directly in
  `implicit_array_loop()`. This ensures correct `}`-before-`]` ordering without
  a separate output scan.
- **`memchr2`-accelerated string scanning** (`junk.rs`) — `scan_string` and
  the string-body loop in `is_implicit_object_sequence` now use `memchr2`
  to skip past long runs without `"`/`\` via SIMD, reducing character-level
  dispatch overhead.

### Removed
- **`is_output_balanced` function** (`repairer.rs`) — the `is_output_balanced`
  function and its helper `closing_for` were removed. Bracket balance is now
  guaranteed by the live `bracket_depth` counter.

### Changed
- **`Repairer`** (`repairer.rs`) — new `bracket_depth: i32` field initialized to 0.
- **`brackets_push`** — increments `bracket_depth`.
- **`brackets_pop`** — decrements `bracket_depth`.
- **`repair()`** (`repairer.rs`) — implicit array opens now call `brackets_push(']')`;
  final `bracket_depth != 0` replaces `is_output_balanced(&out)`.
- **`implicit_array_loop()`** (`structure.rs`) — no longer emits `]`;
  closing bracket is emitted by `close_brackets()` via the bracket stack.

## v0.2.0 (2026-07-12)

### BREAKING
- **Removed `fix_colon_in_key` and `fix_mixed_quotes`** from the public API.
  These standalone pre-processing functions are now private implementation
  details of the fused `preprocess_json`.  Users of these functions should
  call `repair_json` instead, which already runs the fused preprocessor
  internally.  The standalone `needs_colon_fix` helper is also removed.

### Added
- **`yes`/`no`/`nil`/`nullptr` literal support** (`literal.rs`) —
  `"yes"` → `true`, `"no"` → `false`, `"nil"` / `"nullptr"` → `null`.

### Performance
- **Single-pass preprocessor fusion** (`preprocess.rs`) — `preprocess_json`
  applies both mixed‑quote and colon‑in‑key transforms in a single forward
  scan, fixing a correctness bug where the colon scanner ate `','` boundary
  bytes. Internal helpers `needs_colon_fix`/`fix_colon_in_key`/
  `fix_mixed_quotes` replaced by `try_mixed_quote_boundary`/
  `emit_mixed_quote_boundary`/`try_fix_colon_in_key`.
- **ASCII byte fast path** (`cur()`/`char_at()`) — avoids UTF-8 decoder setup for 99%+ JSON chars.
- **`is_implicit_object_sequence` full scan** (`junk.rs`) — removed 64KB
  scan limit (`IMPLICIT_SEQUENCE_MAX_SCAN`); examines the entire remaining
  input.

### Code Quality
- **State-machine v2 redesign** — all implicit contracts between `Repairer` fields eliminated:

  - `ParseFrame` redesign: `ObjectLoop(usize)`, `ArrayLoop(usize)`, `ImplicitArrayLoop(usize)` replace `ResumeObject { prev_expect }`/`ResumeArray`/`ResumeImplicitArray { first }` — frame-local `usize` counter replaces global `expect_key`/`just_emitted_value` flags.
  - 3 field deletions (`expect_key`, `just_emitted_value`, `lookahead_pos`) eliminate 15 implicit synchronization contracts.
  - 3 resume methods deleted (`resume_object`/`resume_array`/`resume_implicit_array`).
  - `run_value` refactored: pushes `ObjectLoop`/`ArrayLoop`/`ImplicitArrayLoop` frames and returns, no longer recursively calls `object_loop`/`array_loop`/handles `expect_key` context.
  - `check_closing_quote(&self, is_key: bool) -> (bool, Option<usize>)` returns tuple instead of writing to `self.lookahead_pos`.
  - `try_split_bareword_after_value(bareword_quote_pos)` accepts parameter.
  - `parse_string(is_key: bool)` accepts context flag.
  - `needs_comma_in_output()` pure byte check replaces `needs_separator(first)`.
  - `skip_prefix_junk` → `normalize_preamble`.

### Changed
- **`repair_json_debug`** (`lib.rs`) — no longer `#[cfg(debug_assertions)]`-gated;
  always compiled; `#[cfg(debug_assertions)]` guards only the assertions inside.
- **`repair_json`** (`lib.rs`) — calls `preprocess::preprocess_json` directly.
- **`emit_unicode_escape`** (`repairer.rs`) — manual `hex_nibble` lookup replaces
  `write!(…, "\\u{:04x}", …)`.
- **`is_output_balanced`** (`repairer.rs`) — uses fixed-size `[char; STACK_CAPACITY]`
  stack instead of `Vec`; gated to `#[cfg(debug_assertions)]`.
- **`INITIAL_OUTPUT_CAP`** constant (`repairer.rs`) — named constant replaces inline `1 << 18`.
- **`peek()` method removed** (`repairer.rs`) — no longer used.
- **Number scanning** (`number.rs`) — uses byte-level `text.as_bytes()[self.i]` matching;
  `+` only accepted after `e`/`E`; `just_emitted_value = true` removed.
- **`normalize_leading_zeros_inplace`** (`number.rs`) — removed `b'+'` prefix handling.
- **`emit_escape` hex validation** (`string.rs`) — `all(|b| b.is_ascii_hexdigit())`
  replaced with per-char `to_digit(16)` loop; invalid hex now emits `\\` + char
  instead of bare `\\u`.
- **`handle_double_quote_escape` / `ensure_closing_quote`** (`string.rs`) —
  extracted into dedicated methods.
- **`close_bracket` / `try_consume_mismatched_bracket` / `push_container`**
  (`structure.rs`) — extracted from inline logic.
- **`is_value_start` removed** (`structure.rs`) — no longer used.
- **`is_implicit_object_sequence`** (`junk.rs`) — full scan; `IMPLICIT_SEQUENCE_MIN_LENGTH`
  8192→128, `IMPLICIT_SEQUENCE_MIN_COUNT` 3→2, `METATAG_MAX_LEN` 64→128.
- **`scan_string` / `try_skip_code_fence` / `try_skip_metatag_or_link` / `utf8_char_len`**
  (`junk.rs`) — new helper methods extracted from `normalize_preamble`.
- **`parse_unquoted_key`** (`keys.rs`) — `ch.is_ascii()` replaces `(ch as u32) < 128`.
- **`tests/preprocess.rs` deleted** — standalone preprocessor tests removed
  (coverage via JSONL test cases).
- **`is_comment_start`** (`comment.rs`) — doc fix: `recognised` → `recognized`.
- **`close_bracket` assert → `debug_assert!`** (`structure.rs`) — bracket‑stack
  overflow handled by `MAX_PARSE_DEPTH=512` guard; runtime check relaxed to debug-only.
- **Backward scan in `check_closing_quote`** (`string.rs`) — scans backward past
  whitespace; if `{`/`[` found, returns `(false, None)` instead of false positive.
- **`is_output_balanced` called at end of `repair()`** (`repairer.rs`) — always-on
  validation to catch unbalanced bracket output (runs in all build profiles).
  Balanced output is an API guarantee — `repair_json` returns `Err` on imbalance.

### Test Infrastructure
- **JSONL auto-discovery refactored** (`helpers.rs`) — `broken_patterns.jsonl`,
  `large_embedded.jsonl`, `unterminated_string.jsonl` excluded from `collect_cases`;
  each has dedicated Rust tests with stronger assertions.
- **Overlapping test coverage eliminated** — coverable static inputs migrated from
  `edge_cases.rs` to JSONL; duplicate JSONL rows removed.
- **`tests/README.md` updated** — corrected `jsonl_cases.rs` description; added
  `proptests.rs` entry.

### Fixed
- **`repair_json_debug` misleading idempotence message** (`lib.rs`) — `unwrap_or_default()`
  replaced with `match` so a second‑repair failure produces a clear error message
  instead of a confusing "not idempotent" assertion.
- **`brackets_push` release‑mode safety** (`repairer.rs`) — `debug_assert!` promoted to
  `assert!` so bracket‑stack overflow panics in all build profiles, not just debug.
- **`test_comma_separated_objects`** (`scenario.rs`) — assertion corrected:
  comma-separated objects wrapped in array (20 elements), was incorrectly
  checking `result.is_object()`.

## v0.1.10 (2026-07-12)

### Performance
- **ASCII byte fast path** (`cur()`/`char_at()`) — avoids full UTF-8 decoder for the 99%+ of JSON chars that are ASCII. Checks `text.as_bytes()[i].is_ascii()` first, falls back to `chars().next()` only for multibyte chars.
- **`needs_colon_fix` fusion** (`preprocess.rs`) — returns `Option<usize>`, so `fix_colon_in_key` jumps directly to the first problematic `"` instead of rescanning from byte 0.
- **Redundant bareword scan eliminated** (`string.rs`) — `is_closing_quote` now `&mut self` and caches the position of the `"` after a bareword lookahead in `self.lookahead_pos`. `parse_string` checks the cache before doing its own bareword scan, avoiding a second traversal.
- **`validate_number` single scan** (`number.rs`) — the identical multi-period/multi-exponent pre-check was duplicated across both `cfg` variants. Extracted as `has_excessive_separators` — a single byte scan (`for &b in s.as_bytes()`).
- **`match_lit` byte comparison** (`literal.rs`) — `eq_ignore_ascii_case` on `&[u8]` slices eliminates per-character UTF-8 decoding.
- **Output buffer** 4096 → 65536 (`repairer.rs`).
- **`is_implicit_object_sequence` early exit** (`junk.rs`) — stops scanning after 65536 bytes. New constant `IMPLICIT_SEQUENCE_MAX_SCAN`.
- **`*/` detection** (`comment.rs`) — `starts_with(b"*/")` replaces `cur() == '*' && char_at(i+1) == '/'`.
- **Triple backtick** (`junk.rs`) — `starts_with(b"```")` in both fence-open and fence-close checks.
- **Metatag content validation** (`junk.rs`) — `bytes().all(|b| ...)` replaces `chars().all(|c| ...)` (all valid chars are ASCII).
- **Preprocess ASCII fast path** (`preprocess.rs`) — `fix_colon_in_key` and `fix_mixed_quotes` now do a byte-indexed ASCII check before `text[i..].chars().next()`.
- **`object_loop` edge check** (`structure.rs`) — `{`/`:` skip guarded behind `self.expect_key`.
- **`\u{200b}` separated from `matches!`** (`keys.rs`) — the zero-width space was the only non-ASCII variant, preventing the compiler from using a bitmask optimization.
- **`emit_escape` hex validation** (`string.rs`) — `all(|b| b.is_ascii_hexdigit())` on `self.text.as_bytes()[i+1..i+5]`.
- **`needs_separator`** (`repairer.rs`) — three `ends_with` calls (`','`, `'{'`, `'['`) replaced with single `matches!` byte lookup.

### Code Quality
- **`debug_assert!` → runtime recovery** (`string.rs`) — state-postcondition checks upgraded from `debug_assert!` to actual runtime correction: `self.state = ParserState::Normal` reset and missing closing-quote auto-insertion.
- **`Some(']')` arm fixed** (`structure.rs`) — `object_loop`'s dead `Some(']')` arm now restores `self.expect_key = prev_expect` (consistent with `Some('}')` arm) and carries a defensive comment.
- **`parse_number` error position** (`number.rs`) — error position changed from `start` to `self.i` so it correctly points at the contaminating non-numeric character.
- **Output buffer capacity** (`repairer.rs`) — raised from 65536 to 262144 (1&lt;&lt;18) to reduce reallocations on large non-ASCII inputs.
- **`out_chars` field removed** — this redundant byte counter duplicated `String::len()` (O(1)). Removed 21 occurrences across `repairer.rs`, `string.rs`, `structure.rs`. All `debug_assert_eq!(out.len(), out_chars)` sync checks deleted.
- **Literal pattern constants** (`literal.rs`) — `LIT_TRUE`, `LIT_FALSE`, `LIT_NULL`, `LIT_NONE`, `LIT_UNDEFINED`, `LIT_NAN`, `LIT_INFINITY`, `LIT_POS_INF`, `LIT_NEG_INF` — use `.len()` instead of hardcoded byte lengths (4, 5, 4, 9, 3, 8, 9).
- **Magic number naming** — `STACK_OVERHEAD` (8, was inline in `STACK_CAPACITY`), `MAX_VALIDATION_DEPTH` (100), `IMPLICIT_SEQUENCE_MIN_COUNT` (3).
- **`emit_bare_word` helper** (`keys.rs`) — extracted the shared char-loop from `parse_unquoted_key` and `parse_unquoted_value`.
- **Redundant doc link fixed** — `[Repairer](self::Repairer)` → `[Repairer]`.
- **Collapsed nested `if`** — clippy fix in `structure.rs`.
- **`MaybeUninit`→`Option`** (`repairer.rs`) — `[MaybeUninit<ParseFrame>; 520]` replaced with `[Option<ParseFrame>; 520]`, eliminating the only `unsafe` in the crate. Zero overhead (niche optimization keeps `Option<ParseFrame>` at 8 bytes).
- **`skip_ws`/`skip_ws_at` byte path** — `text.as_bytes()[i].is_ascii_whitespace()` replaces `char_at` call, eliminating UTF-8 decode overhead for whitespace scanning.

### Tests
- **JSONL data migration** — inline `#[cfg(test)]` data from `prefix_junk.rs`, `control_chars.rs`, `scenario.rs`, `edge_cases.rs`, `number.rs` moved to `tests/cases/*.jsonl`. Test files `prefix_junk.rs` and `control_chars.rs` deleted; `CORPUS_INPUTS` and 8 static-input tests and 6 leading-zero tests removed.
- **UTF-8 BOM removed** — `complex_scenarios.jsonl` and `triple_quoted.jsonl` had BOM bytes that silently broke `serde_json::from_str`.
- **Fuzz corpus extended** — 222 seed inputs drawn from JSONL test cases added to `fuzz/corpus/repair/`.
- **Doc audit** — 4 doc fixes: `peek_is` panic note, `Repairer::new` pre-decompose removal, `skip_suffix_junk` trim scope, `is_key_start` comment exclusion.

## v0.1.9 (2026-07-09)

### Changed
- **`#![deny(missing_docs)]` enforced** (`lib.rs`) — any new public item
  missing a doc comment is now a compile error.
- **Module-level docs** — all 11 modules (`lib`, `error`, `preprocess`,
  `repairer/mod`, `comment`, `junk`, `keys`, `literal`, `number`, `string`,
  `structure`) now have `//!` doc comments describing their purpose.
- **Internal method docs** — all `pub(crate)`/`pub(super)` methods
  (`Repairer::new`/`peek`/`emit_char`/`skip_ws`/`close_brackets`/`emit_str`/
  `repair`/`is_output_balanced`, `skip_comment`, `skip_suffix_junk`/
  `is_implicit_object_sequence`, `parse_key`/`parse_unquoted_key`/
  `parse_unquoted_value`, `parse_literal`, `parse_number`,
  `emit_escape`/`is_closing_quote`/`parse_string`/`parse_triple_string`/
  `parse_single_quoted_string`, `array_loop`/`resume_implicit_array`/
  `implicit_array_loop`) now have `///` doc comments.
- **`Repairer` struct + fields documented** — all 11 fields have `///` docs.
- **`ParserState`/`ParseFrame` variants documented** — all 3+4 enum variants.
- **`is_closing_quote` refactoring** (`string.rs`) — 200-line monolith split
  into 6 focused functions:
  - `lookahead_ws` (8 lines, `#[inline]`) — skip whitespace, return
    `(pos, char)`.
  - `comma_ok` (10 lines, `#[inline]`) — validate comma followed by value.
  - `embedded_quote_guard` (17 lines, `#[inline]`) — `]`/`}` embedded-quote
    detection (3 sub-cases).
  - `looks_like_real_quote_terminator` (35 lines) — string-aware bracket
    balance scan (cold path, not inlined).
  - `bare_key_chain` (12 lines, `#[inline]`) — unquoted key `word"…":` chain.
  - Main `is_closing_quote` (20 lines) — thin dispatcher.
  Branch order restored: structural punctuation (`,}] \n`) checked first.
- **Doc fixes** — `match_lit` doc corrected (returns `bool`, not "length");
  `keys.rs` module doc fixed (removed incorrect "colon insertion");
  `number.rs` module doc fixed (removed incorrect "hex");
  `is_closing_quote` doc expanded to cover `\n`, `{`/`[` key context,
  unquoted-key chain, and embedded-quote guard sub-cases.
- **Bench file documented** (`bench_repair.rs`) — `BenchEntry`,
  `load_entries`, `bench_repair` all have `///` docs.

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
- **Number normalization** — leading zeros in numbers are stripped
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

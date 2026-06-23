# Changelog

## v0.1.7 (2026-06-23)

### Fixed
- Double-comma regression in `_parse_object` / `_parse_array` — extra `,,`
  after string values (e.g. `")","source_turn_ids"`) now skipped by checking
  last emitted character. All 34/34 `json_failures.txt` blocks repairable
  (up from 30/34).
- `_is_closing_quote` alpha lookahead — skip `\n` in whitespace so newline
  before next key is handled correctly.

### Added
- Unbraced-object detection in `_skip_prefix_junk` — text starting with
  `"key" : value` (missing outer `{`/`}`) auto-wrapped with `{...}`.
  Repaired 18 additional `json_failures.txt` blocks (27→30→34 total).
- `tests/cases/double_commas.jsonl` — 8 test cases for double-comma patterns.
- `tests/cases/INDEX.md` — catalog of all 24 `.jsonl` test case files.

### Changed
- `tests/cases/` expanded from 22 to 24 `.jsonl` files.

## v0.1.6 (2026-06-23)

### Changed
- All code consolidated into single `_Repairer` class in `_repair.py`;
  removed `_core.py`, `_string.py`, `_value.py` mixin split.
- Test cases (80+) moved from `test_repair.py` into 22 `.jsonl` files
  under `tests/cases/`, one per category.
- Hypothesis broken-patterns list moved to `tests/cases/broken_patterns.jsonl`.
- `_extract_blocks` renamed to `extract_blocks` for cross-module import.

### Removed
- Dead function `_load_cases` from `test_repair.py`.

### Fixed
- All Pylance `reportUnusedClass` / `reportUnknownVariableType` / `reportUnknownArgumentType`
  diagnostics eliminated (use `cast` instead of suppression).

## v0.1.5 (2026-06-23)

### Added
- Leading comma in arrays/objects is now silently skipped (`[,1]` → `[1]`).
- Leading-dot and trailing-dot numbers are normalized to valid JSON
  (`.5` → `0.5`, `5.` → `5.0`).
- Adjacent objects without commas (`}{`) are detected and wrapped in an
  array, same as comma-separated sequences (≥8 KB, ≥3 transitions).
- FAQ.md documenting known limitations and development guide.
- 11 new tests for the above features.

### Changed
- `_skip_suffix_junk` rewritten to reuse the output list in-place instead
  of creating a new one; removed `re` dependency.

### Fixed
- README closing-quote heuristic now lists all possible characters
  (`,`, `}`, `]`, `:`, `\n`, and another `"`).

## v0.1.4 (2026-06-22)

### Added
- Trailing junk detection: `-lnd`, `junkword` etc. after closing `}` are stripped.
- Stress test for massive implicit arrays (447 objects, ~51 KB input).
- Tests for trailing junk scenarios.

### Fixed
- `_is_implicit_object_sequence`: bracket depth tracking (only triggers at depth 0)
  to avoid false-positives on `}, {` inside valid `[...]` arrays.
- `_parse_object`: junk guard before missing-comma check to stop parsing
  when trailing garbage appears after valid JSON.
- 16/17 blocks in `json_failures.txt` now repairable.

## v0.1.3 (2026-06-22)

### Added
- Implicit object sequence repair: comma-separated `{...}, {...}, {...}`
  without an outer `[...]` is automatically wrapped in an array.
- `check_failures.py`: handles both dict and list repair results.

### Changed
- json_failures.txt: 22/26 blocks now repairable (up from 10/10 previously).

## v0.1.2 (2026-06-22)

### Added
- JavaScript literal support: `NaN`, `Infinity`, `-Infinity`, `undefined`
  are recognized and mapped to `null`.
- Hypothesis property-based tests (4 properties, 1100 examples) using
  `TYPE_CHECKING` stubs — zero `type: ignore` annotations, mypy strict clean.

### Fixed
- `_is_closing_quote` lookahead now skips `\r` in addition to space and tab.
- Defensive bounds checks on `self.out[-1]` accesses.
- `_skip_prefix_junk` now skips quoted strings when scanning for `{` or `[`,
  avoiding false matches on `"}"` and `"]"` as structural brackets.
- `_skip_suffix_junk` uses a string-aware bracket depth counter so that
  `}` and `]` inside string values are not mistaken for structural closes.

## v0.1.1 (2026-06-22)

### Fixed
- Invalid JSON escape sequences are now repaired instead of passed through.
  `\*`, `\(`, `\)`, `\p` and other non-standard escapes have their backslash
  escaped (`\\*`, `\\(`, etc.), producing valid JSON.
- `_is_closing_quote` lookahead now skips `\r` in addition to space and tab.
- `_parse_literal` now handles JavaScript-style literals: `NaN`, `Infinity`,
  `-Infinity`, `undefined` — all mapped to `null`.
- Defensive bounds checks on `self.out[-1]` accesses in object/array/string
  parsers.

## v0.1.0 (2026-06-22)

### Added
- Initial release. Single-pass state machine for repairing malformed JSON from
  LLM outputs.
- Handles: unescaped embedded quotes, Python triple-quoted strings, CSV-style
  `""` escaping, single-quoted strings, unquoted keys, trailing commas,
  missing commas/colons, Python/JS literals (`True`/`False`/`None`),
  comments, control characters, extra text before/after JSON, truncated JSON.
- 65 unit tests + 18 performance benchmarks.
- pre-commit: ruff (lint+format), mypy (strict), uv-lock.

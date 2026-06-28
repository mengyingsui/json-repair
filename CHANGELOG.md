# Changelog

## v0.1.15 (2026-06-28)

### Added
- Duplicate opening brace `{{` → `{` skip in `_parse_object` — fixes
  `{{"key": "value"}` misparsed as empty-key object.
- Missing opening quote on key — `key": value` → `"key": value` (alpha-key
  pre-check accepts `"` after key text; `_parse_unquoted_key` consumes
  trailing `"`).

### Changed
- `tests/cases/duplicate_brace.jsonl` (3 cases) and
  `tests/cases/missing_key_quote.jsonl` (3 cases) added.
- `tests/cases/INDEX.md` updated (32 `.jsonl` files).

## v0.1.14 (2026-06-28)

### Added
- `#` line comment support — `_skip_comment` handles `#…` line-ending comments;
  `_parse_value`/`_parse_object`/`_parse_array` recognize `#` alongside `//` and `/*`.
- `tests/cases/comments.jsonl` with 3 entries.
- Cython `wraparound=False` undefined-behaviour fix (`_out[-1]` → `_out[len(_out)-1]`).

### Changed
- `tests/cases/INDEX.md` regenerated (30 `.jsonl` files).

## v0.1.13 (2026-06-27)

### Added
- `tests/cases/unterminated_string.jsonl` + `test_unterminated_string.py`.

### Fixed
- `_parse_string` (pure Python) and `parse_string` (Cython): when a string
  value is missing its closing `"` before `, », the next key's opening `"`
  (e.g. `"entity"`) is no longer consumed as the string terminator. The
  repair emits a closing `"` and resumes key-value parsing correctly.
- Cython `.pyd` rebuilt from `_cparse.pyx` for the above fix; all 192 tests
  pass (pure Python + Cython paths).

### Changed
- `tests/cases/INDEX.md` updated to list 30 `.jsonl` files (was 24 in v0.1.7).

## v0.1.12 (2026-06-27)

### Added
- Cython-accelerated `_parse_string` via `json_repair/_cparse.pyx` — the hot
  character loop is compiled to C when Cython is available, yielding 2–9×
  speedup on string-heavy inputs (e.g. long embedded quotes drop from
  1.5 ms to 168 µs).
- Build infrastructure: `hatch-cython` plugin compiles `.pyx` → `.pyd`/`.so`
  during wheel build; pre-generated `_cparse.c` included in sdist for
  environments without Cython.
- Pure-Python fallback when C extension is unavailable — zero additional
  runtime dependencies.
- `HAS_CYTHON` public constant (`True`/`False`) so callers can detect
  whether acceleration is active.

### Changed
- Build backend switched from `setuptools` back to `hatchling` with
  `hatch-cython` build hook.
- `setup.py` removed; all build config lives in `pyproject.toml`.
- Performance tests ported from custom `timeit` to `pytest-benchmark`
  fixture — run with `--benchmark-histogram` for comparison charts.
- `TestCythonVsPure` compares Cython vs pure Python on 3 input profiles.

## v0.1.11 (2026-06-27)

### Added
- `_skip_suffix_junk` rewritten from O(n) join + backward scan to O(1)
  depth-tracker lookup, eliminating 15–25% of total repair time.
- `IMPLICIT_SEQUENCE_MIN_LENGTH` (8192) extracted as module-level constant.

### Fixed
- `_parse_unquoted_value`: control characters now emit `\uXXXX` instead of
  being silently dropped.

### Changed
- Class & module docstrings updated to reflect O(1) suffix truncation.
- Performance tables refreshed in both READMEs with throughput column.

## v0.1.10 (2026-06-27)

### Added
- `_fix_mixed_quotes()` pre-processing step: recognizes `','word":"` inside
  double-quoted strings and inserts a closing `"` before `','` so that the
  parser handles the single-quoted key `'word'` as a separate key-value pair.
- `_parse_value` now emits `null` when encountering `}`, `]`, or end-of-input
  in value position — handles truncated JSON after colon (e.g. `{"text":`).
- `_fix_colon_in_key()` pre-processing step: regex detects `"key:value"` followed
  by `,` or `}` and splits it into `"key":"value"`.
- `tests/cases/mixed_quotes.jsonl` (3 cases), `truncated.jsonl` (5 cases),
  `missing_colons.jsonl` (1 case).
- **Caveat** section in both READMEs recommending use with a validator.

### Changed
- `json_failures.txt` 8/8 all repairable (up from 5/8).
- `test_return_object_invalid`: bare comma `","` returns `None` instead of
  raising `ValueError`.

## v0.1.9 (2026-06-26)

### Added
- Brace-as-array-close: `}` used to close an array auto-corrects to `]`
  (e.g. `{"a":[1}}]}` → `{"a":[1]}`).
- `_parse_unquoted_value()`: bare-word string values detected and wrapped in
  double quotes (e.g. `{"name": John}` → `{"name": "John"}`).
- `tests/cases/brace_as_array_close.jsonl` (5 cases).
- `tests/cases/unquoted_values.jsonl` (8 cases).

### Changed
- Tests split from `test_repair.py` into 7 per-class files under `tests/`.
- `tests/_helpers.py` extracted for shared test utilities.
- `json_failures.txt` removed from git tracking (added to `.gitignore`).

## v0.1.8 (2026-06-25)

### Added
- Misordered-bracket fix: `]` before `}` in last array element closes the
  object first (e.g. `[{"key": value]}` → `[{"key": value}]`).
- `tests/cases/misordered_brackets.jsonl` (11 cases).

### Changed
- `pyproject.toml`: ruff `extend-exclude` for `tests/cases/`.

## v0.1.7 (2026-06-23)

### Added
- Unbraced-object detection in `_skip_prefix_junk` — text starting with
  `"key" : value` auto-wrapped with `{...}`.
- `tests/cases/double_commas.jsonl` (8 cases).
- `tests/cases/INDEX.md` — catalog of all 24 `.jsonl` test case files.
- All 34/34 `json_failures.txt` blocks repairable (up from 30/34).

### Fixed
- Double-comma regression in `_parse_object`/`_parse_array` — extra `,,`
  after string values skipped by checking last emitted character.
- `_is_closing_quote` alpha lookahead skips `\n` in whitespace.

### Changed
- `tests/cases/` expanded from 22 to 24 `.jsonl` files.

## v0.1.6 (2026-06-23)

### Changed
- All code consolidated into single `_Repairer` class in `_repair.py`;
  removed `_core.py`, `_string.py`, `_value.py` mixin split.
- Test cases (80+) moved from `test_repair.py` into 22 `.jsonl` files
  under `tests/cases/`.
- `_extract_blocks` renamed to `extract_blocks`.

### Fixed
- All Pylance `reportUnusedClass`/`reportUnknownVariableType`/`reportUnknownArgumentType`
  diagnostics eliminated.

## v0.1.5 (2026-06-23)

### Added
- Leading comma in arrays/objects silently skipped (`[,1]` → `[1]`).
- Leading-dot/trailing-dot number normalization (`.5` → `0.5`, `5.` → `5.0`).
- Adjacent objects without commas (`}{`) detected and wrapped in array
  (≥8 KB, ≥3 transitions).
- FAQ.md documenting known limitations and development guide.

### Changed
- `_skip_suffix_junk` rewritten to reuse output list in-place; removed `re`.

## v0.1.4 (2026-06-22)

### Added
- Trailing junk detection: text after closing `}` stripped.
- Stress test for massive implicit arrays (447 objects, ~51 KB input).
- 16/17 blocks in `json_failures.txt` now repairable.

### Fixed
- `_is_implicit_object_sequence`: bracket depth tracking (only at depth 0).
- `_parse_object`: junk guard before missing-comma check.

## v0.1.3 (2026-06-22)

### Added
- Implicit object sequence repair: `{...}, {...}, {...}` auto-wrapped in array.
- `check_failures.py`: handles both dict and list repair results.
- `json_failures.txt`: 22/26 blocks repairable (up from 10/10).

## v0.1.2 (2026-06-22)

### Added
- JavaScript literal support: `NaN`, `Infinity`, `-Infinity`, `undefined` → `null`.
- Hypothesis property-based tests (4 properties, 1100 examples).

### Fixed
- `_is_closing_quote` lookahead skips `\r`.
- Defensive bounds checks on `self.out[-1]` accesses.
- `_skip_prefix_junk` skips quoted strings when scanning for `{`/`[`.
- `_skip_suffix_junk` uses string-aware bracket depth counter.

## v0.1.1 (2026-06-22)

### Fixed
- Invalid JSON escape sequences repaired: `\*`, `\(`, `\)`, `\p` etc.
  have backslash escaped (`\\*`, `\\(`, ...).
- `_is_closing_quote` lookahead skips `\r`.

## v0.1.0 (2026-06-22)

### Added
- Initial release. Single-pass state machine for repairing malformed JSON from
  LLM outputs.
- Handles: unescaped embedded quotes, Python triple-quoted strings, CSV-style
  `""` escaping, single-quoted strings, unquoted keys, trailing commas,
  missing commas/colons, Python/JS literals, comments, control characters,
  extra text before/after JSON, truncated JSON.
- 65 unit tests + 18 performance benchmarks.
- pre-commit: ruff (lint+format), mypy (strict), uv-lock.

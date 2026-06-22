# Changelog

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

# json_repair

Repair malformed JSON from LLM outputs in a **single pass**.

## Problems Solved

LLM-generated JSON often contains these errors — `json_repair` fixes them all:

| Issue | Input | Repaired |
|-------|-------|----------|
| Unescaped quotes in strings | `"He said "hello""` | `"He said \"hello\""` |
| Python triple-quoted strings | `"""text"""` | `"text"` |
| CSV-style `""` escaping | `"Col1""Data"` | `"Col1\"Data"` |
| Single-quoted strings | `{'key': 'val'}` | `{"key": "val"}` |
| Unquoted keys | `{key: "val"}` | `{"key": "val"}` |
| Trailing commas | `{"a": 1,}` | `{"a": 1}` |
| Missing commas/colons | `{"a": 1 "b": 2}` | `{"a": 1, "b": 2}` |
| Python literals | `True / False / None` | `true / false / null` |
| Comments | `// comment` | stripped |
| Truncated JSON | `{"a": 1` | `{"a": 1}` |
| Control characters | literal newline / tab | `\n` / `\t` |
| Extra text before/after | `Here is JSON: {...}` | `{...}` |
| Invalid escape sequences (v0.1.1) | `"\*keeper, \(d_i\)"` | `"\\*keeper, \\(d_i\\)"` |
| JS literals (v0.1.2) | `NaN, Infinity, undefined` | `null` |
| Implicit object sequence (v0.1.3, ≥8KB) | `{...}, {...}, {...}` | `[{...}, {...}, {...}]` |
| Trailing junk data (v0.1.4) | `{"a":1}-lnd\nuser\n...` | `{"a":1}` |
| Leading comma skip (v0.1.5) | `[,1]` | `[1]` |
| Dot-number normalization (v0.1.5) | `.5` / `5.` | `0.5` / `5.0` |
| Adjacent-object wrapping (v0.1.5) | `}{` (≥8KB, ≥3 transitions) | `[{...},{...}]` |
| Unbraced object detection (v0.1.6) | `"key": value` | `{"key": value}` |
| Double-comma skip (v0.1.7) | `"x",,` / `[1,,2]` | `"x",` / `[1,2]` |
| Misordered-bracket fix (v0.1.8) | `[{"]"}]}` → `[{"..."}]` | Auto-close object when `]` appears before `}` in last array element |
| Brace-as-array-close (v0.1.9) | `{"a":[1}}]}` → `{"a":[1]}` | Auto-close array when `}` used instead of `]` |
| Unquoted string values (v0.1.9) | `{"name": John}` → `{"name": "John"}` | Auto-quote unquoted string values |
| Mixed-quote boundary fix (v0.1.10) | `"text','key":"val"` → `"text","key":"val"` | Splits `','word":"` inside double-quoted text — prevents single-quoted keys leaking into preceding value |
| Missing-value-after-colon fill (v0.1.10) | `{"text":` → `{"text": null}` | Fills `null` when value is missing after key in truncated JSON |
| Colon misplaced in key (v0.1.10) | `"key:value"` → `"key":"value"` | Splits a colon that was written inside the key string into key/value pair |
| Missing closing quote fix (v0.1.13) | `"text","entity"` → `"text","entity"` | String missing closing `"` no longer consumes the next key's opening `"` |
| `#` line comment support (v0.1.14) | `"a": 1  # comment` | `#` comments are skipped during repair, no effect on output |
| Duplicate brace skip (v0.1.15) | `{{"key": "value"}` → `{"key": "value"}` | Extra `{` after object opening `{` is silently skipped |
| Missing key opening quote (v0.1.15) | `key": value` → `"key": value` | Missing opening `"` on key is injected; trailing `"` consumed |
| Comma instead of colon after key (v0.1.16) | `"key", "value": "text"` → `"key":null,"value":"text"` | Comma is kept as separator; null emitted for missing value |
| SQL-style `--` comment (v0.1.17) | `"a": 1  -- comment` | `--` comments are skipped during repair, no effect on output |
| Full Cython acceleration (v0.2.0) | All hot-path parsers (string, single-quoted, triple-quoted, value, object, array) run in C | Per-character loops for string, object, array, and value dispatch are **all** compiled to C via `_cparse.pyx` |

## Install

```bash
pip install git+https://gitee.com/mensui/json_repair.git
```

Or with uv:

```bash
uv add git+https://gitee.com/mensui/json_repair.git
```

## Usage

```python
from json_repair import repair_json

# Fix broken JSON from LLM output
broken = '{"response": "He said "hello" to me"}'
fixed = repair_json(broken)
print(fixed)
# '{"response": "He said \"hello\" to me"}'

# Get Python object directly
obj = repair_json(broken, return_object=True)
print(obj)
# {'response': 'He said "hello" to me'}
```

## Caveat

Repaired JSON is always syntactically valid, but may not be semantically what you need (e.g., missing values become `null`).
**It is recommended to pair with a validator** — parse the result and check its structure before use.

> **`#` line comments** are silently stripped (treated as if they never existed). If the LLM's output contains items with `#` comments — e.g. the model was uncertain about whether to include a field and marked it with a comment — the repaired result will still keep that item. This is intentional: the repairer should not decide what the model was unsure about, but callers should be aware that comments do **not** act as exclusion markers.

```python
from json_repair import repair_json

raw = '{"name": "Alice", "age":'
obj = repair_json(raw, return_object=True)
# obj == {"name": "Alice", "age": null}  ← may not be what you want

# Custom validation
def validate(data):
    return isinstance(data, dict) and "age" in data and data["age"] is not None

if validate(obj):
    print("OK:", obj)
else:
    print("unexpected shape, discard or retry")
```

## Design

```
Input text
  │
  ├─ Pre-processing (regex)
  │    _fix_colon_in_key
  │    _fix_mixed_quotes
  │
  ├─ _Repairer state machine
  │    1. _skip_prefix_junk
  │    2. >=8KB {..}{..} → wrap as array
  │    3. _parse_value
  │      ├─ _parse_object
  │      ├─ _parse_array
  │      ├─ _parse_string
  │      └─ _parse_literal
  │    4. _close_brackets
  │    5. _skip_suffix_junk (O(1) depth-tracker lookup)
  │
  └─ Repaired JSON
```

The state machine is **single-pass** — depth is tracked during the main parse, so suffix cleanup does not require a second traversal. Core heuristic:

> Inside a string, `"` is only treated as closing if the next non-whitespace
> character is `,` `}` `]` `:` `\n` or another `"`. Everything else is escaped.

This is tuned for the natural-language embedded quotes common in LLM output.

## Performance

| Scenario | Size | Time (Cython) | Time (pure Python) | Throughput |
|----------|------|------|------|------|
| Empty `{}` | 2 B | 2 µs | 2 µs | 1.0 MB/s |
| Small JSON | 48 B | 4 µs | 11 µs | 11.4 MB/s |
| Medium JSON | 2.4 KB | 0.09 ms | 0.38 ms | 25.4 MB/s |
| Large JSON | 9.2 KB | 1.5 ms | 4.9 ms | 5.8 MB/s |
| Realistic LLM output | 0.3 KB | 12 µs | 47 µs | 23.8 MB/s |
| Deeply nested | 0.2 KB | 6 µs | 20 µs | 31.7 MB/s |
| Many embedded quotes (short) | 0.2 KB | 4 µs | 13 µs | 47.6 MB/s |
| Many embedded quotes (long) | 12 KB | 155 µs | 1.5 ms | 73.8 MB/s |

Cython acceleration provides **2–10×** speedup on string-heavy inputs.
As of v0.2.0, **all** hot-path parsers run in C — including object/array/value
dispatch (previously pure-Python on structure-heavy inputs).
Measured with `pytest-benchmark` — see [Development](#development).

## Versions

| Version | Date | Description |
|---------|------|-------------|
| v0.2.0 | 2026-06-28 | Full Cython acceleration — all hot-path parsers (single-quoted, triple-quoted, value/object/array dispatch) now run in C; only `_parse_string` was Cythonized before |
| v0.1.17 | 2026-06-28 | SQL-style `--` line comment support (`_skip_comment` skips `--…` lines) |
| v0.1.16 | 2026-06-28 | Comma-after-key fix — `"key", "value":` → `"key":null,"value":` (null for missing value, comma as separator) |
| v0.1.15 | 2026-06-28 | Duplicate brace `{{` → `{` skip; Missing key opening quote — `key":` → `"key":` |
| v0.1.14 | 2026-06-28 | `#` line comment support (`_skip_comment` skips `#…` lines); Cython `wraparound=False` UB fix |
| v0.1.13 | 2026-06-27 | Missing-closing-quote fix — `_parse_string`/`parse_string` no longer consumes next key's opening `"`; added `unterminated_string.jsonl` |
| v0.1.12 | 2026-06-27 | Cython-accelerated `_parse_string` (`_cparse.pyx`); build system migrated to `hatchling` + `hatch-cython`; removed `setup.py`; benchmarks ported to `pytest-benchmark` |
| v0.1.11 | 2026-06-27 | `_skip_suffix_junk` O(1) depth-tracker (eliminates 15–25% of total time); `IMPLICIT_SEQUENCE_MIN_LENGTH` constant; control chars emit `\uXXXX` |
| v0.1.10 | 2026-06-27 | Mixed-quote boundary fix; missing-value-after-colon fill (`{"text":` → `{"text":null}`); colon-misplaced-in-key split; `mixed_quotes.jsonl`; 8/8 json_failures.txt |
| v0.1.9 | 2026-06-26 | Brace-as-array-close; unquoted string value repair; tests split into per-class files |
| v0.1.8 | 2026-06-25 | Misordered-bracket fix; `misordered_brackets.jsonl` |
| v0.1.7 | 2026-06-23 | Unbraced-object detection; double-comma skip; 24 `.jsonl` files; 34/34 json_failures.txt |
| v0.1.6 | 2026-06-23 | Single-file `_Repairer`; 22 `.jsonl` test files; Pylance strict-mode clean |
| v0.1.5 | 2026-06-23 | Leading comma skip; dot-number normalization; adjacent-object `}{` array wrap; FAQ.md |
| v0.1.4 | 2026-06-22 | Trailing junk detection; depth-tracked implicit arrays; 16/17 json_failures.txt |
| v0.1.3 | 2026-06-22 | Implicit object sequence auto-wrap in array |
| v0.1.2 | 2026-06-22 | JS literal support; Hypothesis property tests; defensive fixes |
| v0.1.1 | 2026-06-22 | Fix invalid JSON escape sequences (`\*`, `\(`, `\)`, etc.) |
| v0.1.0 | 2026-06-22 | Initial release — single-pass state machine for LLM JSON |

## Development

```bash
git clone https://gitee.com/mensui/json_repair.git
cd json_repair

# Install deps & compile Cython (.pyx → .c → .pyd)
uv sync

# Run all tests
uv run test

# After modifying _cparse.pyx — rebuild .pyd
uv sync

# Performance benchmarks
uv run bench
uv run bench-hist
uv run bench-compare

# Lint / type check
uv run lint
uv run typecheck

# Pre-commit
uv run precommit
```

`uv sync` compiles `.pyx → .c → .pyd` via `hatch-cython` automatically.
`.c` is build-artifact, git-ignored. Commands defined in `[project.scripts]`
in `pyproject.toml`, implemented by `json_repair/_dev.py`.

## License

GNU General Public License v2.0

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

State machine with a single heuristic:

> Inside a string, `"` is only treated as closing if the next non-whitespace
> character is `,` `}` `]` `:` `\n` or another `"`. Everything else is escaped.

This is tuned for the natural-language embedded quotes common in LLM output.

## Performance

| Scenario | Size | Time |
|----------|------|------|
| Empty `{}` | 2 B | 3 µs |
| Small JSON | 48 B | 20 µs |
| Medium JSON | 2.4 KB | 0.74 ms |
| Large JSON | 9.2 KB | 4.6 ms |
| Realistic LLM output | 0.3 KB | 64 µs |
| Unquoted value repair | 14 B | 6 µs |
| Misordered-bracket / `}` close | 0.2–0.5 KB | 76–143 µs |

Corrupted JSON is repaired at the same speed as valid JSON — near-zero overhead.

## Versions

| Version | Description |
|---------|-------------|
| v0.1.10 | Mixed-quote boundary fix (`','word":"` auto-split); missing-value-after-colon fill (`{"text":` → `{"text":null}`); colon misplaced in key (`"key:value"` → `"key":"value"`); `mixed_quotes.jsonl`; 8/8 `json_failures.txt` all fixed |
| v0.1.9 | Brace-as-array-close (`{"a":[1}}]}` → `{"a":[1]}`); unquoted string value repair (`{"name": John}` → `{"name": "John"}`); tests split into per-class files; `brace_as_array_close.jsonl`, `unquoted_values.jsonl` |
| v0.1.7 | Double-comma skip (`",,"`→`","`); 24 `.jsonl` test files; 34/34 `json_failures.txt` all fixed |
| v0.1.6 | Single-file `_Repairer`; unbraced-object detection; 22 `.jsonl` test files; Pylance strict-mode clean |
| v0.1.5 | Leading comma skip, dot-number normalization, adjacent-object `}{` array wrap |
| v0.1.4 | Trailing junk detection, depth-tracked implicit arrays, 16/17 json_failures.txt |
| v0.1.3 | Implicit object sequence auto-wrapped, massive array stress test |
| v0.1.2 | JS literal support, Hypothesis property tests, defensive fixes |
| v0.1.1 | Fix invalid JSON escape sequences (`\*`, `\(`, `\)`, etc.) |
| v0.1.0 | Initial release — single-pass state machine for LLM JSON |

## Development

```bash
git clone https://gitee.com/mensui/json_repair.git
cd json_repair
uv sync
uv run pytest tests/ -v
uv run pre-commit run --all-files
```

## License

GNU General Public License v2.0

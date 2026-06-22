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
| Invalid escape sequences (v0.1.1) | `"\\*keeper, \\(d_i\\)"` | `"\\\\*keeper, \\\\(d_i\\\\)"` |

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

## Design

State machine with a single heuristic:

> Inside a string, `"` is only treated as closing if the next non-whitespace
> character is `,` `}` `]` or `:`. Everything else is escaped.

This is tuned for the natural-language embedded quotes common in LLM output.

## Performance

| Scenario | Size | Time |
|----------|------|------|
| Empty `{}` | 2 B | 2 µs |
| Small JSON | 68 B | 10 µs |
| Medium JSON | 2.4 KB | 0.4 ms |
| Large JSON | 9.2 KB | 2.2 ms |
| Realistic LLM output | 0.3 KB | 50 µs |
| Invalid escape sequences | 0.1 KB | 15 µs |

Corrupted JSON is repaired at the same speed as valid JSON — near-zero overhead.

## Versions

| Version | Description |
|---------|-------------|
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

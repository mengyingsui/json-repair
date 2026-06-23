# Test Cases Index

All `.jsonl` files under `tests/cases/` are auto-discovered by
`test_repair.py::test_jsonl_cases` (lines with `"expected"`) and
`test_repair.py::test_broken_patterns` (lines with `"input"` only).
Lines use the format: `{"input": "...", "expected": ...}`.

> ⚠️ **When adding a new `.jsonl` file, update this INDEX.md.**

---

| File | Cases | Problem Solved |
|---|---|---|
| `broken_patterns.jsonl` | 16 | Mixed defect types — embedded quotes, unquoted keys, trailing commas, missing commas/colons, comments, unescaped newlines, invalid escapes, Python/JS literals, extra text. *(Input-only: validity + idempotence)* |
| `comments.jsonl` | 2 | C-style `//` and `/* */` comments stripped before parsing. |
| `complex_scenarios.jsonl` | 5 | Realistic multi-fault inputs — mixed quote styles, comments, trailing commas, unquoted keys, unescaped inner quotes, `None`. |
| `control_characters.jsonl` | 2 | Literal `\n` and `\t` preserved as control chars in output. |
| `csv_escaping.jsonl` | 2 | Doubled quotes `""` decoded to single `"` (CSV convention). |
| `double_commas.jsonl` | 8 | Extra commas `,,` / `,,,` in objects and arrays — including after `)`-ending string values (the block-26 Flutter pattern). |
| `edge_cases.jsonl` | 8 | Empty objects/arrays, bare strings/numbers/booleans, multi-byte Unicode (Chinese), special symbols, Windows backslash paths. |
| `extra_text.jsonl` | 3 | Human-language text before/after JSON ("Here is your JSON: …"). |
| `four_quote.jsonl` | 2 | `""""` quadruple-quoted strings (triple-quote variant). |
| `invalid_escape.jsonl` | 8 | Backslash sequences that aren't valid JSON escapes (`\*`, `\(`, `\)`) — backslash preserved as literal. |
| `leading_comma.jsonl` | 4 | Leading comma before first array element removed. |
| `leading_dot_numbers.jsonl` | 5 | `.5` → `0.5`, `5.` → `5.0` normalization. |
| `markdown_code_block.jsonl` | 1 | JSON extracted from \`\`\`json … \`\`\` fence. |
| `missing_colons.jsonl` | 1 | Colon inserted between key and value (`{"key" "value"}`). |
| `missing_commas.jsonl` | 3 | Commas inserted between elements/key-value pairs. |
| `python_literals.jsonl` | 3 | `True`/`False`/`None` → `true`/`false`/`null`. |
| `single_quoted.jsonl` | 3 | Single-quoted keys/values converted to double quotes. |
| `trailing_commas.jsonl` | 3 | Trailing comma after last element removed, including nested. |
| `trailing_junk.jsonl` | 3 | Extraneous text after valid JSON discarded (`-lnd`, `junk`, log lines). |
| `triple_quoted.jsonl` | 5 | `"""…"""` multiline strings — inner quotes, embedded newlines, empty. |
| `truncated.jsonl` | 4 | Missing closing braces/brackets/quotes — parser infers and closes. |
| `unescaped_quotes.jsonl` | 5 | Unescaped `"` inside strings — parser deduces delimiter vs content. |
| `unquoted_keys.jsonl` | 3 | `{key: "value"}` style unquoted object keys. |
| `valid_pass_through.jsonl` | 5 | Already-valid JSON — must pass through unchanged (regression guard). |

## Quick Reference by Feature

| Feature | File | Example Input |
|---|---|---|
| Unbraced object wrapping | `complex_scenarios.jsonl` `[0]` | `'"key": "value"'` |
| Adjacent object wrapping | `complex_scenarios.jsonl` `[1]` | `'{"a":1}{"b":2}'` |
| Double-comma skip | `double_commas.jsonl` | `'{"a":1,, "b":2}'` |
| Leading comma skip | `leading_comma.jsonl` | `'[,1,2,3]'` |
| Dot-number normalize | `leading_dot_numbers.jsonl` | `'{".5": 1}'` |
| Triple-quote | `triple_quoted.jsonl` | `'{"a": """hello"""}'` |
| Four-quote | `four_quote.jsonl` | `'{"a": """"hello""""}'` |
| Single-quoted keys/values | `single_quoted.jsonl` | `"{'a': 'value'}"` |
| Unescaped quote | `unescaped_quotes.jsonl` | `'{"a": "say "hello""}'` |
| CSVer doubled quotes | `csv_escaping.jsonl` | `'{"a": "say ""hello"""}'` |
| Truncated | `truncated.jsonl` | `'{"a": 1'` |
| Missing comma | `missing_commas.jsonl` | `'{"a": 1 "b": 2}'` |
| Missing colon | `missing_colons.jsonl` | `'{"key" "value"}'` |
| Trailing comma | `trailing_commas.jsonl` | `'{"a": 1,}'` |
| Trailing junk | `trailing_junk.jsonl` | `'{"a":1}-lnd'` |
| Extra text before/after | `extra_text.jsonl` | `'Here is JSON: {"a":1}'` |
| Python literals | `python_literals.jsonl` | `'{"a": True, "b": None}'` |
| JS literals | `broken_patterns.jsonl` `[14-15]` | `'{"val": Infinity}'` |
| C-style comments | `comments.jsonl` | `'{"a":1 /* comment */}'` |
| Markdown code fence | `markdown_code_block.jsonl` | `` '```json\n{"a":1}\n```' `` |
| Control chars in string | `control_characters.jsonl` | `'{"a": "line1\nline2"}'` |
| Invalid escape | `invalid_escape.jsonl` | `'{"a": "\\*keeper"}'` |
| Unquoted key | `unquoted_keys.jsonl` | `'{key: "value"}'` |
| Valid passthrough | `valid_pass_through.jsonl` | `'{"a": 1}'` |
| Edge cases | `edge_cases.jsonl` | empty object, bare string, Unicode, … |

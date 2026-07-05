# Test Cases Index

All `.jsonl` files under `tests/cases/` are auto-discovered by
`test_repair.py::test_jsonl_cases` (lines with `"expected"`) and
`test_repair.py::test_broken_patterns` (lines with `"input"` only).
Lines use the format: `{"input": "...", "expected": ...}`.

> ⚠️ **When adding a new `.jsonl` file, update this INDEX.md.**
> **`bench_data.jsonl`** — benchmark input data, included here for reference. Not a test case file (no `"expected"` field).

---

| File | Cases | Problem Solved |
|---|---|---|
| `brace_as_array_close.jsonl` | 5 | Real entries from json_failures.txt — array closed with } instead of ] (}}]} → }]}) in single/multi-item arrays, including code-fenced variants. |
| `broken_patterns.jsonl` | 16 | Mixed defect types — embedded quotes, unquoted keys, trailing commas, missing commas/colons, comments, unescaped newlines, invalid escapes, Python/JS literals, extra text. (Input-only: validity + idempotence.) |
| `colon_misplaced_as_comma.jsonl` | 2 | Comma used instead of colon after key — `"key", "value": "text"` → `"key":null,"value":"text"` (null for missing value, comma remains as separator). |
| `comments.jsonl` | 5 | C-style //, /* */, #, and SQL-style `--` comments stripped before parsing. |
| `complex_scenarios.jsonl` | 9 | Realistic multi-fault inputs — mixed quote styles, comments, trailing commas, unquoted keys, unescaped inner quotes, None. |
| `control_characters.jsonl` | 3 | Literal \n and \t preserved as control chars in output. |
| `csv_escaping.jsonl` | 2 | Doubled quotes "" decoded to single " (CSV convention). |
| `double_commas.jsonl` | 8 | Extra commas ,, / ,,, in objects and arrays — including after )-ending string values (the block-26 Flutter pattern). |
| `duplicate_brace.jsonl` | 3 | Extra `{` after object opening `{` — `{{"key": "value"}` → `{"key": "value"}`. |
| `edge_cases.jsonl` | 19 | Empty objects/arrays, bare strings/numbers/booleans, multi-byte Unicode (Chinese), special symbols, Windows backslash paths, backslash-at-EOF, surrogate escapes. |
| `embedded_quotes_large.jsonl` | 1 | Large multi-segment JSON with embedded ASCII " in both text values and entity arrays, plus literal \n line breaks. (Input-only.) |
| `extra_text.jsonl` | 3 | Human-language text before/after JSON ("Here is your JSON: …"). |
| `four_quote.jsonl` | 2 | """" quadruple-quoted strings (triple-quote variant). |
| `invalid_escape.jsonl` | 9 | Backslash sequences that aren't valid JSON escapes (\*, \(), \), \u...) — backslash preserved as literal. |
| `leading_comma.jsonl` | 4 | Leading comma before first array element removed. |
| `leading_dot_numbers.jsonl` | 5 | .5 → 0.5, 5. → 5.0 normalization. |
| `markdown_code_block.jsonl` | 1 | JSON extracted from ```json … ``` fence. |
| `misordered_brackets.jsonl` | 15 | Array's last object has ] misplaced before/instead of } — simplified + real-world Chinese-text cases from json_failures.txt. |
| `missing_colons.jsonl` | 2 | Colon inserted between key and value. Also colon misplaced inside key ("key:value" → "key":"value"). |
| `missing_key_quote.jsonl` | 3 | Missing opening `"` on key — `key": value` → `"key": value`. |
| `missing_commas.jsonl` | 3 | Commas inserted between elements/key-value pairs. |
| `mixed_quotes.jsonl` | 3 | Mixed single/double quote boundary: ','word":" inside a double-quoted value — LLM output where a single-quoted key leaks into the preceding text value. |
| `python_literals.jsonl` | 3 | True/False/None → true/false/null. |
| `prefix_tags.jsonl` | 9 | Metadata tags `[TEXT_START]`/`[TEXT_END]`/`[X]` and code fences before JSON — `[TAG_A][TAG_B]{"a":1}`; real `[1,2,3]` arrays preserved. |
| `single_quoted.jsonl` | 3 | Single-quoted keys/values converted to double quotes. |
| `trailing_commas.jsonl` | 3 | Trailing comma after last element removed, including nested. |
| `trailing_junk.jsonl` | 3 | Extraneous text after valid JSON discarded (-lnd, junk, log lines). |
| `triple_quoted.jsonl` | 6 | """…""" multiline strings — inner quotes, embedded newlines, empty. |
| `truncated.jsonl` | 8 | Missing closing braces/brackets/quotes — parser infers and closes. Also handles missing value after colon (emits null). |
| `unescaped_quotes.jsonl` | 4 | Unescaped " inside strings — parser deduces delimiter vs content. Handles `"Stop Doing"`, `"Pomodoro Timer"`, `"Done"` in long text values; also `"The Climb"` followed by `,` and `"buddy movie"` in prose. |
| `unquoted_keys.jsonl` | 3 | {key: "value"} style unquoted object keys. |
| `unquoted_values.jsonl` | 8 | Unquoted string values like {"name": John} — also multi-word values with spaces, values containing escaped quotes. |
| `unterminated_string.jsonl` | 1 | String value missing closing " before , — next key's opening " is otherwise consumed as the string terminator. |
| `valid_pass_through.jsonl` | 9 | Already-valid JSON — must pass through unchanged (regression guard). |
| `bench_data.jsonl` | 19 | ⚡ Benchmark input data (no `expected` field). Used by both Rust (`crates/json-repair-core/benches/`) and Python (`tests/python/test_performance.py`) benchmarks. |

## Quick Reference by Feature

| Feature | File | Example Input |
|---|---|---|
| Brace as array close | `brace_as_array_close.jsonl` | `'{"items":[{"x":1}}]}'` |
| C-style comments | `comments.jsonl` | `'{"a":1 /* comment */}'` |
| Control chars in string | `control_characters.jsonl` | `'{"a": "line1\nline2"}'` |
| CSVer doubled quotes | `csv_escaping.jsonl` | `'{"a": "say ""hello"""}'` |
| Dot-number normalize | `leading_dot_numbers.jsonl` | `'{".5": 1}'` |
| Double-comma skip | `double_commas.jsonl` | `'{"a":1,, "b":2}'` |
| Duplicate brace skip | `duplicate_brace.jsonl` | `'{{"key": "value"}'` |
| Duplicate brace skip (nested) | `duplicate_brace.jsonl` | `'{"a": 1, {{"b": 2}}'` |
| Edge cases | `edge_cases.jsonl` | empty object, bare string, Unicode, … |
| Extra text before/after | `extra_text.jsonl` | `'Here is JSON: {"a":1}'` |
| Four-quote | `four_quote.jsonl` | `'{"a": """"hello""""}'` |
| Invalid escape | `invalid_escape.jsonl` | `'{"a": "\\*keeper"}'` |
| JS literals | `broken_patterns.jsonl` | `'{"val": Infinity}'` |
| Leading comma skip | `leading_comma.jsonl` | `'[,1,2,3]'` |
| Markdown code fence | `markdown_code_block.jsonl` | `` '```json\n{"a":1}\n```' `` |
| Missing colon | `missing_colons.jsonl` | `'{"key" "value"}'` |
| Missing key quote | `missing_key_quote.jsonl` | `'{"a": 1, key": 2}'` |
| Missing comma | `missing_commas.jsonl` | `'{"a": 1 "b": 2}'` |
| Mixed quotes | `mixed_quotes.jsonl` | `','word":"` inside double-quoted string |
| Prefix tags / code fences | `prefix_tags.jsonl` | `'[TEXT_START]```text↵...```[TEXT_END]↵{"a":1}'` |
| Python literals | `python_literals.jsonl` | `'{"a": True, "b": None}'` |
| Single-quoted keys/values | `single_quoted.jsonl` | `"{'a': 'value'}"` |
| Trailing comma | `trailing_commas.jsonl` | `'{"a": 1,}'` |
| Trailing junk | `trailing_junk.jsonl` | `'{"a":1}-lnd'` |
| Triple-quote | `triple_quoted.jsonl` | `'{"a": """hello"""}'` |
| Truncated | `truncated.jsonl` | `'{"a": 1'` |
| Unbraced object wrapping | `complex_scenarios.jsonl` `[0]` | `'"key": "value"'` |
| Unescaped quote | `unescaped_quotes.jsonl` | `'{"a": "say "hello""}'` |
| Unquoted key | `unquoted_keys.jsonl` | `'{key: "value"}'` |
| Unquoted value | `unquoted_values.jsonl` | `'{"name": John}'` |
| Unterminated string | `unterminated_string.jsonl` | missing `"` before `,` in multi-attribute JSON |
| Valid passthrough | `valid_pass_through.jsonl` | `'{"a": 1}'` |

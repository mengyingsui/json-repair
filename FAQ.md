# FAQ / Known Limitations

## General

### What does json_repair *not* handle?

- **Non-standard key-value separators** — when a character other than `:`
  separates a key from a non-string value (e.g. `{"key"; 1}`, `{"key"| 2}`,
  `{"key"& 3}`, or `{"key" 4}`), the string parser treats the `"` before
  the separator as embedded content and escapes it. The output is
  syntactically valid JSON but semantically wrong — the entire input
  collapses into a single string key with `null` value.
- **Nested unescaped quotes in deeply ambiguous contexts** — the heuristic
  uses a short lookahead to decide whether `"` closes the string or is
  embedded content. When natural-language text contains multiple adjacent
  quotes followed by structural characters, the guess can be wrong.
- **Single-quoted string escape sequences** — Python-style `\n`, `\t`,
  `\xhh`, etc. inside `'...'` strings are not translated; they are emitted
  as-is (safe, since the output is double-quoted JSON).
- **JSON5 extended syntax** — hex numbers (`0xFF`), leading-zero octals,
  `Infinity`/`NaN` as bare words inside arrays, multi-line strings, and
  other JSON5 features beyond comments and trailing commas are not supported.
- **Broken Unicode escape sequences** — `\u` followed by non-hex characters
  is treated as an invalid escape (backslash gets escaped to `\\u...`).
- **Unquoted values with spaces or special chars** — the unquoted value
  parser stops at `,`, `}`, `]`, so unquoted strings containing whitespace,
  colons, or nested braces are not fully captured (e.g. `{"name": John Doe}`
  yields just `"John"`).

### Is the output guaranteed to be valid JSON?

For most malformed inputs encountered in practice (embedded quotes, missing
brackets, trailing commas, etc.) the repaired output is valid JSON.

However, some patterns produce output that is **syntactically valid but
semantically wrong** — the repairer collapses the entire input into a
single string key with `null` value. These include:
- Non-standard key-value separators (e.g. `";"`, `"|"`, `"&"` with
  non-string values like `{"key"; 1}`)
- Backslash before closing quote (`"\`)
- Isolated stray double-quote characters
- Input that is too short to trigger the state machine's close-bracket logic

In Python, `repair_json` with `return_object=True` raises `ValueError` for
these cases. Without `return_object`, the string is returned as-is and may
or may not be valid JSON — the caller should validate with `json.loads()`
if they need guarantees.

In Rust, `repair_json` always returns `Ok(String)` (the repairer always
produces *some* output), but the output may not pass
`serde_json::from_str::<serde_json::Value>()`. The caller should validate
the result if required.

### Does it handle streaming / partial output?

Not directly. `repair_json` works on a complete string. For incremental
repair (e.g. streaming LLM tokens), you would need to buffer tokens and
call `repair_json` on each chunk.

## Limitations

### Implicit object sequence detection is size-gated

Comma-separated `{...}, {...}, {...}` without an outer `[...]` is only
wrapped in an array when:

- The remaining input is **≥8 KB** (to avoid false positives from small
  objects that happen to contain `}, {` inside strings).
- At least **3 structural `}, {` patterns** are found at bracket depth 0.

Smaller or fewer-object sequences are treated as truncated individual
objects (the first `{` is parsed, and trailing `}` / `,` are consumed
as suffix junk).

### Trailing junk detection is heuristic

After finding the last `}` or `]` at depth 0, the trailing text is
checked: if it contains non-whitespace and does not start with `` ``` ``,
it is stripped. This can fail for valid JSON followed by content that
starts with `{` or `[` (though that case is rare in LLM output).

### Bracket-misorder fixes

Two symmetric fixes handle swapped closing brackets:

| Fix | Pattern | Behavior |
|-----|---------|----------|
| **Object `]` close** | `[{"key": value]}` → `[{"key": value}]` | When `]` appears where `}` is expected in an object, the object is closed with `}` first. |
| **Array `}` close** | `{"a":[1}}]}` → `{"a":[1]}` | When `}` appears where `]` is expected in an array, the array is closed with `]` first. |

Both only trigger when the wrong bracket is found at the *expected* closing
position of a nested construct — they do not rearrange arbitrary bracket
sequences.

### Unquoted string values

When a value position contains a bare word (no opening `"`), the parser
collects characters up to the nearest `,`, `}`, or `]` and wraps the result
in double quotes. This handles common LLM outputs like `{"name": John}`
without requiring a full parser.

Limitations:
- Words with internal spaces, colons, or brackets are truncated at the delimiter.
- Booleans and numbers are always quoted (never normalized to true/false/numeric).
- The heuristic does not attempt to distinguish intended unquoted strings from
  intended JSON literals — everything is wrapped in quotes.

### Mixed single/double quote boundary

When LLM output uses both `'` and `"` quote styles, a double-quoted string
value may contain `','word":"` where `'word'` was originally a single-quoted
key. The pre-processing step `','([a-zA-Z_]\w*)\":\"` → `","$1":"` replaces
this pattern, closing the double-quoted string before `','` so the parser
correctly treats `word` as the next key.

Limitations:
- The pattern must match exactly `','<key>":"` (ASCII single quotes, comma,
  alphanumeric key, double-quote, colon, double-quote).
- If valid text content ever contains this exact sequence, the split will
  incorrectly split it.
- Smart quotes (`'`, `'` U+2018/U+2019) are not matched.

### Markdown code-fence handling

Only `` ``` `` fences are recognized (not `` ````` ``).
The opening fence must be at the very start of the string (after
optional leading whitespace). The closing fence is detected by a
trailing `\n```\s*` pattern.

### Embedded quote accuracy depends on context

The string parser uses a multi-strategy heuristic:

| Next non-whitespace char | Decision |
|--------------------------|----------|
| `,` `:` `\n` `"` | Closing quote |
| `]` / `}` | Uses **bracket-stack + string-aware balance scan**:<br>1. If closing bracket doesn't match the current open bracket → **Embedded** (the bracket can't be a real closer).<br>2. If it matches, a forward scan checks whether treating this `"` as a terminator leads to a balanced close. The scan is string-aware (brackets inside unterminated j unk strings don't count) and requires an out-of-string `:` before `}` when structural `,` follows the terminator — preventing mimic junk like `"c", "d"}` from being falsely accepted. |
| anything else | Embedded content → escape |

This handles natural-language `"He said "]boom" loudly"` and similar
embedded quotes before `]`/`}` without false positives from structural
trailing junk. False positives are still possible in degenerate cases
(e.g. mimic junk that perfectly matches a valid object tail including
key colons), but the common patterns are now safe.

## Performance

### How fast is it?

Single-pass, O(n) character-by-character in Rust. A 100 KB input
typically completes in under 1 ms on modern hardware.

Both Python and Rust benchmarks now read from the same shared data file
`tests/cases/bench_data.jsonl` (20 cases covering fixable and unfixable
inputs of varying sizes). Run:

    cargo bench -p json-repair-core              # Rust (criterion)
    pytest tests/python/test_performance.py --benchmark-only   # Python (pytest-benchmark)

### Why not use a parser-based approach (e.g. serde_json fallback)?

Parser-based approaches require `try`/`catch` retries and cannot handle
many of the structural issues (missing brackets, embedded quotes, etc.)
that the state machine fixes in one pass.

## Development

### How do I add a new repair rule?

1. Add logic to the appropriate submodule in `crates/json-repair-core/src/repairer/`
   (`string.rs`, `number.rs`, `literal.rs`, `keys.rs`, `structure.rs`, `comment.rs`,
   `junk.rs`) or the module entry at `repairer/mod.rs`.
2. Wire it into `parse_value()` or `parse_string()` in the relevant submodule at the right priority.
3. Add a `.jsonl` test data file under `tests/cases/` (one JSON object per line
   with `input` and optionally `expected` keys).
4. If `expected` is provided, add a parametrized Rust integration test in
   `crates/json-repair-core/tests/`, or rely on the existing `jsonl_cases.rs`
   which automatically picks up all `.jsonl` files.
5. Run `cargo test -p json-repair-core` to verify.
6. Run `cargo bench -p json-repair-core` to benchmark.
7. Run `uv run ruff check json_repair/ tests/python/` to lint.

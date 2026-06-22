# FAQ / Known Limitations

## General

### What does json_repair *not* handle?

- **Nested unescaped quotes in deeply ambiguous contexts** — the heuristic
  `_is_closing_quote` uses a short lookahead to decide whether `"` closes
  the string or is embedded content.  When natural-language text contains
  multiple adjacent quotes followed by structural characters, the guess
  can be wrong.
- **Single-quoted string escape sequences** — Python-style `\n`, `\t`,
  `\xhh`, etc. inside `'...'` strings are not translated; they are emitted
  as-is (safe, since the output is double-quoted JSON).
- **JSON5 extended syntax** — hex numbers (`0xFF`), leading-zero octals,
  `Infinity`/`NaN` as bare words inside arrays, multi-line strings, and
  other JSON5 features beyond comments and trailing commas are not supported.
- **Broken Unicode escape sequences** — `\u` followed by non-hex characters
  is treated as an invalid escape (backslash gets escaped to `\\u...`).

### Is the output guaranteed to be valid JSON?

No — if the input is catastrophically malformed (e.g. random bytes, wrong
language entirely), the repaired string may still fail `json.loads()`.
When `return_object=True`, a `ValueError` is raised in that case.

### Does it handle streaming / partial output?

Not directly.  `repair_json` works on a complete string.  For incremental
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
checked: if it contains non-whitespace and does not start with ` ``` `,
it is stripped.  This can fail for valid JSON followed by content that
starts with `{` or `[` (though that case is rare in LLM output).

### Markdown code-fence handling

Only ```` ``` ```` fences are recognized (not ```` ````` ````).
The opening fence must be at the very start of the string (after
optional leading whitespace).  The closing fence is detected by a
trailing `\n```\s*` pattern.

### Embedded quote accuracy depends on context

`_is_closing_quote` looks ahead at the next non-whitespace character:

| Next char | Decision |
|-----------|----------|
| `,` `}` `]` `:` `\n` | Closing quote |
| `"` | Closing quote (next string follows — common missing-comma case) |
| anything else | Embedded content → escape |

This is tuned for LLM output where unescaped natural-language quotes
are common, but unusual formatting can still produce false positives.

## Performance

### How fast is it?

Single-pass, O(n) character-by-character.  A 100 KB input typically
completes in under 1 ms on modern hardware.  See `tests/perf_report.py`
for micro-benchmarks.

### Why not use a parser-based approach (e.g. `json.loads` fallback)?

Parser-based approaches require `try`/`except` retries and cannot handle
many of the structural issues (missing brackets, embedded quotes, etc.)
that the state machine fixes in one pass.

## Development

### How do I add a new repair rule?

1. Add a new method to `_Repairer` (e.g. `_parse_my_feature`).
2. Wire it into `_parse_value()` or `_parse_string()` at the right priority.
3. Add tests to `tests/test_repair.py`.
4. Run `uv run pytest` to verify.
5. Run `uv run ruff check && uv run mypy json_repair` to lint and type-check.

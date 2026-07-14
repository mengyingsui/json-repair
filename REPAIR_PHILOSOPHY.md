# Repair Philosophy

json_repair is a repairer, not a validator.  It reads malformed JSON character-by-character and emits valid JSON.  This document describes the heuristic rules it uses to decide how to repair.

At every character position, the repairer must answer one question: **what valid JSON structure does this character *want* to belong to?**  There is no perfect answer, so the repairer applies locally optimal heuristics and commits.

---

## String repair

### Embedded quotes

When the repairer encounters `"` inside an apparent string, it must decide: does this `"` close the string, or was it meant to be escaped?

**Rule**: If `""` is found, emit `\"` and advance past the first `"` only.  The second `"` is re-evaluated next iteration — it may close the string or be another embedded quote.

| Input                               | Repaired                            |
|-------------------------------------|-------------------------------------|
| `{"msg":"He said ""hello"" to me"}` | `{"msg":"He said \"hello\" to me"}` |

**Rule**: If `"` is followed by `,`, `}`, `]`, or `\n`, it likely closes the string.

| Input                       | Repaired                    |
|-----------------------------|-----------------------------|
| `{"key":"value", "next":1}` | `{"key":"value", "next":1}` |
| `{"key":"value"}`           | `{"key":"value"}`           |

- *Exception (value-star check)*: `,` after the quote must be followed by a valid value starter — otherwise the quote was embedded.

| Input                    | Repaired                    |
|--------------------------|-----------------------------|
| `{"a": "text,", "b": 2}` | `{"a": "text,\", \"b\": 2}` |

  Here the `,` is inside string content (followed by `"b"`, which is a key, not a value starter at the array level), so the quote stays open.

- *Exception (embedded bracket)*: `}` / `]` after the quote might be bracket characters inside the string, not the container closer.

| Input   | Repaired                                               |
|---------|--------------------------------------------------------|
| `["]}]` | `["]}]` — the `]` and `}` are inside the string value. |

**Rule**: If `"` is followed immediately by another `"`, it closes the string (the doubled `""` was already handled, and now the string truly ends).

| Input              | Repaired            |
|--------------------|---------------------|
| `{"text":""rest"}` | `{"text":"\"rest"}` |

**Rule**: If `"` is inside a value context and followed by `:`, the string *may* be a key.  Close only if the character before `"` is printable.

| Input                      | Repaired                         |
|----------------------------|----------------------------------|
| `{"key": "value": "rest"}` | `{"key": null, "value": "rest"}` |

  But if `{` or `[` appears before any structural separator when scanning backward, the quote opens a nested key — don't close.

| Input                  | Repaired                                                                                       |
|------------------------|------------------------------------------------------------------------------------------------|
| `{"a": {"b": "c": 1}}` | `{"a": {"b": "c\": 1}}` — the `"c"` is inside nested object `{"b": ...}`, not a top-level key. |

**Rule**: If `"` is followed by a bareword + `"` + `:` pattern, the first `"` closes the string and the bareword becomes part of the next key.

| Input                      | Repaired                      |
|----------------------------|-------------------------------|
| `{"text": "value"key": 1}` | `{"text": "value", "key": 1}` |

  But in key context, if no separator exists between the two quotes, the entire span is one key.

| Input               | Repaired                                                                    |
|---------------------|-----------------------------------------------------------------------------|
| `{"step"valxt": 1}` | `{"step\"valxt": 1}` — `step"valxt` is a single key with an embedded quote. |

If no close heuristic matches, the `"` is an embedded quote → emit `\"`.

| Input                          | Repaired                         |
|--------------------------------|----------------------------------|
| `{"msg":"Say "hello" please"}` | `{"msg":"Say \"hello\" please"}` |

### Single quotes

**Rule**: `'...'` becomes `"..."`.  If the next non-whitespace char after `'` is structural, close — otherwise emit `'` literally.

| Input                  | Repaired                                                          |
|------------------------|-------------------------------------------------------------------|
| `{'key': 'value'}`     | `{"key": "value"}`                                                |
| `{"key": "it's fine"}` | `{"key": "it's fine"}` — trailing `'` not structural, so literal. |

### Triple quotes

**Rule**: `"""..."""` becomes `"..."`.  Interior `"` is escaped.

| Input                             | Repaired                        |
|-----------------------------------|---------------------------------|
| `{"text": """He said "hello""""}` | `{"text": "He said \"hello\""}` |

### Missing closing quote

**Rule**: If input ends inside a string, emit `"`.

| Input                 | Repaired                |
|-----------------------|-------------------------|
| `{"key": "unfinished` | `{"key": "unfinished"}` |

### Control characters

**Rule**: Raw newlines, tabs, and control chars become `\n`, `\r`, `\t`, or `\uXXXX`.

| Input                     | Repaired                                                            |
|---------------------------|---------------------------------------------------------------------|
| `{"msg":"line1\nline2"}`  | `{"msg":"line1\\nline2"}`                                           |
| `{"key":"` + NUL + `":1}` | `{"key\u0000": 1}` — NUL in key closes string and lets `:` through. |

### Escape sequences

**Rule**: `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t` pass through.  `\uXXXX` is validated; surrogates replaced with `\ufffd`.  Any other `\X` emits `\\X`.

| Input                 | Repaired               |
|-----------------------|------------------------|
| `{"key": "\*keeper"}` | `{"key": "\\*keeper"}` |
| `{"key": "\uD800"}`   | `{"key": "\ufffd"}`    |

---

## Bareword (unquoted) handling

### Literal normalization

**Rule**: Case-insensitive match against aliases:

| Input                       | Repaired                    |
|-----------------------------|-----------------------------|
| `[True, False, None]`       | `[true, false, null]`       |
| `[yes, no, undefined, NaN]` | `[true, false, null, null]` |
| `[nil, nullptr, +infinity]` | `[null, null, null]`        |

### Unquoted strings

**Rule**: If no literal matches, the bareword is emitted as a quoted string with escaping.

| Input                      | Repaired                       |
|----------------------------|--------------------------------|
| `{"name": John}`           | `{"name": "John"}`             |
| `{"name": John "McClane"}` | `{"name": "John \"McClane\""}` |

### After-value split

**Rule**: `"value"key":` → undo the closing `"`, re-emit it, leaving `key` as the next key.

| Input                                    | Repaired                                    |
|------------------------------------------|---------------------------------------------|
| `{"text": "A longer value"next_key": 2}` | `{"text": "A longer value", "next_key": 2}` |

---

## Structural repair

### Missing commas

**Rule**: If output ends with a value (no trailing `,`, `{`, `[`) and input is a value start, emit `,`.

| Input             | Repaired           |
|-------------------|--------------------|
| `{"a": 1 "b": 2}` | `{"a": 1, "b": 2}` |
| `[1 2 3]`         | `[1, 2, 3]`        |

### Trailing commas

**Rule**: Before `}` or `]`, pop trailing `,`.

| Input       | Repaired   |
|-------------|------------|
| `{"a": 1,}` | `{"a": 1}` |
| `[1, 2,]`   | `[1, 2]`   |

### Missing values

**Rule**: In an object, `"key": }` / `"key": ]` / `"key": "nextkey":` → emit `null`.

| Input           | Repaired              |
|-----------------|-----------------------|
| `{"a": "b": 1}` | `{"a": null, "b": 1}` |
| `{"a": }`       | `{"a": null}`         |

### Bracket mismatch

**Rule**: `]` inside object or `}` inside array → emit expected closer and return.

| Input             | Repaired                                        |
|-------------------|-------------------------------------------------|
| `{"key": [1, 2]}` | `{"key": [1, 2]}` — mismatched `]` fixed to `}` |
| `[1, 2, 3}`       | `[1, 2, 3]` — mismatched `}` fixed to `]`       |

### Missing closing brackets

**Rule**: At EOF, pop and emit all open brackets.

| Input         | Repaired        |
|---------------|-----------------|
| `{"a": 1`     | `{"a": 1}`      |
| `{"a": [1, 2` | `{"a": [1, 2]}` |

### Implicit arrays

**Rule**: Adjacent top-level values ≥ 8 KB are wrapped in `[...]`.

| Input                    | Repaired             |
|--------------------------|----------------------|
| `{"a":1}{"b":2}` (≥8 KB) | `[{"a":1}, {"b":2}]` |

---

## Key detection

### Quote-based key lookout

**Rule**: When `"` starts a value, peek ahead — if the string is followed by `:`, it is a key.

| Input           | Repaired                                                                |
|-----------------|-------------------------------------------------------------------------|
| `{"a": "b": 1}` | `{"a": null, "b": 1}`                                                   |
| `["a": 1]`      | `["a\": 1]` — in array context, `: ` after quote is not treated as key. |

### Bareword key detection

**Rule**: At key position, if text starts with alphanumeric / `_` / `/` / `'`, scan ahead — if followed by `,`, `"`, `:`, or `}`, treat as key.

| Input            | Repaired           |
|------------------|--------------------|
| `{key: "value"}` | `{"key": "value"}` |
| `{_private: 1}`  | `{"_private": 1}`  |

---

## Number repair

**Rule**: Various edge-case normalizations:

| Input      | Repaired                                      |
|------------|-----------------------------------------------|
| `{.5}`     | `{0.5}`                                       |
| `{5.}`     | `{5.0}`                                       |
| `{+.5}`    | `{0.5}`                                       |
| `{-.5}`    | `{-0.5}`                                      |
| `{007}`    | `{7}`                                         |
| `{-007.5}` | `{-7.5}`                                      |
| `{1.2.3}`  | `{0}` — more than one `.` → invalid, emit `0` |
| `{123abc}` | `{0}` — followed by alpha → not a number      |

**Rule**: Time-value guard — if `:` follows the number span, treat as unquoted string.

| Input             | Repaired            |
|-------------------|---------------------|
| `{"time": 10:30}` | `{"time": "10:30"}` |

---

## Boundary and junk handling

### Comments

**Rule**: `//`, `#`, `--` → skip to newline.  `/* ... */` → skip to `*/`.

| Input                  | Repaired   |
|------------------------|------------|
| `{// comment\n"a": 1}` | `{"a": 1}` |
| `{# comment\n"a": 1}`  | `{"a": 1}` |
| `{/* block */"a": 1}`  | `{"a": 1}` |

### Trailing junk

**Rule**: After depth-0 closer, whitespace trimmed.  Non-whitespace junk is not removed.

| Input          | Repaired                        |
|----------------|---------------------------------|
| `{"a": 1}   `  | `{"a": 1}`                      |
| `{"a": 1}-lnd` | `{"a": 1}-lnd` — junk preserved |

### Adjacent objects (small)

**Rule**: Below 8 KB threshold, `}{` is treated as corrupt and kept as-is (user must manually split).

| Input                       | Repaired           |
|-----------------------------|--------------------|
| `{"a": 1}{"b": 2}` (< 8 KB) | `{"a": 1}{"b": 2}` |

### Code fences and metatags

**Rule**: Preprocessing strips ``` ```json ``` / ``` ``` and `<json>` / `</json>`.

| Input                        | Repaired   |
|------------------------------|------------|
| `` ```json\n{"a": 1}\n``` `` | `{"a": 1}` |
| `<json>{"a": 1}</json>`      | `{"a": 1}` |

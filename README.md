# json_repair

[![Security: v0.3.7+](https://img.shields.io/badge/Security-v0.3.7%2B-2ea44f?labelColor=333)](SECURITY.md)

Repair malformed JSON from LLM outputs in a **single pass** — now powered by Rust.

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
| Comments | `// comment`, `# comment`, `-- comment` | stripped |
| Truncated JSON | `{"a": 1` | `{"a": 1}` |
| Control characters | literal newline / tab | `\n` / `\t` |
| Extra text before/after | `Here is JSON: {...}` | `{...}` |
| Invalid escape sequences | `"\*keeper, \(d_i\)"` | `"\\*keeper, \\(d_i\\)"` |
| JS literals | `NaN, Infinity, undefined` | `null` |
| Implicit object sequence (≥8KB) | `{...}, {...}, {...}` | `[{...}, {...}, {...}]` |
| Trailing junk data | `{"a":1}-lnd\nuser...` | `{"a":1}` |
| Leading comma skip | `[,1]` | `[1]` |
| Dot-number normalization | `.5` / `5.` | `0.5` / `5.0` |
| Adjacent-object wrapping | `}{` (≥8KB) | `[{...},{...}]` |
| Unbraced object detection | `"key": value` | `{"key": value}` |
| Double-comma skip | `"x",,` / `[1,,2]` | `"x",` / `[1,2]` |
| Misordered-bracket fix | `[{"key": value]}` | `[{"key": value}]` |
| Brace-as-array-close | `{"a":[1}}]}` | `{"a":[1]}` |
| Unquoted string values | `{"name": John}` | `{"name": "John"}` |
| Mixed-quote boundary fix | `"text','key":"val"` | `"text","key":"val"` |
| Missing-value-after-colon fill | `{"text":` | `{"text": null}` |
| Colon misplaced in key | `"key:value"` → `"key":"value"` | auto-split |
| Missing closing quote fix | `"text","entity"` | `"text","entity"` |
| Duplicate brace skip | `{{"key": "value"}` | `{"key": "value"}` |
| Missing key opening quote | `key": value` | `"key": value` |
| Comma instead of colon after key | `"key", "value":` | `"key":null,"value":` |

## Install

### Python

```bash
pip install git+https://github.com/mengyingsui/json-repair.git
```

Or with uv:

```bash
uv add git+https://github.com/mengyingsui/json-repair.git
```

### Rust

```toml
[dependencies]
json-repair-core = { git = "https://github.com/mengyingsui/json-repair" }
```

## Usage

### Python

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

### Rust

```toml
[dependencies]
json-repair-core = { git = "https://github.com/mengyingsui/json-repair" }
```

```rust
use json_repair_core::repair_json;

fn main() {
    let broken = r#"{"response": "He said "hello" to me"}"#;
    let repaired = repair_json(broken).unwrap();
    println!("{repaired}");
    // {"response": "He said \"hello\" to me"}
}
```

## Caveat

### Python

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

### Rust

`repair_json` returns a `Result<String, JsonRepairError>`. The output is always syntactically valid JSON on `Ok`, but may contain `null` for missing values. Validate the parsed output against your expected schema.

## Architecture

```
Input text
  │
  ├─ Pre-processing (Rust)
  │    fix_colon_in_key
  │    fix_mixed_quotes
  │
  ├─ Repairer state machine (Rust)
  │    1. skip_prefix_junk
  │    2. ≥8KB {..}{..} → wrap as array
  │    3. parse_value
  │      ├─ parse_object
  │      ├─ parse_array
  │      ├─ parse_string
  │      └─ parse_literal
  │    4. close_brackets
  │    5. skip_suffix_junk (O(1) depth-tracker lookup)
  │
  └─ Repaired JSON
```

All hot-path logic runs in native Rust, exposed to Python via PyO3.

## Versions

| Version | Date | Description |
|---------|------|-------------|
| v0.3.7 🔒 | 2026-07-05 | **Fuzzer crash fixes, CI bench summary** — 3 crash fixes (STATUS_STACK_BUFFER_OVERRUN, ASAN stack overflow); surrogate sanitization; runtime bracket balance check; serde_json depth guard; JSONL/Rust crash regression tests; CI bench summary via GITHUB_STEP_SUMMARY; clippy/deny fixes. See [`SECURITY.md`](SECURITY.md) |
| v0.3.6 🔒 | 2026-07-04 | **PyO3 0.29, CI hardening** — Cargo.lock tracked; Actions bumped to checkout@v5, upload-artifact@v5; `allow_threads` → `detach` for pyo3 0.29 compat; wheel build via `uv build --wheel`; trailing-comma EOF fix from fuzzer. |
| v0.3.5 🔒 | 2026-07-04 | **Module refactoring** — repairer split into 7 submodules; Cargo feature `serde-validate`; debug assertions; refreshed benchmarks. |
| v0.3.4 🔒 | 2026-07-04 | **Leading-zero normalisation** — numbers with leading zeros stripped to RFC 8259; `is_closing_quote` comma/key hardening; numeric-corruption proptests. |
| v0.3.3 🔒 | 2026-07-04 | **Prefix junk hardening** — metadata tags `[TEXT_*]`, code fences, link parens; Cow<str> preprocessor; peek_is correctness fix; fuzz-verified. |
| v0.3.2 🔒 | 2026-07-03 | **Security-hardened release** — depth/numeric Err, ParserState enum, GIL release, fuzz, proptest, pip-audit, coverage. See [`SECURITY.md`](SECURITY.md) |
| v0.3.1 | 2026-07-03 | Security hardening — recursion depth limit, allocation fixes, CI, docs |
| v0.3.0 | 2026-07-03 | Rust rewrite — entire state machine ported from Cython to Rust via PyO3 |
| v0.2.0 | 2026-06-28 | Full Cython acceleration for all hot-path parsers |
| v0.1.17 | 2026-06-28 | SQL-style `--` line comment support |
| v0.1.16 | 2026-06-28 | Comma-after-key fix (null for missing value, comma as separator) |
| v0.1.15 | 2026-06-28 | Duplicate brace skip; missing key opening quote fix |
| v0.1.14 | 2026-06-28 | `#` line comment support |
| v0.1.13 | 2026-06-27 | Unterminated string fix |
| v0.1.12 | 2026-06-27 | Cython-accelerated `_parse_string` |
| v0.1.11 | 2026-06-27 | O(1) suffix junk cleanup |
| v0.1.10 | 2026-06-27 | Mixed-quote boundary fix; colon-in-key split |
| v0.1.9 | 2026-06-26 | Brace-as-array-close; unquoted value repair |
| v0.1.8 | 2026-06-25 | Misordered-bracket fix |
| v0.1.7 | 2026-06-23 | Unbraced-object detection; double-comma skip |
| v0.1.6 | 2026-06-23 | Single-file `_Repairer` |
| v0.1.5 | 2026-06-23 | Leading comma skip; dot-number normalization |
| v0.1.4 | 2026-06-22 | Trailing junk detection |
| v0.1.3 | 2026-06-22 | Implicit object array wrapping |
| v0.1.2 | 2026-06-22 | JS literal support |
| v0.1.1 | 2026-06-22 | Invalid escape sequence fix |
| v0.1.0 | 2026-06-22 | Initial release |

## Development

```bash
# Clone
git clone https://github.com/mengyingsui/json-repair.git
cd json_repair

# Install deps (Rust extension built automatically)
uv sync

# ── Python (performance benchmarks only) ──
uv run pytest tests/python/test_performance.py --benchmark-only

# ── Rust ──
# Unit + integration tests
cargo test -p json-repair-core

# Lint
cargo clippy -p json-repair-core --all-targets -- -D warnings

# Benchmarks
cargo bench -p json-repair-core

# Fuzz testing (requires nightly + cargo-fuzz)
# Windows: run from a Visual Studio Developer Command Prompt (x64),
# then add the Clang/LLVM ASan runtime DLL to PATH:
#   set PATH=D:\vs\community\Tools\Llvm\x64\bin;%PATH%
$env:PATH = "D:\vs\community\Tools\Llvm\x64\bin;$env:PATH"  # PowerShell
cargo +nightly fuzz run repair --fuzz-dir crates/json-repair-core/fuzz

# Rebuild Rust .pyd (after Rust changes)
uv build --wheel  # outputs to dist/
# or for editable installs:
uv run maturin develop --release -m crates/json-repair-python/Cargo.toml

# Lint / type check
uv run ruff check json_repair/ tests/python/

# ── Pre-commit (CI gate) ──
# Install hooks (runs on every commit):
uv run pre-commit install
# Run all hooks manually:
uv run pre-commit run --all-files
```

### CI Pipeline

The full CI workflow (`.github/workflows/ci.yml`) runs on every push:

| Step | What it does |
|------|-------------|
| `lint` | `ruff check`, `ruff format --check`, `cargo clippy -D warnings` |
| `test` | `cargo test` (all targets) |
| `fuzz` | `cargo +nightly fuzz run … -max_total_time=900` (15 min) |
| `wheels` | Builds `manylinux` / `musllinux` / `macos` wheels via `uv build --wheel` |
| `coverage` | `cargo llvm-cov --html`, uploaded to Codecov |
| `audit` | `cargo deny check advisories`, `pip-audit` |
| `python-test` | `uv run pytest` against the built wheel |

## License

GNU General Public License v2.0

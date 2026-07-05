# json-repair-core

Core Rust library for repairing malformed JSON from LLM outputs. Used by the [`json-repair`](https://github.com/mengyingsui/json-repair) Python package (GitHub).

## Features

- Single-pass state machine — linear time, no backtracking
- Pre-processing helpers: `fix_colon_in_key`, `fix_mixed_quotes`
- Heuristic string-closing logic tuned for LLM natural-language embedded quotes
- Modular architecture: `repairer/` submodules (`string`, `number`, `literal`, `keys`, `structure`, `comment`, `junk`)
- Cargo feature `serde-validate` (`--no-default-features` to make `serde_json` optional)
- Runtime validation for bracket balance (returns `Err` on imbalance, never panics)
- `repair_json_debug` API with extra assertions (zero-cost in release)

## Usage

```rust
use json_repair_core::repair_json;

fn main() {
    let broken = r#"{"response": "He said "hello" to me"}"#;
    let repaired = repair_json(broken).unwrap();
    println!("{repaired}");
    // {"response": "He said \"hello\" to me"}
}
```

## API

| Function | Description |
|----------|-------------|
| `repair_json(text)` | Repair malformed JSON, returns `Ok(String)` or `Err(JsonRepairError)` |
| `repair_json_debug(text)` | Like `repair_json` with extra assertions (zero-cost in release) |
| `fix_colon_in_key(text)` | Split `"key:value"` → `"key":"value"` when followed by `,` or `}` |
| `fix_mixed_quotes(text)` | Fix `','word":"` boundary between double- and single-quoted segments |

## Architecture

```
Input text
  │
  ├─ fix_colon_in_key
  ├─ fix_mixed_quotes
  ├─ Repairer state machine
  │    1. skip_prefix_junk
  │    2. ≥8KB implicit object sequence → wrap as array
  │    3. parse_value
  │      ├─ parse_object
  │      ├─ parse_array
  │      ├─ parse_string
  │      └─ parse_literal
  │    4. close_brackets
  │    5. skip_suffix_junk
  │
  └─ Repaired JSON
```

## License

GNU General Public License v2.0

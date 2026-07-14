# json-repair-core

Core Rust library for repairing malformed JSON from LLM outputs. Used by the [`json-repair`](https://github.com/mengyingsui/json-repair) Python package (GitHub).

## Features

- Single-pass state machine — linear time, no backtracking
- Single-pass preprocessor (`preprocess_json`) — fused mixed-quote + colon-in-key transforms
- Heuristic string-closing logic tuned for LLM natural-language embedded quotes
- Modular architecture: `repairer/` submodules (`string`, `number`, `literal`, `keys`, `structure`, `comment`, `sequence`)
- Cargo feature: `serde-validate` *(default)* — fast-path already-valid JSON via `serde_json`
- Configurable via [`RepairConfig`] — override max parse depth, etc.
- Typed errors via [`JsonRepairErrorKind`] — programmatically match failure modes
- Runtime validation for bracket balance (returns `Err` on imbalance, never panics)

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

### Custom configuration

```rust
use json_repair_core::{repair_json_with, RepairConfig};

// Allow deeper nesting than the default 512
let config = RepairConfig::default().with_max_depth(1024);
let repaired = repair_json_with("[[[[1]]]]", &config).unwrap();
```

### Typed error matching

```rust
use json_repair_core::{repair_json, error::JsonRepairErrorKind};

let deep = format!("{}{}", "[".repeat(600), "]".repeat(600));
match repair_json(&deep) {
    Err(e) => match e.kind() {
        JsonRepairErrorKind::DepthExceeded { max, position } => {
            println!("too deep: max={max}, at byte {position}");
        }
        JsonRepairErrorKind::UnbalancedBrackets => {
            println!("brackets could not be balanced");
        }
        _ => unreachable!("non-exhaustive enum"),
    }
    Ok(json) => println!("{json}"),
}
```

## API

| Function                          | Description                                                           |
|-----------------------------------|-----------------------------------------------------------------------|
| `repair_json(text)`               | Repair malformed JSON, returns `Ok(String)` or `Err(JsonRepairError)` |
| `repair_json_with(text, &cfg)`    | Like `repair_json` with a custom [`RepairConfig`]                     |
| `repair_json_debug(text)`         | Like `repair_json` with a debug-build idempotence check              |
| `RepairConfig::default()`         | Default config (max depth 512); use `.with_max_depth(n)` to override  |
| *(preprocessor)*                  | `preprocess_json` (internal) — single-pass mixed-quote + colon-in-key |

## Architecture

```
Input text
  │
  ├─ preprocess_json (single-pass fusion)
  ├─ Repairer state machine
  │    1. normalize_preamble
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

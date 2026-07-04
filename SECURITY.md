# Security Policy

## Security Declaration

Starting with **v0.3.4** (Python) / **v0.1.4** (Rust),
`json_repair` makes the following security guarantees:

- **No stack overflow** — iterative parser with explicit `Vec` stack;
  exceeding `MAX_PARSE_DEPTH=512` returns `Err`, never corrupts memory.
- **No silent data corruption** — numeric corruption (`123abc`) and
  structural anomalies return `Err(JsonRepairError)` with position info.
- **No GIL deadlocks** — PyO3 binding releases GIL during Rust computation;
  zero global state (`no lazy_static`/`OnceCell`).
- **No unsafe code** — all Rust code is safe Rust; no `unsafe` blocks.
- **Memory safe** — no use-after-free, no buffer overflow, no type confusion
  (PyO3 surface is minimal: one function, no `extract`/`downcast`/weak refs).
- **Always-on fuzzing** — `cargo-fuzz` target in CI guards against regressions.
- **Dependency auditing** — `cargo-deny` (advisories + licenses + bans) and
  `pip-audit` run in CI on every push.

## Supported Versions

| Package | Version | Supported |
|---------|---------|-----------|
| **json-repair** (Python) | 0.3.x | :white_check_mark: |
| | < 0.3 | :x: |
| **json-repair-core** (Rust) | 0.1.x | :white_check_mark: |
| | < 0.1 | :x: |

Security guarantees in the declaration above apply to **Python v0.3.4+** / **Rust v0.1.4+**.

## Reporting a Vulnerability

Report vulnerabilities privately to the maintainer via Gitee issues
at https://gitee.com/mensui/json_repair/issues.

Please do **not** open a public issue for security-critical bugs.

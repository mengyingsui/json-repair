# Security Policy

## Security Declaration

Starting with **v0.3.4** (Python) / **v0.1.4** (Rust),
`json_repair` makes the following security guarantees:

- **No stack overflow** — iterative parser with fixed-size stack;
  exceeding `MAX_PARSE_DEPTH=512` returns `Err`, never corrupts memory.
- **No silent data corruption** — numeric corruption (`123abc`) and
  structural anomalies return `Err(JsonRepairError)` with position info.
- **No GIL deadlocks** — PyO3 binding releases GIL during Rust computation;
  zero global state (`no lazy_static`/`OnceCell`).
- **No `unsafe` code** — zero `unsafe` blocks across the entire Rust crate.
- **Memory safe** — no use-after-free, no buffer overflow, no type confusion
  (PyO3 surface is minimal: one function, no `extract`/`downcast`/weak refs).
- **Always-on fuzzing** — `cargo-fuzz` target in CI guards against regressions.
- **Dependency auditing** — `cargo-deny` (advisories + licenses + bans) and
  `pip-audit` run in CI on every push.

## Supported Versions

| Package                     | Version | Supported          |
|-----------------------------|---------|--------------------|
| **json-repair** (Python)    | 0.3.x   | :white_check_mark: |
|                             | < 0.3   | :x:                |
| **json-repair-core** (Rust) | 0.1.x   | :white_check_mark: |
|                             | < 0.1   | :x:                |

Security guarantees in the declaration above apply to **Python v0.3.4+** / **Rust v0.1.4+**.

The module refactoring in v0.3.5 / v0.1.5 adds no new security features but
preserves all existing guarantees — the same safe‑Rust state machine with
additional `debug_assert!` guards (active in debug builds only).

The Cargo.lock tracking and CI updates in v0.3.6 / v0.1.6 are operational
changes only; security posture is unchanged.

**v0.3.10 / v0.1.10** adds:
- **No new security features** — this release is a performance optimization,
  code quality, and test infrastructure release. All security guarantees from
  v0.3.9+ / v0.1.9+ preserved. All refactored code paths pass the full test
  suite and clippy (zero warnings).
- See [`CHANGELOG.md`](CHANGELOG.md) for full details.

**v0.3.9 / v0.1.9** adds:
- **No new security features** — this release is a documentation completion
  and `is_closing_quote` refactoring release. All security guarantees from
  v0.3.8+ / v0.1.8+ preserved. Refactored code paths are covered by existing
  test coverage; behavior is equivalent (verified by full test suite +
  benchmarks).
- See [`CHANGELOG.md`](CHANGELOG.md) for full details.

**v0.3.8 / v0.1.8** adds:
- **No new security features** — this release is a hot-path maintenance and
  performance optimization release. All security guarantees from v0.3.7+ /
  v0.1.7+ preserved. Refactored code paths are covered by existing test and
  fuzz coverage.
- See [`CHANGELOG.md`](CHANGELOG.md) for full details.

**v0.3.7 / v0.1.7** adds:
- **Stack overflow guard** — serde_json recursion limit bypass removed;
  deeply nested output (>100 brackets) skips validation instead of crashing.
- **Surrogate sanitisation** — `\uXXXX` in 0xD800–0xDFFF range emitted as
  `\ufffd`; raw surrogate code points no longer appear in output.
- **Runtime bracket balance** — `is_output_balanced` `debug_assert!` replaced
  with `Err(JsonRepairError)`. Bracket imbalance now returns a proper error
  in all build profiles, not just debug.
- **Always-on number validation** — `validate_number` length bypass removed;
  all numbers validated via `serde_json` per RFC 8259.

## Reporting a Vulnerability

Report vulnerabilities privately to the maintainer via GitHub issues
at https://github.com/mengyingsui/json-repair/issues.

Please do **not** open a public issue for security-critical bugs.

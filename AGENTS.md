# json_repair — Agent instructions

## Toolchain

- **Rust** — Cargo workspace with one member: `crates/json-repair-core/` (edition 2024, MSRV 1.85).
  Run `cargo test -p json-repair-core` for all Rust tests (integration + unit).
- **Python** — managed solely by `uv` (no pip/venv directly). Use `uv sync` to create the venv + install deps, then `uv run maturin develop --uv` to install the project. `uv run <script>` to execute.
- **pre-commit** — runs in CI; `pre-commit run --all-files` locally before committing.

## Shell quirk (Windows)

`&&` is intercepted by cmd.exe before reaching pwsh. Use `;` instead:

```bash
cargo test -p json-repair-core ; cargo clippy -p json-repair-core --all-targets -- -D warnings
```

## Architecture

```
crates/
  json-repair-core/    ← the engine (single-pass streaming repairer)
  json-repair-python/  ← thin PyO3 wrapper, excluded from workspace
tests/
  cases/*.jsonl        ← all test cases (shared by Rust + Python)
  python/              ← only test_performance.py (benchmarks only)
```

- Pure Rust; Python is a thin wrapper via PyO3 (`crates/json-repair-python/`).
- Entry point: `json_repair_core::repair_json` in `crates/json-repair-core/src/lib.rs`.
- All functional and fuzz testing lives in Rust. **No Python functional tests** — `tests/python/` has only `test_performance.py` for benchmarks.
- Default feature `serde-validate` uses serde_json to fast-path already-valid JSON. Disable with `--no-default-features` to remove the dependency.

## Tests

All Rust tests: `cargo test -p json-repair-core`

| Test file            | What it tests                                       |
|----------------------|-----------------------------------------------------|
| `jsonl_cases.rs`     | Every line with `expected` in `tests/cases/*.jsonl` |
| `broken_patterns.rs` | Hand-crafted inputs — validity + idempotence        |
| `corpus_scan.rs`     | Every fuzz corpus seed produces balanced JSON       |
| `proptests.rs`       | Property-based random tests (proptest)              |
| `edge_cases.rs`      | Empty input, fuzzer regression crashes, edge cases  |
| `helpers.rs`         | Shared test utilities                               |

Single test: `cargo test -p json-repair-core --test edge_cases test_empty_input`

## Benchmarks

Both Rust (criterion) and Python (pytest-benchmark) read from the same `tests/cases/bench_data.jsonl`:

```bash
cargo bench -p json-repair-core
uv run pytest tests/python/test_performance.py --benchmark-only
```

## JSONL test data

- Every `.jsonl` file under `tests/cases/` is auto-discovered by `jsonl_cases.rs`.
- Format: `{"input":"...", "expected":...}` (lines without `expected` are skipped — used by benchmarks).
- Always update `tests/cases/INDEX.md` when adding a new `.jsonl` file.
- `bench_data.jsonl` has no `expected` field; used only by benchmarks.

## Python rebuild (after Rust changes)

`uv sync` does NOT build the Rust extension (see `[tool.uv] package = false` in `pyproject.toml`).
Always use `maturin develop` after Rust changes:

```bash
uv run maturin develop --uv             # debug build (fast for iteration)
uv run maturin develop --release --uv   # release build (for benchmarks)
```

## Lint & format

```bash
uv run ruff check json_repair/ tests/python/
uv run mypy json_repair/
uv run pyright json_repair/
cargo clippy -p json-repair-core --all-targets -- -D warnings
cargo fmt --check --all
```

## Version bump checklist

Every version bump (especially breaking) touches these files — check each:

| Area                   | File(s)                                                     |
|------------------------|-------------------------------------------------------------|
| Rust workspace version | `Cargo.toml` (`workspace.package.version`)                  |
| Python package version | `pyproject.toml`                                            |
| Lock files             | `uv.lock` (search for `json-repair` version)                |
| Root changelog         | `CHANGELOG.md`                                              |
| Core changelog         | `crates/json-repair-core/CHANGELOG.md`                      |
| Version table          | `README.md` (add row + update security badge if applicable) |
| Security history       | `SECURITY.md` (add entry tracking new guarantees)           |

Both `CHANGELOG.md` files must be updated together. Python version follows tags (`v0.3.N` → `v0.4.N`); Rust crate version is workspace-level in `Cargo.toml` (`v0.1.N` → `v0.2.N`).

## Security & safety

- **Zero `unsafe`** across the entire Rust crate.
- `#![deny(missing_docs)]` — all public items must have doc comments.
- No stack overflow: iterative parser with `MAX_PARSE_DEPTH=512` returns `Err`.
- PyO3 binding releases GIL during Rust computation.
- `SECURITY.md` tracks all security guarantees.

## Fuzz

Target at `crates/json-repair-core/fuzz/fuzz_targets/repair.rs`.
Seed corpus (force-added in git) under `fuzz/corpus/repair/`. Generated artifacts are gitignored.

```bash
cargo fuzz run repair -- -max_len=4096
cargo fuzz build
```

## CodeGraph

A `.codegraph/` index exists at the repo root. Use `codegraph explore "<question>"` before grep/read loops to find symbols and their call paths.

## CI

(GitHub Actions) runs on push to master and PRs: Rust test+clippy+bench+deny, Python ruff+mypy+pyright+pip-audit+bench. Wheels built on tags only.

"""Dev helper scripts — invoked via `uv run <entry>`.

Defined in pyproject.toml under [project.scripts].
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _run(*args: str) -> None:
    sys.exit(subprocess.call(args, cwd=str(ROOT)))


def test() -> None:
    _run("pytest", "tests/", "-v")


def bench() -> None:
    _run("pytest", "tests/test_performance.py", "--benchmark-only")


def bench_hist() -> None:
    _run("pytest", "tests/test_performance.py", "--benchmark-histogram")


def bench_compare() -> None:
    _run("pytest", "tests/test_performance.py", "--benchmark-compare")


def lint() -> None:
    _run("ruff", "check", "json_repair/", "tests/")


def typecheck() -> None:
    _run("mypy", "json_repair/", "tests/")


def precommit() -> None:
    _run("pre-commit", "run", "--all-files")

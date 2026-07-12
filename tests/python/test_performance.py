"""Performance benchmarks for json_repair.

Reads all cases from bench_data.jsonl (fixable + unfixable) and
benchmarks the speed of repair + validation for each.

Run with::

    pytest tests/python/test_performance.py --benchmark-only
    pytest tests/python/test_performance.py --benchmark-histogram
"""

from __future__ import annotations

import contextlib
import json
from pathlib import Path

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from json_repair import repair_json

CASES_DIR = Path(__file__).parent.parent / "cases"
BENCH_DATA_PATH = CASES_DIR / "bench_data.jsonl"


def _load_entries() -> list[dict[str, str]]:
    lines = BENCH_DATA_PATH.read_text(encoding="utf-8").strip().splitlines()
    return [json.loads(line) for line in lines if line.strip()]


def _repair_and_validate(text: str) -> None:
    """Repair and validate; raises on invalid output."""
    repaired = repair_json(text)
    json.loads(repaired)


def _repair_and_validate_unfixable(text: str) -> None:
    """Repair and attempt to validate; expect parse failure."""
    repaired = repair_json(text)
    with contextlib.suppress(json.JSONDecodeError):
        json.loads(repaired)


ALL_ENTRIES = _load_entries()


class TestAllCases:
    @pytest.mark.parametrize(
        "entry",
        [pytest.param(e, id=f"{e['label']}") for e in ALL_ENTRIES],
    )
    def test_repair_speed(
        self, entry: dict[str, str], benchmark: BenchmarkFixture
    ) -> None:
        inp = entry["input"]
        expected_valid = entry["expected_valid"]

        if expected_valid:
            _repair_and_validate(inp)
            benchmark(_repair_and_validate, inp)
        else:
            _repair_and_validate_unfixable(inp)
            benchmark(_repair_and_validate_unfixable, inp)

"""
Parametrized tests for json_repair — driven by .jsonl case files.
Each class-based test lives in its own file under tests/.
"""

from __future__ import annotations

import json
from typing import Any

import pytest

from json_repair import repair_json
from tests._helpers import CASES_DIR, load_inputs, run


def _all_cases() -> list[Any]:
    """Yield pytest.param for every case in every .jsonl file."""
    params: list[Any] = []
    for path in sorted(CASES_DIR.glob("*.jsonl")):
        cat = path.stem
        for i, line in enumerate(path.read_text(encoding="utf-8").strip().splitlines()):
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if "expected" not in obj:
                continue
            params.append(pytest.param(obj["input"], obj["expected"], id=f"{cat}[{i}]"))
    return params


# ── Parametrized from .jsonl files ──────────────────────────────────────


@pytest.mark.parametrize("input_str,expected", _all_cases())
def test_jsonl_cases(input_str: str, expected: object) -> None:
    run(input_str, expected)


# ── Broken patterns (validity + idempotence) ────────────────────────────


@pytest.mark.parametrize("input_str", load_inputs("broken_patterns"))
def test_broken_patterns(input_str: str) -> None:
    """Every known broken pattern should produce valid, idempotent JSON."""
    repaired = repair_json(input_str)
    assert isinstance(repaired, str)
    if not repaired:
        return
    json.loads(repaired)  # should not raise
    second = repair_json(repaired)
    assert isinstance(second, str)
    assert repaired == second, (
        f"Not idempotent:\n  first: {repaired!r}\n  second: {second!r}"
    )

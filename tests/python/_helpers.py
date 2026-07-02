"""Shared helper functions for json_repair tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from json_repair import repair_json

CASES_DIR = Path(__file__).parent.parent / "cases"


def roundtrip(text: str) -> Any:
    """Repair and parse; raise AssertionError with context on failure."""
    repaired = repair_json(text)
    assert isinstance(repaired, str)
    try:
        return json.loads(repaired)
    except json.JSONDecodeError as exc:
        raise AssertionError(
            f"Repaired JSON is still invalid:\n"
            f"  Input:    {text!r}\n"
            f"  Repaired: {repaired!r}\n"
            f"  Error:    {exc}"
        ) from exc


def load_inputs(name: str) -> list[str]:
    """Load just the input strings from a .jsonl file (no expected)."""
    path = CASES_DIR / f"{name}.jsonl"
    inputs: list[str] = []
    for line in path.read_text(encoding="utf-8").strip().splitlines():
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        inputs.append(obj["input"])
    return inputs


def run(input_str: str, expected: object) -> None:
    """Assert that repair_json(input) parses to expected."""
    result = roundtrip(input_str)
    assert result == expected, f"Expected {expected!r}, got {result!r}"

"""
Tests for json_repair — driven by .jsonl case files + in-code complex tests.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from json_repair import repair_json
from tests.check_failures import extract_blocks

CASES_DIR = Path(__file__).parent / "cases"

# ── Helpers ────────────────────────────────────────────────────────────────


def _roundtrip(text: str) -> Any:
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


def _load_inputs(name: str) -> list[str]:
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


def _run(input_str: str, expected: object) -> None:
    """Assert that repair_json(input) parses to expected."""
    result = _roundtrip(input_str)
    assert result == expected, f"Expected {expected!r}, got {result!r}"


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
                continue  # handled by a dedicated test (e.g. broken_patterns)
            params.append(pytest.param(obj["input"], obj["expected"], id=f"{cat}[{i}]"))
    return params


# ── Parametrized from .jsonl files (80 cases across 22 categories) ────────


@pytest.mark.parametrize("input_str,expected", _all_cases())
def test_jsonl_cases(input_str: str, expected: object) -> None:
    _run(input_str, expected)


# ── Broken patterns (validity + idempotence) from hypothesis cases ──────────


@pytest.mark.parametrize("input_str", _load_inputs("broken_patterns"))
def test_broken_patterns(input_str: str) -> None:
    """Every known broken pattern should produce valid, idempotent JSON."""
    repaired = repair_json(input_str)
    assert isinstance(repaired, str)
    if not repaired:
        return
    json.loads(repaired)  # should not raise
    # Idempotence
    second = repair_json(repaired)
    assert isinstance(second, str)
    assert repaired == second, (
        f"Not idempotent:\n  first: {repaired!r}\n  second: {second!r}"
    )


# ── Tests that need special logic (kept in Python) ─────────────────────────


class TestControlCharacters:
    """Carriage return test uses `in` assertion (multiple valid outcomes)."""

    def test_literal_carriage_return(self) -> None:
        result = repair_json('{"text": "line1\r\nline2"}')
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert parsed["text"] in ("line1\r\nline2", "line1\nline2")


class TestComplexScenarios:
    """Tests with partial assertions (in, multi-field checks on same input)."""

    def test_llm_response_with_quotes(self) -> None:
        text = (
            "{\n"
            '  "response": "The user said "hello" and I replied "hi there"",\n'
            '  "sentiment": "positive"\n'
            "}"
        )
        result = _roundtrip(text)
        assert result["response"] == 'The user said "hello" and I replied "hi there"'
        assert result["sentiment"] == "positive"

    def test_llm_json_with_code(self) -> None:
        text = '{"code": """def greet(name):\\n    print(f"Hello, {name}")\n"""}'
        result = _roundtrip(text)
        assert "def greet(name):" in result["code"]
        assert "Hello, {name}" in result["code"]


class TestEdgeCases:
    """Edge cases that can't live in a static .jsonl file."""

    def test_empty_input(self) -> None:
        assert repair_json("") == ""
        assert repair_json("   ") == ""

    def test_very_long_string(self) -> None:
        long_text = "A" * 10000
        result = _roundtrip(f'{{"key": "{long_text}"}}')
        assert result == {"key": long_text}


class TestImplicitArray:
    """Dynamic large inputs for heuristic-gated array wrapping."""

    def test_comma_separated_objects(self) -> None:
        obj = '{"a": 1}'
        text = ",\n".join([obj] * 20)
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, dict)

    def test_two_objects_only(self) -> None:
        text = '{"x": "hello"},\n{"y": "world"}'
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, dict)

    def test_not_triggered_for_single_object(self) -> None:
        _run(
            '{"key": "value with }, { pattern inside"}',
            {"key": "value with }, { pattern inside"},
        )

    def test_small_block_not_wrapped(self) -> None:
        _run(
            '{"a": "}, {"}',
            {"a": "}, {"},
        )

    def test_large_implicit_array(self) -> None:
        big_obj = '{"key": "' + "a" * 350 + '", "num": 42}'
        text = ",\n".join([big_obj] * 25)
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, list), f"Expected list, got {type(result)}"
        assert len(result) == 25

    def test_massive_implicit_array(self) -> None:
        failures_path = Path(__file__).parent.parent / "json_failures.txt"
        if not failures_path.exists():
            pytest.skip("json_failures.txt not found")
        text = failures_path.read_text(encoding="utf-8")
        blocks = extract_blocks(text)
        real_blocks = [b.strip() for b in blocks if b.strip() and b.strip()[0] == "{"]
        large = [b for b in real_blocks if len(b) > 50000]
        if not large:
            pytest.skip("no large block found")
        result = _roundtrip(large[0])
        assert isinstance(result, list)
        assert len(result) == 447


class TestAdjacentObjects:
    """Large dynamically generated inputs for adjacent-object wrapping."""

    def test_adjacent_objects_wrapped(self) -> None:
        big_obj = '{"key": "' + "a" * 500 + '", "num": 42}'
        text = "".join([big_obj] * 20)
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, list)
        assert len(result) == 20

    def test_adjacent_objects_mixed_commas(self) -> None:
        big_obj = '{"key": "' + "a" * 500 + '", "num": 42}'
        parts = [big_obj] * 7 + ["," + big_obj] + [big_obj] * 12
        text = "".join(parts)
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, list)
        assert len(result) == 20


class TestReturnObject:
    """Tests for the return_object=True code path."""

    def test_return_object(self) -> None:
        obj = repair_json('{"a": 1}', return_object=True)
        assert obj == {"a": 1}

    def test_return_object_invalid(self) -> None:
        with pytest.raises(ValueError):
            repair_json("not json at all", return_object=True)

    def test_return_object_empty(self) -> None:
        with pytest.raises(ValueError):
            repair_json("", return_object=True)

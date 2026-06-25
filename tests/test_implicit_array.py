"""Heuristic-gated implicit array wrapping for comma-separated objects."""

from __future__ import annotations

import json

from json_repair import repair_json
from tests._helpers import run


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
        run(
            '{"key": "value with }, { pattern inside"}',
            {"key": "value with }, { pattern inside"},
        )

    def test_small_block_not_wrapped(self) -> None:
        run(
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

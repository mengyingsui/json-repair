"""Adjacent-object wrapping (concatenated objects without commas)."""

from __future__ import annotations

import json

from json_repair import repair_json


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

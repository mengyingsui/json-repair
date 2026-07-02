"""Misordered brackets — `]` in place of `}` (object) and vice versa."""

from __future__ import annotations

from _helpers import run


class TestMisorderedBrackets:
    """Array's last object has `]` misplaced before/instead of `}`."""

    def test_bracket_instead_of_brace(self) -> None:
        run(
            '{"actions": [{"text": "a", "verb": "b", "object": "c"]}]}',
            {"actions": [{"text": "a", "verb": "b", "object": "c"}]},
        )

    def test_mixed_objects_last_broken(self) -> None:
        run(
            '{"arr": [{"x": 1}, {"y": 2}, {"z": 3]}',
            {"arr": [{"x": 1}, {"y": 2}, {"z": 3}]},
        )

    def test_swapped_brackets(self) -> None:
        run(
            '{"data": [{"id": 1, "val": "test"]}}',
            {"data": [{"id": 1, "val": "test"}]},
        )

    def test_deeply_nested(self) -> None:
        run(
            '{"a": {"b": [{"c": 1, "d": 2]}}',
            {"a": {"b": [{"c": 1, "d": 2}]}},
        )

    def test_extra_brackets_in_input(self) -> None:
        run(
            '{"items": [{"name": "x", "value": ""]}]}',
            {"items": [{"name": "x", "value": ""}]},
        )

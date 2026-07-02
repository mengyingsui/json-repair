"""Edge cases that can't live in static .jsonl files."""

from __future__ import annotations

from _helpers import roundtrip

from json_repair import repair_json


class TestEdgeCases:
    """Edge cases that can't live in a static .jsonl file."""

    def test_empty_input(self) -> None:
        assert repair_json("") == ""
        assert repair_json("   ") == ""

    def test_very_long_string(self) -> None:
        long_text = "A" * 10000
        result = roundtrip(f'{{"key": "{long_text}"}}')
        assert result == {"key": long_text}

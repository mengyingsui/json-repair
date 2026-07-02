"""Carriage return handling in JSON strings."""

from __future__ import annotations

import json

from json_repair import repair_json


class TestControlCharacters:
    """Carriage return test uses `in` assertion (multiple valid outcomes)."""

    def test_literal_carriage_return(self) -> None:
        result = repair_json('{"text": "line1\r\nline2"}')
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert parsed["text"] in ("line1\r\nline2", "line1\nline2")

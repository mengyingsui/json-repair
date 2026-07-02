"""Tests for the return_object=True code path."""

from __future__ import annotations

import pytest

from json_repair import repair_json


class TestReturnObject:
    """Tests for the return_object=True code path."""

    def test_return_object(self) -> None:
        obj = repair_json('{"a": 1}', return_object=True)
        assert obj == {"a": 1}

    def test_return_object_invalid(self) -> None:
        result = repair_json(",", return_object=True)
        assert result is None

        # Still raises for truly empty input
        with pytest.raises(ValueError):
            repair_json("", return_object=True)

    def test_return_object_empty(self) -> None:
        with pytest.raises(ValueError):
            repair_json("", return_object=True)

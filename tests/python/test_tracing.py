"""Tests for the ``traced`` parameter of ``json_repair.repair_json``.

These tests verify that ``repair_json`` returns either a plain result or a
``(result, trace)`` tuple depending on the ``traced`` flag, and that the
trace dict contains the expected event types.
"""

from __future__ import annotations

from json_repair import repair_json


def test_repair_json_default_returns_string() -> None:
    assert repair_json("Infinity") == "null"


def test_repair_json_traced_returns_tuple() -> None:
    repaired, trace = repair_json("Infinity", traced=True)
    assert repaired == "null"
    assert isinstance(trace, dict)
    assert "events" in trace
    assert len(trace["events"]) > 0


def test_repair_json_traced_event_value_normalized() -> None:
    repaired, trace = repair_json("Infinity", traced=True)
    assert repaired == "null"
    assert any(
        e["type"] == "ValueNormalized" and e["kind"] == "infinity_to_null"
        for e in trace["events"]
    )


def test_repair_json_traced_with_return_object() -> None:
    obj, trace = repair_json("{'key': 'value'}", return_object=True, traced=True)
    assert obj == {"key": "value"}
    assert isinstance(trace, dict)
    assert "events" in trace


def test_repair_json_traced_event_comment_skipped() -> None:
    _, trace = repair_json("// comment\n42", traced=True)
    assert any(e["type"] == "CommentSkipped" for e in trace["events"])

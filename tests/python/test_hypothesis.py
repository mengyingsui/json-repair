"""
Property-based tests for json_repair using Hypothesis.

Run with:
    uv run pytest tests/test_hypothesis.py -v
"""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any, TypeVar, cast

from hypothesis import assume
from hypothesis import strategies as st

from json_repair import repair_json

# Work around hypothesis stubs requiring Literal for blacklist_categories.
_CC = cast(Any, ("Cc", "Cs"))

if TYPE_CHECKING:
    _F = TypeVar("_F", bound=Callable[..., Any])

    def given(st: Any) -> Callable[[_F], _F]: ...
    def settings(**kwargs: Any) -> Callable[[_F], _F]: ...

else:
    from hypothesis import given, settings

# ── Strategies ────────────────────────────────────────────────────────────────

_json_scalars = st.one_of(
    st.none(),
    st.booleans(),
    st.integers(min_value=-1_000_000, max_value=1_000_000),
    st.floats(allow_nan=False, allow_infinity=False, width=32),
    st.text(st.characters(blacklist_characters='\\"', blacklist_categories=_CC)),
)

_json_value: st.SearchStrategy[Any] = st.recursive(
    base=_json_scalars,
    extend=lambda children: st.one_of(
        st.lists(children, max_size=5),
        st.dictionaries(
            st.text(
                st.characters(
                    blacklist_characters='\\"',
                    min_codepoint=97,
                    max_codepoint=122,
                ),
                min_size=1,
                max_size=10,
            ),
            children,
            max_size=5,
        ),
    ),
    max_leaves=20,
)

_BROKEN_RAW: list[str] = []
_path = Path(__file__).parent.parent / "cases" / "broken_patterns.jsonl"
for _line in _path.read_text(encoding="utf-8").strip().splitlines():
    if _line.strip():
        _BROKEN_RAW.append(json.loads(_line)["input"])

_BROKEN_PATTERNS = st.sampled_from(_BROKEN_RAW)


# ── Property: identity round-trip ─────────────────────────────────────────────


@given(_json_value)
@settings(max_examples=500)
def test_valid_json_passthrough(value: Any) -> None:
    """Valid JSON should pass through unchanged (modulo whitespace)."""
    valid_text = json.dumps(value, ensure_ascii=False)
    repaired = repair_json(valid_text)
    assert isinstance(repaired, str)
    result = json.loads(repaired)
    assert result == value, f"Expected {value!r}, got {result!r}"


# ── Property: always produces valid JSON ──────────────────────────────────────


@given(_BROKEN_PATTERNS)
@settings(max_examples=200)
def test_broken_produces_valid_json(text: str) -> None:
    """Every known broken pattern should produce valid, parsable JSON."""
    repaired = repair_json(text)
    assert isinstance(repaired, str)
    if repaired:
        json.loads(repaired)  # should not raise


# ── Property: idempotence ─────────────────────────────────────────────────────


@given(_BROKEN_PATTERNS | st.just('{"a": 1}'))
@settings(max_examples=100)
def test_repair_is_idempotent(text: str) -> None:
    """Repairing again should not change the result."""
    first = repair_json(text)
    assert isinstance(first, str)
    if not first:
        return
    second = repair_json(first)
    assert isinstance(second, str)
    assert first == second, (
        f"Not idempotent:\n  first:  {first!r}\n  second: {second!r}"
    )


# ── Property: string repair preserves content ──────────────────────────────────


@given(
    st.text(
        st.characters(blacklist_characters='\\"', blacklist_categories=_CC),
        min_size=1,
        max_size=100,
    )
)
@settings(max_examples=300)
def test_repair_preserves_string_content(content: str) -> None:
    """Wrapping text in valid JSON should survive repair unchanged."""
    assume('"' not in content and "\\" not in content)
    text = json.dumps({"key": content})
    repaired = repair_json(text)
    assert isinstance(repaired, str)
    result = json.loads(repaired)
    assert result["key"] == content

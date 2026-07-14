"""
Rust-accelerated JSON repair — no Python fallback.

This module wraps the ``_rust_parse`` native extension (PyO3) and provides
the public :func:`repair_json` entry point.
"""

from __future__ import annotations

import json
import re
from typing import Literal, overload

from ._rust_parse import py_repair_json as _repair_json

# Surrogate code points U+D800–U+DFFF are invalid in Rust's &str.
# Replace each with U+FFFD (REPLACEMENT CHARACTER) before the FFI call.
_SURROGATE_RE = re.compile(r"[\ud800-\udfff]")
_SURROGATE_TABLE = str.maketrans(dict.fromkeys(range(0xD800, 0xE000), "\ufffd"))


def _strip_surrogates(text: str) -> str:
    """Replace all surrogate code points (U+D800-U+DFFF) with U+FFFD.

    PyO3 cannot pass lone surrogates to Rust (``&str`` requires valid UTF-8),
    so they are stripped before the FFI call.

    Uses a fast-path regex check to avoid allocation when the input is clean,
    then falls back to :meth:`str.translate` (C-level single-pass code point
    replacement) which is faster than ``encode``/``decode`` round-tripping.
    """
    if _SURROGATE_RE.search(text) is None:
        return text
    return text.translate(_SURROGATE_TABLE)


type JsonValue = (
    dict[str, JsonValue] | list[JsonValue] | str | int | float | bool | None
)
"""Any JSON-deserializable Python value.

Recursive type alias (PEP 695): JSON objects map ``str`` keys to ``JsonValue``,
JSON arrays hold ``JsonValue`` elements, matching the RFC 8259 grammar.
"""


@overload
def repair_json(text: str, *, return_object: Literal[False] = ...) -> str: ...
@overload
def repair_json(text: str, *, return_object: Literal[True]) -> JsonValue: ...


def repair_json(text: str, *, return_object: bool = False) -> str | JsonValue:
    """Repair malformed JSON text and return valid JSON or a Python object.

    Delegates to the Rust core (via PyO3) which releases the GIL during
    computation.  Handles common JSON errors produced by LLMs: missing
    quotes, mixed quote styles, unescaped embedded quotes, trailing
    commas, truncated input, unquoted literals, comments, and more.

    Args:
        text: Malformed JSON string to repair.
        return_object: If ``True``, parse the repaired JSON and return the
            resulting Python object instead of a JSON string.

    Returns:
        A valid JSON string (default), or a parsed Python object when
        ``return_object`` is ``True``.  Empty input returns ``""`` (or
        raises if ``return_object`` is ``True``).

    Raises:
        ValueError: If *text* is empty or whitespace-only, or if the
            repaired result is still invalid JSON and ``return_object``
            is ``True``.

    Examples:
        >>> from json_repair import repair_json
        >>> repair_json('{key: value}')
        '{"key":"value"}'
        >>> repair_json("{'k': 'v'}", return_object=True)
        {'k': 'v'}
    """
    if not text or text.isspace():
        if return_object:
            raise ValueError("empty input")
        return ""

    text = _strip_surrogates(text)
    result = _repair_json(text)

    result_str: str = result

    if return_object:
        try:
            result_obj: JsonValue = json.loads(result_str)
            return result_obj
        except json.JSONDecodeError as exc:
            raise ValueError(
                f"Repaired JSON is still invalid: {exc}\n"
                f"Repaired text (first 500 chars): {result_str[:500]}"
            ) from exc

    return result_str

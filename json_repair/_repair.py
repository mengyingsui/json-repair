"""
Rust-accelerated JSON repair — no Python fallback.

This module wraps the ``_rust_parse`` native extension (PyO3) and provides
the public :func:`repair_json` entry point.
"""

from __future__ import annotations

import json
import re
from typing import Any, Literal, overload

from . import _rust_parse as _rust_parse_module
from ._rust_parse import py_format_json as _format_json
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
def repair_json(
    text: str,
    *,
    return_object: Literal[False] = ...,
    max_depth: int | None = ...,
    indent: int | None = ...,
    traced: Literal[False] = ...,
) -> str: ...
@overload
def repair_json(
    text: str,
    *,
    return_object: Literal[True],
    max_depth: int | None = ...,
    indent: int | None = ...,
    traced: Literal[False] = ...,
) -> JsonValue: ...
@overload
def repair_json(
    text: str,
    *,
    return_object: Literal[False] = ...,
    max_depth: int | None = ...,
    indent: int | None = ...,
    traced: Literal[True],
) -> tuple[str, dict[str, Any]]: ...
@overload
def repair_json(
    text: str,
    *,
    return_object: Literal[True],
    max_depth: int | None = ...,
    indent: int | None = ...,
    traced: Literal[True],
) -> tuple[JsonValue, dict[str, Any]]: ...


def repair_json(
    text: str,
    *,
    return_object: bool = False,
    max_depth: int | None = None,
    indent: int | None = None,
    traced: bool = False,
) -> str | JsonValue | tuple[str, dict[str, Any]] | tuple[JsonValue, dict[str, Any]]:
    """Repair malformed JSON text and return valid JSON or a Python object.

    Delegates to the Rust core (via PyO3) which releases the GIL during
    computation.  Handles common JSON errors produced by LLMs: missing
    quotes, mixed quote styles, unescaped embedded quotes, trailing
    commas, truncated input, unquoted literals, comments, and more.

    Args:
        text: Malformed JSON string to repair.
        return_object: If ``True``, parse the repaired JSON and return the
            resulting Python object instead of a JSON string.
        max_depth: Maximum nesting depth for objects/arrays.  ``None``
            uses the Rust default (``512``).  Deeper inputs raise
            ``ValueError``.
        indent: If provided, pretty-print the repaired JSON string with
            *indent* spaces per nesting level. Ignored when
            ``return_object`` is ``True``.
        traced: If ``True``, return a tuple ``(result, trace)`` where
            *trace* is a dict describing the repair actions performed by
            the Rust core.  When ``return_object`` is also ``True`` the
            result is the parsed Python object; otherwise it is the
            repaired JSON string.

    Returns:
        A valid JSON string (default), a parsed Python object when
        ``return_object`` is ``True``, or a tuple with one of those and a
        trace dict when ``traced`` is ``True``.  Empty input returns
        ``""`` (or raises if ``return_object`` is ``True``).

    Raises:
        ValueError: If *text* is empty or whitespace-only, if the
            repaired result is still invalid JSON and ``return_object``
            is ``True``, or if ``max_depth`` is exceeded.
        RuntimeError: If ``traced`` is ``True`` but the native extension
            was built without tracing support.

    Examples:
        >>> from json_repair import repair_json
        >>> repair_json('{key: value}')
        '{"key":"value"}'
        >>> repair_json("{'k': 'v'}", return_object=True)
        {'k': 'v'}
        >>> repair_json('{"a":1,"b":2}', indent=2)
        '{\\n  "a": 1,\\n  "b": 2\\n}'
        >>> repair_json('{key: value}', traced=True)
        ('{"key":"value"}', {'events': [...]})
    """
    if not text or text.isspace():
        if return_object:
            raise ValueError("empty input")
        if traced:
            return "", {"events": []}
        return ""

    text = _strip_surrogates(text)

    if traced:
        try:
            repaired, trace = _rust_parse_module.py_repair_json_with_trace(
                text, max_depth
            )
        except AttributeError as exc:
            raise RuntimeError(
                "tracing is not available in this build; "
                "reinstall with the tracing feature enabled"
            ) from exc

        if return_object:
            try:
                result_obj: JsonValue = json.loads(repaired)
            except json.JSONDecodeError as exc:
                raise ValueError(
                    f"Repaired JSON is still invalid: {exc}\n"
                    f"Repaired text (first 500 chars): {repaired[:500]}"
                ) from exc
            return result_obj, trace

        result_str = repaired
        if indent is not None:
            result_str = _format_json(result_str, indent)
        return result_str, trace

    repaired = _repair_json(text, max_depth)
    result_str = repaired

    if return_object:
        try:
            result_obj = json.loads(result_str)
        except json.JSONDecodeError as exc:
            raise ValueError(
                f"Repaired JSON is still invalid: {exc}\n"
                f"Repaired text (first 500 chars): {result_str[:500]}"
            ) from exc
        return result_obj

    if indent is not None:
        result_str = _format_json(result_str, indent)

    return result_str

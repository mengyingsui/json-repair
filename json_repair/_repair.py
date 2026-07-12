"""
Rust-accelerated JSON repair — no Python fallback.

This module wraps the ``_rust_parse`` native extension (PyO3) and provides
the public :func:`repair_json` entry point.
"""

from __future__ import annotations

import json
import re
from typing import Any, Literal, overload

from ._rust_parse import py_repair_json as _repair_json

_SURROGATE_RE = re.compile(r"[\ud800-\udfff]")


def _strip_surrogates(text: str) -> str:
    """Replace all surrogate code points (U+D800-U+DFFF) with U+FFFD.

    PyO3 cannot pass lone surrogates to Rust, so they are stripped before
    the FFI call.
    """
    if _SURROGATE_RE.search(text) is None:
        return text
    return text.encode("utf-8", errors="surrogatepass").decode(
        "utf-8", errors="replace"
    )


JsonValue = dict[Any, Any] | list[Any] | str | int | float | bool | None
"""Any JSON-deserializable Python value."""


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

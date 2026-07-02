"""
Rust-accelerated JSON repair — no Python fallback.
"""

from __future__ import annotations

import json

from json_repair._rust_parse import py_repair_json as _repair_json


def _strip_surrogates(text: str) -> str:
    """Replace lone surrogates (which PyO3 can't pass to Rust) with U+FFFD."""
    if not any("\ud800" <= c <= "\udfff" for c in text):
        return text
    return text.encode("utf-8", errors="surrogatepass").decode(
        "utf-8", errors="replace"
    )


def repair_json(text: str, *, return_object: bool = False) -> str | object:
    if not text or not text.strip():
        if return_object:
            raise ValueError("empty input")
        return ""

    text = _strip_surrogates(text)
    result = _repair_json(text)

    result_str: str = result

    if return_object:
        try:
            result_obj: object = json.loads(result_str)
            return result_obj
        except json.JSONDecodeError as exc:
            raise ValueError(
                f"Repaired JSON is still invalid: {exc}\n"
                f"Repaired text (first 500 chars): {result_str[:500]}"
            ) from exc

    return result_str

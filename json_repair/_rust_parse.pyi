"""Type stubs for the compiled Rust extension _rust_parse.

This extension is built by ``maturin`` from
``crates/json-repair-python/src/lib.rs`` and wraps the
:mod:`json_repair_core` Rust crate via PyO3.
"""

from typing import Any

def py_repair_json(text: str, max_depth: int | None = None) -> str:
    """Repair malformed JSON text and return a valid JSON string.

    Args:
        text: Malformed JSON string.  Must not contain lone surrogate
            code points (U+D800–U+DFFF); the Python wrapper
            :func:`json_repair.repair_json` strips them before calling
            this function.
        max_depth: Maximum nesting depth for objects/arrays.  ``None``
            uses the Rust default (``512``).

    Returns:
        A valid JSON string.

    Raises:
        ValueError: If the input is catastrophically malformed and cannot
            be repaired into valid JSON (e.g. numeric corruption,
            nesting depth exceeds the configured maximum, or the repaired
            output has unbalanced brackets).
    """
    ...

def py_format_json(text: str, indent: int) -> str:
    """Pretty-print a valid JSON string with configurable indentation.

    Args:
        text: A valid JSON string.
        indent: Number of spaces to use per nesting level.

    Returns:
        The formatted JSON string.

    Raises:
        ValueError: If *text* is not structurally valid JSON.
    """
    ...

def py_repair_json_with_trace(
    text: str, max_depth: int | None = None
) -> tuple[str, dict[str, Any]]:
    """Repair malformed JSON text and return the repaired string plus a trace.

    Args:
        text: Malformed JSON string.  Must not contain lone surrogate
            code points (U+D800–U+DFFF); the Python wrapper
            :func:`json_repair.repair_json` strips them before calling
            this function.
        max_depth: Maximum nesting depth for objects/arrays.  ``None``
            uses the Rust default (``512``).

    Returns:
        A tuple of ``(repaired_json, trace)`` where *trace* is a dict
        containing an ``events`` list of recorded repair actions.

    Raises:
        ValueError: If the input is catastrophically malformed and cannot
            be repaired into valid JSON.
    """
    ...

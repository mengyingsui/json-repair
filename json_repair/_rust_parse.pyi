"""Type stubs for the compiled Rust extension _rust_parse.

This extension is built by ``maturin`` from
``crates/json-repair-python/src/lib.rs`` and wraps the
:mod:`json_repair_core` Rust crate via PyO3.
"""

def py_repair_json(text: str) -> str:
    """Repair malformed JSON text and return a valid JSON string.

    Args:
        text: Malformed JSON string.  Must not contain lone surrogate
            code points (U+D800–U+DFFF); the Python wrapper
            :func:`json_repair.repair_json` strips them before calling
            this function.

    Returns:
        A valid JSON string.

    Raises:
        ValueError: If the input is catastrophically malformed and cannot
            be repaired into valid JSON (e.g. numeric corruption,
            nesting depth exceeds the configured maximum, or the repaired
            output has unbalanced brackets).
    """
    ...

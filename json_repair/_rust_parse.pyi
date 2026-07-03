"""
Type stubs for the compiled Rust extension _rust_parse.
"""

def py_repair_json(text: str) -> str:
    """Repair malformed JSON text and return valid JSON.

    Raises ValueError if the input is catastrophically malformed and cannot
    be repaired into valid JSON (e.g. numeric corruption, too-deep nesting).
    """
    ...

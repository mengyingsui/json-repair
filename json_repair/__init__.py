"""
json_repair — Repair malformed JSON from LLM outputs.

All processing is done in Rust via PyO3 for maximum performance.

Handles common JSON errors produced by large language models:

- Missing quotes around keys/values
- Mixed single/double quotes
- Unescaped (embedded) quotes inside string values
- Trailing commas
- Truncated JSON
- Unquoted literals (``true``, ``false``, ``null``)
- Single-line and block comments (``//``, ``/* … */``, ``#``, ``--``)
- Consecutive colons or space-separated keys

Usage::

    >>> from json_repair import repair_json
    >>> repair_json('{key: "value"}')
    '{"key":"value"}'
    >>> repair_json("{'key': 'value'}", return_object=True)
    {'key': 'value'}
"""

from json_repair._repair import repair_json as repair_json

__all__ = ["repair_json"]
__version__ = "0.3.9"

"""
json_repair â€?Repair malformed JSON from LLM outputs.

All processing is done in Rust via PyO3 for maximum performance.

Usage::

    >>> from json_repair import repair_json
    >>> repair_json('{"key": "value with "embedded" quotes"}')
    '{"key": "value with \\"embedded\\" quotes"}'
"""

from json_repair._repair import repair_json as repair_json

__all__ = ["repair_json"]
__version__ = "0.3.0"

"""String missing closing quote before comma — next key's opening quote consumed."""

from __future__ import annotations

from tests._helpers import load_inputs, roundtrip


def test_repair_missing_closing_quote() -> None:
    """Repair JSON where a text value is missing its closing ``"`` before ``,``.

    The next key's opening ``"`` (e.g. ``"entity"``) is otherwise mistaken for
    the string terminator, absorbing the trailing ``,`` into the value.
    """
    (text,) = load_inputs("unterminated_string")
    result = roundtrip(text)
    segments = result["attributes"]
    assert len(segments) == 8
    assert segments[1]["entity"] == "\u94c1\u7532\u8230"  # 铁甲舰
    assert segments[1]["text"].endswith("\u2019")  # ends with smart quote, not ,

"""Multi-segment JSON with embedded ASCII quotes in string values and entity arrays."""

from __future__ import annotations

from _helpers import load_inputs, roundtrip


class TestLargeEmbeddedQuotes:
    """Repair a large multi-segment JSON where both text values and
    entity array elements contain unescaped ASCII ``"``."""

    def test_repair_multi_segment(self) -> None:
        [text] = load_inputs("embedded_quotes_large")
        result = roundtrip(text)
        assert len(result["segments"]) == 5
        assert "阿尔杰" in result["segments"][0]["text"]
        assert result["segments"][0]["process"] == "讨论聚会的意义和潜在合作"
        assert result["segments"][-1]["process"] == "确认聚会地点并讨论塔罗牌的性质"

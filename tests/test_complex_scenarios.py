"""Complex multi-fault LLM-style JSON inputs."""

from __future__ import annotations

from tests._helpers import roundtrip


class TestComplexScenarios:
    """Tests with partial assertions (in, multi-field checks on same input)."""

    def test_llm_response_with_quotes(self) -> None:
        text = (
            "{\n"
            '  "response": "The user said "hello" and I replied "hi there"",\n'
            '  "sentiment": "positive"\n'
            "}"
        )
        result = roundtrip(text)
        assert result["response"] == 'The user said "hello" and I replied "hi there"'
        assert result["sentiment"] == "positive"

    def test_llm_json_with_code(self) -> None:
        text = '{"code": """def greet(name):\\n    print(f"Hello, {name}")\n"""}'
        result = roundtrip(text)
        assert "def greet(name):" in result["code"]
        assert "Hello, {name}" in result["code"]

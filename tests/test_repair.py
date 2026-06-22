"""
Tests for json_repair — covering common LLM JSON output errors.
"""

from __future__ import annotations

import json
from typing import Any

import pytest

from json_repair import repair_json

# ── Helper ────────────────────────────────────────────────────────────────────


def _roundtrip(text: str) -> Any:
    """Repair and parse; raise AssertionError with context on failure."""
    repaired = repair_json(text)
    assert isinstance(repaired, str)  # narrow type for mypy when return_object=False
    try:
        return json.loads(repaired)
    except json.JSONDecodeError as exc:
        raise AssertionError(
            f"Repaired JSON is still invalid:\n"
            f"  Input:    {text!r}\n"
            f"  Repaired: {repaired!r}\n"
            f"  Error:    {exc}"
        ) from exc


def _check(text: str, expected: object) -> None:
    """Repair, parse, and assert the result matches expected."""
    result = _roundtrip(text)
    assert result == expected, f"Expected {expected!r}, got {result!r}"


# ── 1. Valid JSON (passthrough) ───────────────────────────────────────────────


class TestValidPassThrough:
    def test_simple_object(self) -> None:
        _check('{"a": 1}', {"a": 1})

    def test_nested(self) -> None:
        _check('{"a": {"b": [1, 2, 3]}}', {"a": {"b": [1, 2, 3]}})

    def test_string_with_escapes(self) -> None:
        _check('{"a": "hello \\"world\\""}', {"a": 'hello "world"'})

    def test_array(self) -> None:
        _check('[1, "two", true, null]', [1, "two", True, None])

    def test_number_formats(self) -> None:
        _check("[1, -2, 3.14, 1.5e10]", [1, -2, 3.14, 1.5e10])


# ── 2. Unescaped quotes in strings ────────────────────────────────────────────


class TestUnescapedQuotes:
    def test_single_embedded_quote(self) -> None:
        _check(
            '{"key": "value with "embedded" quotes"}',
            {"key": 'value with "embedded" quotes'},
        )

    def test_multiple_embedded_quotes(self) -> None:
        _check(
            '{"text": "He said "hello" to "world""}',
            {"text": 'He said "hello" to "world"'},
        )

    def test_embedded_quote_at_start(self) -> None:
        _check(
            '{"key": ""start" with quote"}',
            {"key": '"start" with quote'},
        )

    def test_quotes_in_nested_object(self) -> None:
        _check(
            '{"outer": {"inner": "say "yes" please"}}',
            {"outer": {"inner": 'say "yes" please'}},
        )

    def test_quotes_in_array_element(self) -> None:
        _check(
            '["normal", "with "quote"", "another"]',
            ["normal", 'with "quote"', "another"],
        )


# ── 3. Triple-quoted strings ──────────────────────────────────────────────────


class TestTripleQuotedStrings:
    def test_basic_triple(self) -> None:
        _check(
            '{"key": """triple quoted value"""}',
            {"key": "triple quoted value"},
        )

    def test_triple_with_inner_quotes(self) -> None:
        _check(
            '{"key": """value with "inner" quotes"""}',
            {"key": 'value with "inner" quotes'},
        )

    def test_triple_multiline(self) -> None:
        _check(
            '{"key": """line1\nline2\nline3"""}',
            {"key": "line1\nline2\nline3"},
        )

    def test_code_in_triple(self) -> None:
        _check(
            '{"code": """def foo():\n    return "bar"\n"""}',
            {"code": 'def foo():\n    return "bar"\n'},
        )

    def test_empty_triple(self) -> None:
        _check('{"key": """"""}', {"key": ""})


# ── 4. Four-quote patterns ("""") ─────────────────────────────────────────────


class TestFourQuotePatterns:
    def test_four_quotes_surrounding_word(self) -> None:
        # """"word"""" → """ opens, "word" content, """ closes
        _check(
            '{"key": """"quoted""""}',
            {"key": '"quoted"'},
        )

    def test_four_quotes_at_value(self) -> None:
        _check(
            '{"key": """"value""""}',
            {"key": '"value"'},
        )


# ── 5. CSV-style "" escaping ──────────────────────────────────────────────────


class TestCsvStyleEscaping:
    def test_double_double_quote(self) -> None:
        _check(
            '{"data": "Column1""Data""More"}',
            {"data": 'Column1"Data"More'},
        )

    def test_csv_style_start(self) -> None:
        # """" at start = opening " + CSV "" + embedded " (third ")
        # Produces two escaped quotes at the start of content.
        _check(
            '{"key": """"quoted text"" extra"}',
            {"key": '""quoted text" extra'},
        )


# ── 6. Single-quoted strings ──────────────────────────────────────────────────


class TestSingleQuotedStrings:
    def test_single_quoted_key_and_value(self) -> None:
        _check(
            "{'key': 'value'}",
            {"key": "value"},
        )

    def test_single_quoted_with_double_inside(self) -> None:
        _check(
            "{'key': 'it has \"double\" quotes inside'}",
            {"key": 'it has "double" quotes inside'},
        )

    def test_mixed_quotes_nested(self) -> None:
        _check(
            "{'outer': {'inner': 'val'}}",
            {"outer": {"inner": "val"}},
        )


# ── 7. Unquoted keys ─────────────────────────────────────────────────────────


class TestUnquotedKeys:
    def test_simple_unquoted_key(self) -> None:
        _check("{key: 'value'}", {"key": "value"})

    def test_multiple_unquoted_keys(self) -> None:
        _check("{name: 'John', age: 30}", {"name": "John", "age": 30})

    def test_unquoted_key_with_double_quote_value(self) -> None:
        _check('{key: "value"}', {"key": "value"})


# ── 8. Trailing commas ───────────────────────────────────────────────────────


class TestTrailingCommas:
    def test_object_trailing_comma(self) -> None:
        _check('{"a": 1, "b": 2,}', {"a": 1, "b": 2})

    def test_array_trailing_comma(self) -> None:
        _check("[1, 2, 3,]", [1, 2, 3])

    def test_nested_trailing_commas(self) -> None:
        _check(
            '{"a": [1, 2,], "b": {"c": 3,},}',
            {"a": [1, 2], "b": {"c": 3}},
        )


# ── 9. Missing commas ────────────────────────────────────────────────────────


class TestMissingCommas:
    def test_missing_comma_in_object(self) -> None:
        _check('{"a": 1 "b": 2}', {"a": 1, "b": 2})

    def test_missing_comma_in_array(self) -> None:
        _check("[1 2 3]", [1, 2, 3])

    def test_missing_comma_after_string(self) -> None:
        _check('["a" "b" "c"]', ["a", "b", "c"])


# ── 10. Missing colons ───────────────────────────────────────────────────────


class TestMissingColons:
    def test_missing_colon(self) -> None:
        _check('{"key" "value"}', {"key": "value"})


# ── 11. Control characters in strings ────────────────────────────────────────


class TestControlCharacters:
    def test_literal_newline(self) -> None:
        result = repair_json('{"text": "line1\nline2"}')
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert parsed == {"text": "line1\nline2"}

    def test_literal_tab(self) -> None:
        result = repair_json('{"text": "col1\tcol2"}')
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert parsed == {"text": "col1\tcol2"}

    def test_literal_carriage_return(self) -> None:
        result = repair_json('{"text": "line1\r\nline2"}')
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert parsed["text"] in ("line1\r\nline2", "line1\nline2")


# ── 12. Comments ─────────────────────────────────────────────────────────────


class TestComments:
    def test_line_comment(self) -> None:
        _check(
            '{\n  // this is a comment\n  "key": "value"\n}',
            {"key": "value"},
        )

    def test_block_comment(self) -> None:
        _check(
            '{"key": /* inline comment */ "value"}',
            {"key": "value"},
        )


# ── 13. Extra text before / after ────────────────────────────────────────────


class TestExtraText:
    def test_prefix_text(self) -> None:
        _check(
            'Here is your JSON: {"a": 1}',
            {"a": 1},
        )

    def test_suffix_text(self) -> None:
        _check(
            '{"a": 1} Some trailing text',
            {"a": 1},
        )

    def test_markdown_code_block(self) -> None:
        result = _roundtrip('```json\n{"a": 1}\n```')
        assert result == {"a": 1}

    def test_prefix_with_newlines(self) -> None:
        _check(
            'Sure! Here is the JSON you requested:\n\n{"name": "test"}',
            {"name": "test"},
        )


# ── 14. Truncated / missing brackets ─────────────────────────────────────────


class TestTruncatedJson:
    def test_missing_closing_brace(self) -> None:
        result = _roundtrip('{"a": 1, "b": 2')
        assert result == {"a": 1, "b": 2}

    def test_missing_closing_bracket(self) -> None:
        result = _roundtrip("[1, 2, 3")
        assert result == [1, 2, 3]

    def test_unclosed_string(self) -> None:
        result = _roundtrip('{"key": "unclosed')
        assert result == {"key": "unclosed"}

    def test_deeply_nested_unclosed(self) -> None:
        result = _roundtrip('{"a": {"b": [1, 2')
        assert result == {"a": {"b": [1, 2]}}


# ── 15. Python-style literals ────────────────────────────────────────────────


class TestPythonLiterals:
    def test_python_true(self) -> None:
        _check('{"a": True}', {"a": True})

    def test_python_false(self) -> None:
        _check('{"a": False}', {"a": False})

    def test_python_none(self) -> None:
        _check('{"a": None}', {"a": None})


# ── 16. Complex / mixed scenarios ─────────────────────────────────────────────


class TestComplexScenarios:
    def test_llm_response_with_quotes(self) -> None:
        """Realistic LLM output: JSON with natural-language embedded quotes."""
        text = (
            "{\n"
            '  "response": "The user said "hello" and I replied "hi there"",\n'
            '  "sentiment": "positive"\n'
            "}"
        )
        result = _roundtrip(text)
        assert result["response"] == 'The user said "hello" and I replied "hi there"'
        assert result["sentiment"] == "positive"

    def test_llm_json_with_code(self) -> None:
        text = '{"code": """def greet(name):\\n    print(f"Hello, {name}")\n"""}'
        result = _roundtrip(text)
        assert "def greet(name):" in result["code"]
        assert "Hello, {name}" in result["code"]

    def test_mixed_single_double(self) -> None:
        text = """{'name': 'O'Brien', "city": "Dublin"}"""
        result = _roundtrip(text)
        assert result == {"name": "O'Brien", "city": "Dublin"}

    def test_many_issues_combined(self) -> None:
        text = """{
            name: 'Test',
            // a comment
            "message": "He said "try it" to me",
            "items": [1, 2, 3,],
            "nested": {'key': "value",}
            extra_key: None,
        }"""
        result = _roundtrip(text)
        assert result["name"] == "Test"
        assert result["message"] == 'He said "try it" to me'
        assert result["items"] == [1, 2, 3]
        assert result["nested"] == {"key": "value"}
        assert result["extra_key"] is None

    def test_empty_string_values(self) -> None:
        _check('{"a": "", "b": " "}', {"a": "", "b": " "})

    def test_escaped_quotes_preserved(self) -> None:
        """Already-escaped quotes should remain escaped."""
        _check(
            '{"key": "already \\"escaped\\" properly"}',
            {"key": 'already "escaped" properly'},
        )

    def test_deep_nesting(self) -> None:
        text = '{"l1": {"l2": {"l3": [{"k": "v"}]}}}'
        _check(text, {"l1": {"l2": {"l3": [{"k": "v"}]}}})


# ── 17. Edge cases ────────────────────────────────────────────────────────────


class TestEdgeCases:
    def test_empty_input(self) -> None:
        assert repair_json("") == ""
        assert repair_json("   ") == ""

    def test_only_brackets(self) -> None:
        _check("{}", {})
        _check("[]", [])

    def test_single_value(self) -> None:
        _check('"just a string"', "just a string")
        _check("123", 123)
        _check("true", True)

    def test_unicode_in_strings(self) -> None:
        _check('{"key": "你好世界"}', {"key": "你好世界"})

    def test_special_characters(self) -> None:
        _check(
            '{"key": "!@#$%^&*()_+-=[]{}|;:,.<>?/"}',
            {"key": "!@#$%^&*()_+-=[]{}|;:,.<>?/"},
        )

    def test_very_long_string(self) -> None:
        long_text = "A" * 10000
        result = _roundtrip(f'{{"key": "{long_text}"}}')
        assert result == {"key": long_text}

    def test_backslash_in_string(self) -> None:
        _check('{"path": "C:\\\\Users\\\\test"}', {"path": "C:\\Users\\test"})


# ── 18. Invalid escape sequences (\\*, \\(, etc.) ──────────────────────────────


class TestInvalidEscape:
    def test_backslash_star(self) -> None:
        _check('{"who": "\\*keeper, dwarf"}', {"who": "\\*keeper, dwarf"})

    def test_latex_parens(self) -> None:
        _check(
            '{"what": "the link offset \\(d_i\\) refers to"}',
            {"what": "the link offset \\(d_i\\) refers to"},
        )

    def test_latex_subscripts(self) -> None:
        # \t inside \theta is a valid JSON tab escape — preserved as-is.
        # \( \) and \p are invalid — their backslashes get escaped.
        _check(
            '{"params": "\\(a_i\\), \\(\\theta_i\\), \\(d_i\\), \\(\\phi_i\\)"}',
            {"params": "\\(a_i\\), \\(\theta_i\\), \\(d_i\\), \\(\\phi_i\\)"},
        )

    def test_valid_escapes_preserved(self) -> None:
        _check('{"a": "hello\\nworld\\t!"}', {"a": "hello\nworld\t!"})

    def test_backslash_quote_escape(self) -> None:
        _check('{"a": "he said \\"hello\\""}', {"a": 'he said "hello"'})

    def test_mixed_valid_invalid(self) -> None:
        _check(
            '{"text": "newline\\nhere and star\\*there"}',
            {"text": "newline\nhere and star\\*there"},
        )

    def test_invalid_escape_in_entities(self) -> None:
        _check(
            '{"entities": ["\\*keeper", "\\*dwarf", "normal"]}',
            {"entities": ["\\*keeper", "\\*dwarf", "normal"]},
        )

    def test_full_llm_failure(self) -> None:
        text = """{
            "facts": [{
                "what": "The team needs to find the first branch.",
                "who": "\\*keeper, dwarf, bear, user",
                "entities": ["\\*keeper", "dwarf", "Tree of Life"]
            }]
        }"""
        _check(
            text,
            {
                "facts": [
                    {
                        "what": "The team needs to find the first branch.",
                        "who": "\\*keeper, dwarf, bear, user",
                        "entities": ["\\*keeper", "dwarf", "Tree of Life"],
                    }
                ]
            },
        )


# ── 19. return_object=True ────────────────────────────────────────────────────


# ── 19. Implicit object sequences (}, { without outer []) ─────────────────────


class TestImplicitArray:
    def test_comma_separated_objects(self) -> None:
        # Generate a large-enough block to trigger the >8KB/≥3-match heuristic
        obj = '{"a": 1}'
        text = ",\n".join([obj] * 20)  # ~160 bytes (too small for detection)
        # For unit test, use a repair_json call directly
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        # Small blocks parse as single object (implicit array not triggered)
        result = json.loads(repaired)
        assert isinstance(result, dict)

    def test_two_objects_only(self) -> None:
        text = '{"x": "hello"},\n{"y": "world"}'
        # Small block — not wrapped, parsed as single object
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        assert isinstance(result, dict)

    def test_not_triggered_for_single_object(self) -> None:
        _check(
            '{"key": "value with }, { pattern inside"}',
            {"key": "value with }, { pattern inside"},
        )

    def test_small_block_not_wrapped(self) -> None:
        _check(
            '{"a": "}, {"}',
            {"a": "}, {"},
        )

    def test_large_implicit_array(self) -> None:
        # Build a >8KB block with ≥3 objects to trigger the heuristic
        obj = '{"what": "test ' + "x" * 200 + '", "when": "2023", "why": "test"}'
        text = ",\n".join([obj] * 6)  # ~1.5KB (still < 8KB)
        # Actually generate a big enough block
        big_obj = '{"key": "' + "a" * 350 + '", "num": 42}'
        text = ",\n".join([big_obj] * 25)  # > 9KB
        repaired = repair_json(text)
        assert isinstance(repaired, str)
        result = json.loads(repaired)
        # Should be wrapped in array: 25 objects
        assert isinstance(result, list), f"Expected list, got {type(result)}"
        assert len(result) == 25

    def test_massive_implicit_array(self) -> None:
        """Stress test: 447 objects from real LLM output (~51 KB)."""
        from pathlib import Path

        from tests.check_failures import _extract_blocks

        failures_path = Path(__file__).parent.parent / "json_failures.txt"
        if not failures_path.exists():
            pytest.skip("json_failures.txt not found")
        text = failures_path.read_text(encoding="utf-8")
        blocks = _extract_blocks(text)
        real_blocks = [b.strip() for b in blocks if b.strip() and b.strip()[0] == "{"]
        # Find the 447-item block (~51 KB)
        large = [b for b in real_blocks if len(b) > 50000]
        if not large:
            pytest.skip("no large block found")
        result = _roundtrip(large[0])
        assert isinstance(result, list)
        assert len(result) == 447


# ── 20. Trailing junk after valid JSON ──────────────────────────────────────


class TestTrailingJunk:
    def test_hyphen_word_junk(self) -> None:
        _check(
            '{"event_type":"X","role_argument_pairs":[{"role":"r","argument":"text."}]}-lnd',
            {
                "event_type": "X",
                "role_argument_pairs": [{"role": "r", "argument": "text."}],
            },
        )

    def test_word_junk_after_close(self) -> None:
        _check(
            '{"event_type":"X","role_argument_pairs":[{"role":"r","argument":"text."}]}junk',
            {
                "event_type": "X",
                "role_argument_pairs": [{"role": "r", "argument": "text."}],
            },
        )

    def test_multiline_junk(self) -> None:
        _check(
            '{"a":1}-lnd\nuser\nCRITICAL: more text\n[TEXT_START]\ncontent\n[TEXT_END]',
            {"a": 1},
        )


class TestReturnObject:
    def test_return_object(self) -> None:
        obj = repair_json('{"a": 1}', return_object=True)
        assert obj == {"a": 1}

    def test_return_object_invalid(self) -> None:
        # Input with no JSON structure at all should raise
        with pytest.raises(ValueError):
            repair_json("not json at all", return_object=True)

    def test_return_object_empty(self) -> None:
        with pytest.raises(ValueError):
            repair_json("", return_object=True)

"""
Performance benchmarks for json_repair.

Uses pytest-benchmark for statistical timing and chart output.

Run with::

    pytest tests/test_performance.py -v
    pytest tests/test_performance.py --benchmark-histogram
    pytest tests/test_performance.py -m cython_vs_pure --benchmark-histogram
"""

from __future__ import annotations

import json

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from json_repair import HAS_CYTHON, repair_json


def _assert_ok(text: str) -> None:
    """Verify repair produces valid JSON."""
    repaired = repair_json(text)
    assert isinstance(repaired, str)
    json.loads(repaired)


def _generate_valid_json(depth: int, width: int) -> str:
    if depth <= 0:
        return json.dumps(list(range(width)))
    inner = _generate_valid_json(depth - 1, max(width - 1, 1))
    items = [f'"key_{i}": {inner}' for i in range(width)]
    return "{" + ", ".join(items) + "}"


def _corrupt_strings(text: str, num_corruptions: int = 3) -> str:
    result: list[str] = []
    i = 0
    n = len(text)
    count = 0
    while i < n and count < num_corruptions:
        marker = text.find(': "', i)
        if marker == -1:
            break
        result.append(text[i : marker + 3])
        j = marker + 3
        k = j
        while k < n and text[k] != '"':
            k += 1
        if k > j + 8 and k < n:
            content = text[j:k]
            mid = len(content) // 2
            corrupted = content[:mid] + ' "oops" ' + content[mid:]
            result.append(corrupted)
            result.append('"')
            i = k + 1
            count += 1
        else:
            result.append(text[j:])
            i = n
    result.append(text[i:])
    return "".join(result)


# ── Data fixtures ──────────────────────────────────────────────────────────────

SMALL_VALID = '{"name": "Alice", "age": 30, "city": "New York"}'
MEDIUM_VALID = json.dumps(
    {
        "users": [
            {
                "id": i,
                "name": f"user_{i}",
                "email": f"user_{i}@example.com",
                "bio": f"Bio for user {i} with some text.",
                "active": i % 2 == 0,
            }
            for i in range(20)
        ],
        "meta": {"page": 1, "total": 20, "query": "search term"},
    }
)
LARGE_VALID = _generate_valid_json(depth=3, width=8)

SMALL_CORRUPT = _corrupt_strings(SMALL_VALID)
MEDIUM_CORRUPT = _corrupt_strings(MEDIUM_VALID, num_corruptions=8)
LARGE_CORRUPT = _corrupt_strings(LARGE_VALID, num_corruptions=20)

TRIPLE_QUOTED = (
    '{"code": """def hello():\n    print("Hello, World!")\n    return 42\n"""}'
)

MANY_EMBEDDED = (
    '{"dialogue": "'
    + 'He said "hello" and she replied "hi there" then he asked '
    + '"how are you" and she said "I\'m fine" '
    + 'then he said "great" and she replied "wonderful"'
    + '"}'
)

DEEP_NESTED = """\
{
    "level1": {
        "level2": {
            "level3": {
                "level4": {
                    "level5": {
                        "level6": "deep value"
                    }
                }
            }
        }
    }
}"""

REALISTIC_LLM = (
    "Here is the JSON you requested:\n\n"
    "```json\n"
    "{\n"
    '    "analysis": {\n'
    '        "summary": "The user said "I need help"'
    ' and the system responded "How can I assist?"",\n'
    '        "sentiment": "positive",\n'
    '        "entities": ["user", "system"],\n'
    '        "code_snippet": """def foo():\n'
    '    return "bar"\n'
    '"""\n'
    "    }\n"
    "}\n"
    "```"
)

LONG_EMBEDDED = (
    '{"text": "'
    + "".join(f'segment {i} with a "quote" inside. ' for i in range(200))
    + '"}'
)

LONG_PLAIN = '{"data": "' + "Some text content. " * 5000 + '"}'  # ~100 KB


# ── 1. Passthrough (valid JSON) ────────────────────────────────────────────────


class TestPassthroughPerf:
    """Valid JSON should be processed with near-zero overhead."""

    def test_small_valid(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(SMALL_VALID)
        benchmark(repair_json, SMALL_VALID)

    def test_medium_valid(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(MEDIUM_VALID)
        benchmark(repair_json, MEDIUM_VALID)

    def test_large_valid(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LARGE_VALID)
        benchmark(repair_json, LARGE_VALID)


# ── 2. Corrupted (embedded quotes) ─────────────────────────────────────────────


class TestCorruptedPerf:
    """Malformed JSON with unescaped embedded quotes — the main use case."""

    def test_small_corrupted(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(SMALL_CORRUPT)
        benchmark(repair_json, SMALL_CORRUPT)

    def test_medium_corrupted(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(MEDIUM_CORRUPT)
        benchmark(repair_json, MEDIUM_CORRUPT)

    def test_large_corrupted(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LARGE_CORRUPT)
        benchmark(repair_json, LARGE_CORRUPT)


# ── 3. Specific patterns ───────────────────────────────────────────────────────


class TestSpecificPatterns:
    def test_triple_quoted(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(TRIPLE_QUOTED)
        benchmark(repair_json, TRIPLE_QUOTED)

    def test_many_embedded_quotes(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(MANY_EMBEDDED)
        benchmark(repair_json, MANY_EMBEDDED)

    def test_deep_nested(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(DEEP_NESTED)
        benchmark(repair_json, DEEP_NESTED)

    def test_realistic_llm_output(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(REALISTIC_LLM)
        benchmark(repair_json, REALISTIC_LLM)


# ── 4. Stress tests ────────────────────────────────────────────────────────────


class TestStress:
    def test_very_long_string_value(self, benchmark: BenchmarkFixture) -> None:
        long_content = "Some text content. " * 5000
        text = f'{{"data": "{long_content}"}}'
        _assert_ok(text)
        benchmark(repair_json, text)

    def test_long_string_with_embedded_quotes(
        self, benchmark: BenchmarkFixture
    ) -> None:
        parts: list[str] = []
        for i in range(100):
            parts.append(f'This is segment {i} with a "quote" inside. ')
        long_content = "".join(parts)
        text = f'{{"text": "{long_content}"}}'
        _assert_ok(text)
        benchmark(repair_json, text)

    def test_many_small_strings(self, benchmark: BenchmarkFixture) -> None:
        items: list[str] = []
        for i in range(200):
            if i % 10 == 0:
                items.append(f'"key_{i}": "value with "quote" inside"')
            else:
                items.append(f'"key_{i}": "normal value {i}"')
        text = "{" + ", ".join(items) + "}"
        _assert_ok(text)
        benchmark(repair_json, text)

    def test_deeply_nested_array(self, benchmark: BenchmarkFixture) -> None:
        text = "[" * 50 + "1" + "]" * 50
        _assert_ok(text)
        benchmark(repair_json, text)


# ── 5. Regression — trivial cases ──────────────────────────────────────────────


class TestTrivial:
    def test_empty_object(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok("{}")
        benchmark(repair_json, "{}")

    def test_empty_array(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok("[]")
        benchmark(repair_json, "[]")

    def test_simple_number(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok("42")
        benchmark(repair_json, "42")

    def test_bare_string(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok('"hello world"')
        benchmark(repair_json, '"hello world"')


# ── 6. Cython vs pure Python benchmark ─────────────────────────────────────


@pytest.mark.skipif(
    not HAS_CYTHON,
    reason="Cython not available — comparison meaningless",
)
class TestCythonVsPure:
    """Compare Cython-accelerated path vs pure Python."""

    def _repair_pure(self, text: str) -> object:
        import json_repair._repair as _rp

        saved = _rp.HAS_CYTHON
        _rp.HAS_CYTHON = False
        try:
            return repair_json(text)
        finally:
            _rp.HAS_CYTHON = saved

    # ── Cython (fast) path ──

    def test_cython_short_embedded(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(SMALL_CORRUPT)
        benchmark(repair_json, SMALL_CORRUPT)

    def test_cython_long_embedded(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LONG_EMBEDDED)
        benchmark(repair_json, LONG_EMBEDDED)

    def test_cython_long_plain(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LONG_PLAIN)
        benchmark(repair_json, LONG_PLAIN)

    # ── Pure Python path ──

    def test_pure_short_embedded(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(SMALL_CORRUPT)
        benchmark(self._repair_pure, SMALL_CORRUPT)

    def test_pure_long_embedded(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LONG_EMBEDDED)
        benchmark(self._repair_pure, LONG_EMBEDDED)

    def test_pure_long_plain(self, benchmark: BenchmarkFixture) -> None:
        _assert_ok(LONG_PLAIN)
        benchmark(self._repair_pure, LONG_PLAIN)


# ── 7. Correctness (no benchmark) ────────────────────────────────────────────


class TestCorrectness:
    def test_repair_valid(self) -> None:
        for src in (SMALL_VALID, MEDIUM_VALID, LARGE_VALID):
            _assert_ok(src)

    def test_repair_corrupt(self) -> None:
        for src in (SMALL_CORRUPT, MEDIUM_CORRUPT, LARGE_CORRUPT):
            _assert_ok(src)

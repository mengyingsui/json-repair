"""
Performance benchmarks for json_repair.

Covers common LLM JSON output sizes and error patterns.
"""

from __future__ import annotations

import json
import timeit

import pytest

from json_repair import HAS_CYTHON, repair_json

# ── Helpers ────────────────────────────────────────────────────────────────────


def _bench(text: str, iterations: int = 500) -> float:
    """Return average execution time in milliseconds."""
    timer = timeit.Timer(lambda: repair_json(text))
    total = timer.timeit(number=iterations)
    return (total / iterations) * 1000  # ms per call


def _assert_ok(text: str) -> None:
    """Verify repair produces valid JSON."""
    repaired = repair_json(text)
    assert isinstance(repaired, str)
    json.loads(repaired)


def _generate_valid_json(depth: int, width: int) -> str:
    """Generate a syntactically valid nested JSON object."""
    if depth <= 0:
        return json.dumps(list(range(width)))
    inner = _generate_valid_json(depth - 1, max(width - 1, 1))
    items = [f'"key_{i}": {inner}' for i in range(width)]
    return "{" + ", ".join(items) + "}"


def _corrupt_strings(text: str, num_corruptions: int = 3) -> str:
    """Insert unescaped embedded quotes into string **values** only.

    Only corrupts values that follow ``: `` to avoid breaking keys.
    """
    result: list[str] = []
    i = 0
    n = len(text)
    count = 0
    while i < n and count < num_corruptions:
        # Find pattern `: "`  (colon-space-quote) indicating a string value
        marker = text.find(': "', i)
        if marker == -1:
            break
        # Emit everything up to and including `: "`
        result.append(text[i : marker + 3])
        j = marker + 3  # start of string content
        # Find the closing quote of this string value
        k = j
        while k < n and text[k] != '"':
            k += 1
        if k > j + 8 and k < n:  # only corrupt strings with meaningful content
            content = text[j:k]
            mid = len(content) // 2
            corrupted = content[:mid] + ' "oops" ' + content[mid:]
            result.append(corrupted)
            result.append('"')
            i = k + 1  # after closing quote
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


# ── 1. Passthrough (valid JSON) ────────────────────────────────────────────────


class TestPassthroughPerf:
    """Valid JSON should be processed with near-zero overhead."""

    def test_small_valid(self) -> None:
        t = _bench(SMALL_VALID, iterations=2000)
        _assert_ok(SMALL_VALID)
        assert t < 1.0, f"Small valid JSON too slow: {t:.3f} ms"

    def test_medium_valid(self) -> None:
        t = _bench(MEDIUM_VALID, iterations=500)
        _assert_ok(MEDIUM_VALID)
        assert t < 10.0, f"Medium valid JSON too slow: {t:.3f} ms"

    def test_large_valid(self) -> None:
        t = _bench(LARGE_VALID, iterations=100)
        _assert_ok(LARGE_VALID)
        # Large JSON: should process ~1 MB/s at minimum
        kb = len(LARGE_VALID) / 1024
        throughput = (kb / (t / 1000)) / 1024  # MB/s
        assert throughput > 0.5, f"Throughput too low: {throughput:.1f} MB/s"


# ── 2. Corrupted (embedded quotes) ─────────────────────────────────────────────


class TestCorruptedPerf:
    """Malformed JSON with unescaped embedded quotes — the main use case."""

    def test_small_corrupted(self) -> None:
        t = _bench(SMALL_CORRUPT, iterations=2000)
        _assert_ok(SMALL_CORRUPT)
        assert t < 2.0, f"Small corrupted too slow: {t:.3f} ms"

    def test_medium_corrupted(self) -> None:
        t = _bench(MEDIUM_CORRUPT, iterations=500)
        _assert_ok(MEDIUM_CORRUPT)
        assert t < 15.0, f"Medium corrupted too slow: {t:.3f} ms"

    def test_large_corrupted(self) -> None:
        t = _bench(LARGE_CORRUPT, iterations=100)
        _assert_ok(LARGE_CORRUPT)
        kb = len(LARGE_CORRUPT) / 1024
        throughput = (kb / (t / 1000)) / 1024
        assert throughput > 0.15, f"Throughput too low: {throughput:.1f} MB/s"


# ── 3. Specific patterns ───────────────────────────────────────────────────────


class TestSpecificPatterns:
    def test_triple_quoted(self) -> None:
        t = _bench(TRIPLE_QUOTED, iterations=2000)
        _assert_ok(TRIPLE_QUOTED)
        assert t < 2.0, f"Triple-quoted too slow: {t:.3f} ms"

    def test_many_embedded_quotes(self) -> None:
        t = _bench(MANY_EMBEDDED, iterations=2000)
        _assert_ok(MANY_EMBEDDED)
        assert t < 3.0, f"Many embedded quotes too slow: {t:.3f} ms"

    def test_deep_nested(self) -> None:
        t = _bench(DEEP_NESTED, iterations=2000)
        _assert_ok(DEEP_NESTED)
        assert t < 2.0, f"Deep nested too slow: {t:.3f} ms"

    def test_realistic_llm_output(self) -> None:
        t = _bench(REALISTIC_LLM, iterations=1000)
        _assert_ok(REALISTIC_LLM)
        assert t < 5.0, f"Realistic LLM output too slow: {t:.3f} ms"


# ── 4. Stress tests ────────────────────────────────────────────────────────────


class TestStress:
    def test_very_long_string_value(self) -> None:
        """A single string value with 100 KB of content."""
        long_content = "Some text content. " * 5000  # ~100 KB
        text = f'{{"data": "{long_content}"}}'
        t = _bench(text, iterations=50)
        _assert_ok(text)
        kb = len(text) / 1024
        throughput = (kb / (t / 1000)) / 1024
        assert throughput > 0.8, (
            f"Long string throughput too low: {throughput:.1f} MB/s"
        )

    def test_long_string_with_embedded_quotes(self) -> None:
        """A 10 KB string with embedded quotes every ~100 chars."""
        parts: list[str] = []
        for i in range(100):
            parts.append(f'This is segment {i} with a "quote" inside. ')
        long_content = "".join(parts)
        text = f'{{"text": "{long_content}"}}'
        t = _bench(text, iterations=200)
        _assert_ok(text)
        assert t < 20.0, f"Long string with embedded quotes too slow: {t:.3f} ms"

    def test_many_small_strings(self) -> None:
        """Object with 200 string keys, some with embedded quotes."""
        items: list[str] = []
        for i in range(200):
            if i % 10 == 0:
                items.append(f'"key_{i}": "value with "quote" inside"')
            else:
                items.append(f'"key_{i}": "normal value {i}"')
        text = "{" + ", ".join(items) + "}"
        t = _bench(text, iterations=200)
        _assert_ok(text)
        assert t < 30.0, f"Many small strings too slow: {t:.3f} ms"

    def test_deeply_nested_array(self) -> None:
        """Array nested 50 levels deep."""
        text = "[" * 50 + "1" + "]" * 50
        t = _bench(text, iterations=500)
        _assert_ok(text)
        assert t < 3.0, f"Deep array too slow: {t:.3f} ms"


# ── 5. Regression — verify no slowdown on trivial cases ────────────────────────


class TestTrivial:
    def test_empty_object(self) -> None:
        t = _bench("{}", iterations=10000)
        _assert_ok("{}")
        assert t < 0.1, f"Empty object too slow: {t:.3f} ms"

    def test_empty_array(self) -> None:
        t = _bench("[]", iterations=10000)
        _assert_ok("[]")
        assert t < 0.1, f"Empty array too slow: {t:.3f} ms"

    def test_simple_number(self) -> None:
        t = _bench("42", iterations=10000)
        _assert_ok("42")
        assert t < 0.1, f"Simple number too slow: {t:.3f} ms"

    def test_bare_string(self) -> None:
        t = _bench('"hello world"', iterations=10000)
        _assert_ok('"hello world"')
        assert t < 0.1, f"Bare string too slow: {t:.3f} ms"


# ── 6. Cython vs pure Python benchmark ─────────────────────────────────────


def _bench_pure(text: str, iterations: int = 500) -> float:
    """Benchmark pure Python path by temporarily disabling Cython."""
    import json_repair._repair as _rp

    saved = _rp.HAS_CYTHON
    _rp.HAS_CYTHON = False
    try:
        timer = timeit.Timer(lambda: repair_json(text))
        total = timer.timeit(number=iterations)
        return (total / iterations) * 1000
    finally:
        _rp.HAS_CYTHON = saved


LONG_EMBEDDED = (
    '{"text": "'
    + "".join(f'segment {i} with a "quote" inside. ' for i in range(200))
    + '"}'
)

LONG_PLAIN = (
    '{"data": "'
    + "Some text content. " * 5000  # ~100 KB
    + '"}'
)


@pytest.mark.skipif(
    not HAS_CYTHON, reason="Cython not available — comparison meaningless"
)
class TestCythonVsPure:
    """Compare Cython-accelerated path vs pure Python."""

    def test_short_embedded_string(self) -> None:
        t_fast = _bench(SMALL_CORRUPT, iterations=2000)
        t_pure = _bench_pure(SMALL_CORRUPT, iterations=2000)
        ratio = t_pure / t_fast
        print(f"\n  short: Cython={t_fast:.3f}ms  Pure={t_pure:.3f}ms  {ratio:.1f}×")
        assert ratio >= 0.8

    def test_long_embedded_string(self) -> None:
        t_fast = _bench(LONG_EMBEDDED, iterations=200)
        t_pure = _bench_pure(LONG_EMBEDDED, iterations=200)
        ratio = t_pure / t_fast
        print(f"\n  embedded: Cython={t_fast:.3f}ms  Pure={t_pure:.3f}ms  {ratio:.1f}×")
        assert ratio >= 0.8

    def test_long_plain_string(self) -> None:
        t_fast = _bench(LONG_PLAIN, iterations=50)
        t_pure = _bench_pure(LONG_PLAIN, iterations=50)
        ratio = t_pure / t_fast
        print(f"\n  plain:    Cython={t_fast:.3f}ms  Pure={t_pure:.3f}ms  {ratio:.1f}×")
        assert ratio >= 0.8


# ── 7. Performance report (informational, not a pass/fail test) ────────────────


@pytest.mark.skip(reason="informational — run manually with -m report")
class TestReport:
    """Generate a human-readable performance report."""

    def test_print_report(self) -> None:
        cases: list[tuple[str, str, int]] = [
            ("empty object", "{}", 5000),
            ("small valid", SMALL_VALID, 2000),
            ("small corrupt", SMALL_CORRUPT, 2000),
            ("medium valid", MEDIUM_VALID, 500),
            ("medium corrupt", MEDIUM_CORRUPT, 500),
            ("large valid", LARGE_VALID, 100),
            ("large corrupt", LARGE_CORRUPT, 100),
            ("triple-quoted", TRIPLE_QUOTED, 2000),
            ("many embedded", MANY_EMBEDDED, 2000),
            ("deep nested", DEEP_NESTED, 2000),
            ("realistic LLM", REALISTIC_LLM, 1000),
        ]
        print("\n" + "=" * 72)
        print(f"{'Case':<22} {'Size':>8} {'Time (ms)':>10} {'Throughput':>14}")
        print("-" * 72)
        for name, text, n in cases:
            t = _bench(text, iterations=n)
            size_kb = len(text) / 1024
            mbps = (size_kb / (t / 1000)) / 1024 if t > 0 else float("inf")
            print(f"{name:<22} {size_kb:>7.1f}K {t:>9.3f}ms {mbps:>12.1f} MB/s")
        print("=" * 72)

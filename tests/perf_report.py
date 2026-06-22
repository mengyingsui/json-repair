"""
Performance report script — run with:
    uv run python tests/perf_report.py
"""

from __future__ import annotations

import timeit

from json_repair import repair_json
from tests.test_performance import (
    DEEP_NESTED,
    LARGE_CORRUPT,
    LARGE_VALID,
    MANY_EMBEDDED,
    MEDIUM_CORRUPT,
    MEDIUM_VALID,
    REALISTIC_LLM,
    SMALL_CORRUPT,
    SMALL_VALID,
    TRIPLE_QUOTED,
)

INVALID_ESCAPE = (
    '{"entities": ["\\*keeper", "\\*dwarf"],'
    '"what": "the link offset \\(d_i\\) refers to"}'
)

cases: list[tuple[str, str, int]] = [
    ("empty object", "{}", 5000),
    ("small valid", SMALL_VALID, 2000),
    ("small corrupt", SMALL_CORRUPT, 2000),
    ("medium valid (2 KB)", MEDIUM_VALID, 500),
    ("medium corrupt", MEDIUM_CORRUPT, 500),
    ("large valid (12 KB)", LARGE_VALID, 100),
    ("large corrupt", LARGE_CORRUPT, 100),
    ("triple-quoted", TRIPLE_QUOTED, 2000),
    ("many embedded", MANY_EMBEDDED, 2000),
    ("deep nested", DEEP_NESTED, 2000),
    ("realistic LLM", REALISTIC_LLM, 1000),
    ("invalid escape", INVALID_ESCAPE, 2000),
]

print()
print("=" * 68)
print(f"  {'Case':<26} {'Size':>6} {'Time':>9} {'Throughput':>12}")
print("-" * 68)


def _bench(text: str, iterations: int) -> float:
    """Return average execution time in milliseconds."""
    timer = timeit.Timer(lambda: repair_json(text))
    return timer.timeit(number=iterations) / iterations * 1000


for name, text, n in cases:
    t = _bench(text, n)
    kb = len(text) / 1024
    mbps = (kb / (t / 1000)) / 1024 if t > 0 else float("inf")
    print(f"  {name:<26} {kb:>5.1f}K {t:>7.3f}ms {mbps:>10.1f} MB/s")
print("=" * 68)
print()

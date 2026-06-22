"""
Validate json_failures.txt — all JSON blocks should be repairable.

Run with:
    uv run python -m tests.check_failures
"""

from __future__ import annotations

import json
from pathlib import Path

from json_repair import repair_json

FAILURES_PATH = Path(__file__).parent.parent / "json_failures.txt"


def _extract_blocks(text: str) -> list[str]:
    """Extract JSON blocks from the failures file."""
    lines = text.splitlines(keepends=True)
    blocks: list[str] = []
    i = 0
    sep = "=" * 60
    while i < len(lines):
        line = lines[i].strip()
        if line == sep:
            i += 2  # skip error-message line
            i += 1  # skip second separator
            buf: list[str] = []
            while i < len(lines) and not lines[i].strip().startswith("="):
                buf.append(lines[i])
                i += 1
            if buf:
                blocks.append("".join(buf))
        else:
            i += 1
    return blocks


def main() -> None:
    if not FAILURES_PATH.exists():
        print(f"File not found: {FAILURES_PATH}")
        return

    text = FAILURES_PATH.read_text(encoding="utf-8")
    blocks = _extract_blocks(text)
    print(f"Found {len(blocks)} JSON blocks\n")

    success = 0
    fail = 0
    for idx, block in enumerate(blocks):
        block = block.strip()
        if not block or block[0] != "{":
            continue
        try:
            repaired = repair_json(block)
            assert isinstance(repaired, str)
            obj = json.loads(repaired)
            n_facts = len(obj.get("facts", []))
            elapsed = len(block)
            success += 1
            print(f"  [{idx + 1}] OK  {elapsed:>5} chars  {n_facts} facts")
        except (json.JSONDecodeError, ValueError) as exc:
            fail += 1
            print(f"  [{idx + 1}] FAIL  {exc}")

    print(f"\nResult: {success} fixed, {fail} failed (out of {success + fail})")


if __name__ == "__main__":
    main()

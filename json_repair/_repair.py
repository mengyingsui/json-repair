"""
Single-pass character-by-character state machine for repairing malformed JSON.
Depth tracking during parse enables O(1) suffix-junk truncation.

Handles common LLM output issues:
- Unescaped double quotes inside strings (the core problem)
- Python-style triple-quoted strings (three double-quotes)
- CSV-style double-quote escaping (two double-quotes)
- Single-quoted strings
- Unquoted object keys
- Trailing commas
- Missing commas between items
- Missing colons in objects
- Control characters in strings (newlines, tabs, etc.)
- Extra text/markdown before and after JSON
- Truncated JSON (missing closing brackets)
"""

from __future__ import annotations

import json
import re

# ── Cython acceleration (optional) ─────────────────────────────────────────


try:
    from json_repair._cparse import parse_string as _parse_string_fast

    HAS_CYTHON = True
except ImportError:
    _parse_string_fast = None
    HAS_CYTHON = False


# ── Constants ──────────────────────────────────────────────────────────────


IMPLICIT_SEQUENCE_MIN_LENGTH: int = 8192


# ── Pre-processing helpers ─────────────────────────────────────────────────


def _fix_mixed_quotes(text: str) -> str:
    """Fix mixed single/double quote boundary leaks.

    LLM output sometimes mixes quote styles: a double-quoted string value
    contains ','word":" where 'word' was originally a single-quoted key.
    This pattern inserts a closing " before ',' so the double-quoted string
    ends properly and 'word' becomes a separate key.

    Pattern:  ,'word":"  →  ","word":"
    """
    return re.sub(r"','([a-zA-Z_]\w*)\":\"", r'","\1":"', text)


def _fix_colon_in_key(text: str) -> str:
    """Fix keys that contain a colon instead of having it as a delimiter.

    Pattern:  "key:value"  →  "key":"value"
    Triggered when the string is followed by , or } — meaning the colon
    was meant to separate key and value, not be part of the key name.
    """
    return re.sub(
        r'"([a-zA-Z_][a-zA-Z0-9_]*):([a-zA-Z_][a-zA-Z0-9_]*)"\s*([,}])',
        r'"\1":"\2"\3',
        text,
    )


# ── Public API ────────────────────────────────────────────────────────────


def repair_json(text: str, *, return_object: bool = False) -> str | object:
    """Repair malformed JSON text and return a valid JSON string.

    Performs a single pass over the input, handling the most common
    formatting errors found in LLM-generated JSON.

    Args:
        text: The malformed JSON string.
        return_object: If True, parse the repaired string and return the
            resulting Python object.  Raises ValueError if parsing still fails.

    Returns:
        A valid JSON string, or (if return_object is True) the parsed object.

    Raises:
        ValueError: If return_object is True and the repaired text still
            cannot be parsed as JSON.
    """
    if not text or not text.strip():
        if return_object:
            raise ValueError("empty input")
        return ""

    text = _fix_colon_in_key(text)
    text = _fix_mixed_quotes(text)
    repairer = _Repairer(text)
    result = repairer.repair()

    if return_object:
        try:
            result_obj: object = json.loads(result)
            return result_obj
        except json.JSONDecodeError as exc:
            raise ValueError(
                f"Repaired JSON is still invalid: {exc}\n"
                f"Repaired text (first 500 chars): {result[:500]}"
            ) from exc

    return result


# ── State machine ─────────────────────────────────────────────────────────


class _Repairer:
    """Single-pass state machine with O(1) suffix-junk truncation via depth tracking."""

    __slots__ = (
        "text",
        "n",
        "i",
        "out",
        "brackets",
        "_expect_key",
        "_just_emitted_value",
        "_out_chars",
        "_last_depth0_pos",
    )

    def __init__(self, text: str) -> None:
        self.text: str = text
        self.n: int = len(text)
        self.i: int = 0
        self.out: list[str] = []
        self.brackets: list[str] = []
        self._expect_key: bool = False
        self._just_emitted_value: bool = False
        self._out_chars: int = 0
        self._last_depth0_pos: int = 0

    # ── top-level ─────────────────────────────────────────────────────────

    def repair(self) -> str:
        self._skip_prefix_junk()
        if self.i >= self.n:
            return ""
        if self._is_implicit_object_sequence():
            self._parse_implicit_array()
        else:
            self._parse_value()
        self._close_brackets()
        self._skip_suffix_junk()
        return "".join(self.out)

    def _parse_implicit_array(self) -> None:
        self._emit("[")
        first = True
        while self.i < self.n:
            self._skip_ws()
            if self.i >= self.n:
                break
            if self.text[self.i] != "{":
                break
            if not first:
                self._emit(",")
            self._parse_value()
            first = False
            self._skip_ws()
            if self.i < self.n and self.text[self.i] == ",":
                self.i += 1
        if not first and self.out and self.out[-1] == ",":
            self.out.pop()
            self._out_chars -= 1
        self._emit("]")

    # ── implicit object sequence detection ────────────────────────────────

    def _is_implicit_object_sequence(self) -> bool:
        if self.i >= self.n or self.text[self.i] != "{":
            return False
        remaining = self.n - self.i
        if remaining < IMPLICIT_SEQUENCE_MIN_LENGTH:
            return False
        j = self.i
        count = 0
        depth = 0
        in_string = False
        esc = False
        while j < self.n - 2:
            ch = self.text[j]
            if esc:
                esc = False
                j += 1
                continue
            if ch == "\\":
                esc = True
                j += 1
                continue
            if ch == '"':
                in_string = not in_string
                j += 1
                continue
            if in_string:
                j += 1
                continue
            if ch in "{[":
                depth += 1
                j += 1
                continue
            if ch in "}]":
                depth -= 1
            if ch == "}" and depth == 0:
                k = j + 1
                if k < self.n and self.text[k] == ",":
                    k += 1
                while k < self.n and self.text[k] in " \t\r\n":
                    k += 1
                if k < self.n and self.text[k] == "{":
                    count += 1
                    if count >= 3:
                        return True
                    j = k
                    continue
            j += 1
        return False

    # ── helpers ───────────────────────────────────────────────────────────

    def _peek(self, offset: int = 0) -> str:
        pos = self.i + offset
        if pos < self.n:
            return self.text[pos]
        return ""

    def _peek_str(self, length: int) -> str:
        return self.text[self.i : self.i + length]

    def _emit(self, s: str) -> None:
        self.out.append(s)
        self._out_chars += len(s)

    def _skip_ws(self) -> None:
        while self.i < self.n and self.text[self.i] in " \t\r\n":
            self.i += 1

    def _skip_ws_emit(self) -> None:
        while self.i < self.n and self.text[self.i] in " \t\r\n":
            self._emit(self.text[self.i])
            self.i += 1

    def _close_brackets(self) -> None:
        while self.brackets:
            self._emit(self.brackets.pop())
        self._last_depth0_pos = self._out_chars

    _VALID_ESCAPES = frozenset('"\\/bfnrt')

    def _emit_escape(self, ch: str) -> None:
        if ch in self._VALID_ESCAPES:
            self._emit("\\")
            self._emit(ch)
        elif ch == "u" and self._peek(1) in "0123456789abcdefABCDEF":
            self._emit("\\")
            self._emit("u")
        else:
            self._emit("\\\\")
            self._emit(ch)

    # ── prefix / suffix junk ──────────────────────────────────────────────

    def _skip_prefix_junk(self) -> None:
        fence_stripped = self.text.lstrip()
        prefix_len = len(self.text) - len(fence_stripped)
        if fence_stripped.startswith("```"):
            newline_pos = fence_stripped.find("\n")
            if newline_pos != -1:
                self.text = fence_stripped[newline_pos + 1 :]
                self.n = len(self.text)
            else:
                self.text = fence_stripped[3:]
                self.n = len(self.text)
        elif prefix_len > 0:
            self.text = fence_stripped
            self.n = len(self.text)

        saved = self.i
        unbraced_start = -1
        while self.i < self.n:
            ch = self.text[self.i]
            if ch in "{[":
                if unbraced_start != -1:
                    self.text = "{" + self.text[unbraced_start:] + "}"
                    self.n = len(self.text)
                    self.i = 0
                    return
                break
            if ch == '"':
                str_start = self.i
                self.i += 1
                while self.i < self.n:
                    if self.text[self.i] == "\\":
                        self.i += 2
                    elif self.text[self.i] == '"':
                        self.i += 1
                        break
                    else:
                        self.i += 1
                j = self.i
                while j < self.n and self.text[j] in " \t\r\n":
                    j += 1
                if j < self.n and self.text[j] == ":" and unbraced_start == -1:
                    unbraced_start = str_start
            else:
                self.i += 1
        if self.i >= self.n:
            self.i = saved

    def _skip_suffix_junk(self) -> None:
        result = "".join(self.out)
        if self._last_depth0_pos < len(result):
            tail = result[self._last_depth0_pos :]
            if tail.strip():
                result = result[: self._last_depth0_pos]
        self.out.clear()
        self.out.append(result)

    # ── string (double-quoted) ────────────────────────────────────────────

    def _parse_string(self) -> None:
        if HAS_CYTHON:
            new_i, added = _parse_string_fast(self.text, self.i, self.n, self.out)
            self.i = new_i
            self._out_chars += added
            return

        self._emit('"')
        self.i += 1

        while self.i < self.n:
            ch = self.text[self.i]

            if ch == "\\":
                self.i += 1
                if self.i < self.n:
                    self._emit_escape(self.text[self.i])
                    self.i += 1
                else:
                    self._emit("\\\\")
                continue

            if ch == '"':
                if self._peek(1) == '"':
                    self._emit('\\"')
                    self.i += 1
                    j = self.i + 1
                    while j < self.n and self.text[j] in " \t\r":
                        j += 1
                    if j < self.n and self.text[j] in ",}:]\n":
                        continue
                    else:
                        self.i += 1
                        if self.i < self.n and self.text[self.i] == '"':
                            self._emit('\\"')
                            self.i += 1
                        continue

                if self._is_closing_quote():
                    self._emit('"')
                    nc = self._peek(1)
                    if nc.isalpha() or nc == "_":
                        k = self.i + 1
                        while k < self.n and (
                            self.text[k].isalnum() or self.text[k] == "_"
                        ):
                            k += 1
                        while k < self.n and self.text[k] in " \t\r\n":
                            k += 1
                        if k < self.n and self.text[k] == '"':
                            self.out.pop()
                            self._out_chars -= 1
                            while self.out and self.out[-1] in " \t\r\n":
                                self.out.pop()
                                self._out_chars -= 1
                            if self.out and self.out[-1] == ",":
                                self.out.pop()
                                self._out_chars -= 1
                            self._emit('"')
                            return
                    self.i += 1
                    return
                else:
                    self._emit('\\"')
                    self.i += 1
                    continue

            if ch == "\n":
                self._emit("\\n")
                self.i += 1
                continue

            if ch == "\r":
                self._emit("\\r")
                self.i += 1
                continue

            if ch == "\t":
                self._emit("\\t")
                self.i += 1
                continue

            if ord(ch) < 0x20:
                self._emit(f"\\u{ord(ch):04x}")
                self.i += 1
                continue

            self._emit(ch)
            self.i += 1

        if not self.out or self.out[-1] != '"':
            self._emit('"')

    def _is_closing_quote(self) -> bool:
        j = self.i + 1
        while j < self.n and self.text[j] in " \t\r":
            j += 1

        if j >= self.n:
            return True

        nc = self.text[j]

        if nc in ",}]:\n":
            return True

        if nc == '"':
            return True

        if nc.isalpha() or nc == "_":
            k = j
            while k < self.n and (self.text[k].isalnum() or self.text[k] == "_"):
                k += 1
            while k < self.n and self.text[k] in " \t\r\n":
                k += 1
            if k < self.n and self.text[k] == '"':
                k += 1
            while k < self.n and self.text[k] in " \t\r\n":
                k += 1
            if k < self.n and self.text[k] == ":":
                return True

        return False

    # ── triple-quoted string ──────────────────────────────────────────────

    def _parse_triple_string(self) -> None:
        self.i += 3
        self._emit('"')

        while self.i < self.n:
            if self._peek_str(3) == '"""':
                after = self.i + 3
                if after < self.n and self.text[after] == '"':
                    pass
                else:
                    self.i += 3
                    self._emit('"')
                    self._just_emitted_value = True
                    return

            ch = self.text[self.i]

            if ch == "\\":
                self.i += 1
                if self.i < self.n:
                    self._emit_escape(self.text[self.i])
                    self.i += 1
                else:
                    self._emit("\\\\")
                continue

            if ch == '"':
                self._emit('\\"')
                self.i += 1
                continue

            if ch == "\n":
                self._emit("\\n")
                self.i += 1
                continue

            if ch == "\r":
                self._emit("\\r")
                self.i += 1
                continue

            if ch == "\t":
                self._emit("\\t")
                self.i += 1
                continue

            if ord(ch) < 0x20:
                self._emit(f"\\u{ord(ch):04x}")
                self.i += 1
                continue

            self._emit(ch)
            self.i += 1

        self._emit('"')

    # ── single-quoted string ──────────────────────────────────────────────

    def _parse_single_quoted_string(self) -> None:
        self._emit('"')
        self.i += 1

        while self.i < self.n:
            ch = self.text[self.i]

            if ch == "\\":
                if self._peek(1) == "'":
                    self._emit("'")
                    self.i += 2
                    continue
                self.i += 1
                if self.i < self.n:
                    self._emit_escape(self.text[self.i])
                    self.i += 1
                else:
                    self._emit("\\\\")
                continue

            if ch == "'":
                j = self.i + 1
                while j < self.n and self.text[j] in " \t\r":
                    j += 1
                if j >= self.n or self.text[j] in ",}:]\n":
                    self._emit('"')
                    self.i += 1
                    self._just_emitted_value = True
                    return
                else:
                    self._emit("'")
                    self.i += 1
                    continue

            if ch == '"':
                self._emit('\\"')
                self.i += 1
                continue

            if ch == "\n":
                self._emit("\\n")
                self.i += 1
                continue

            if ch == "\r":
                self._emit("\\r")
                self.i += 1
                continue

            if ch == "\t":
                self._emit("\\t")
                self.i += 1
                continue

            if ord(ch) < 0x20:
                self._emit(f"\\u{ord(ch):04x}")
                self.i += 1
                continue

            self._emit(ch)
            self.i += 1

        self._emit('"')

    # ── value dispatch ────────────────────────────────────────────────────

    def _parse_value(self) -> None:
        self._skip_ws()
        if self.i >= self.n:
            self._emit("null")
            return

        ch = self.text[self.i]

        if ch == "{":
            self._parse_object()
        elif ch == "[":
            self._parse_array()
        elif ch == '"':
            if self._peek_str(3) == '"""':
                rest = self.text[self.i + 3 :]
                if '"""' in rest:
                    self._parse_triple_string()
                    return
            self._parse_string()
        elif ch == "'":
            self._parse_single_quoted_string()
        elif ch in "tfnTFNnNiIuU":
            self._parse_literal()
        elif ch == "-" and self._peek_str(2) == "--":
            self._skip_comment()
            self._parse_value()
        elif ch in "-.0123456789":
            self._parse_number()
        elif ch in "/#":
            self._skip_comment()
            self._parse_value()
        elif ch in "}]" or ch == ",":
            self._emit("null")
        elif self._expect_key:
            if ch.isalpha() or ch == "_":
                self._parse_unquoted_key()
            else:
                return
        elif ch.isalpha() or ch == "_":
            self._parse_unquoted_value()
        else:
            self.i += 1
            self._parse_value()

    # ── object ────────────────────────────────────────────────────────────

    def _parse_object(self) -> None:
        self._emit("{")
        self.brackets.append("}")
        self.i += 1

        prev_expect = self._expect_key
        self._expect_key = True
        first = True

        while self.i < self.n:
            self._skip_ws()
            if self.i >= self.n:
                break

            ch = self.text[self.i]

            if ch == "{" and self._expect_key:
                self.i += 1
                continue

            if ch == ":" and self._expect_key:
                self.i += 1
                continue

            if ch == "}":
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                    self._out_chars -= 1
                self._emit("}")
                self.brackets.pop()
                if not self.brackets:
                    self._last_depth0_pos = self._out_chars
                self.i += 1
                self._expect_key = prev_expect
                self._just_emitted_value = True
                return

            if ch == ",":
                if not first and (not self.out or self.out[-1] != ","):
                    self._emit(",")
                self.i += 1
                self._expect_key = True
                continue

            if ch in "/#" or (ch == "-" and self._peek_str(2) == "--"):
                self._skip_comment()
                continue

            if ch == '"' and self._just_emitted_value:
                j = self.i + 1
                while j < self.n and self.text[j] in " \t\r\n":
                    j += 1
                if j >= self.n or self.text[j] in "},]:":
                    self.i += 1
                    continue

            if ch == "]":
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                    self._out_chars -= 1
                self._emit("}")
                self.brackets.pop()
                if not self.brackets:
                    self._last_depth0_pos = self._out_chars
                self._expect_key = prev_expect
                return

            if self._expect_key:
                if not first and ch not in "\"_/'" and not ch.isalpha():
                    break
                if ch.isalpha():
                    j = self.i + 1
                    while j < self.n and self.text[j] in (
                        "abcdefghijklmnopqrstuvwxyz"
                        "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                        "0123456789_"
                    ):
                        j += 1
                    while j < self.n and self.text[j] in " \t\r":
                        j += 1
                    if j >= self.n or self.text[j] not in ':",':
                        break
                if (
                    not first
                    and self._just_emitted_value
                    and self.out
                    and self.out[-1] not in ",{["
                ):
                    self._emit(",")
                self._parse_key()
                self._skip_ws()
                if self.i < self.n and self.text[self.i] == ":":
                    self._emit(":")
                    self.i += 1
                elif self.i < self.n and self.text[self.i] != ":":
                    self._emit(":")
                self._expect_key = False
                self._parse_value()
                self._expect_key = True
                self._just_emitted_value = True
            else:
                if not first and ch not in "\"{['tfnTFNnNiIuU-0123456789":
                    break
                if (
                    not first
                    and self._just_emitted_value
                    and self.out
                    and self.out[-1] not in ",{["
                    and ch not in "}],"
                ):
                    self._emit(",")
                self._parse_value()
                self._just_emitted_value = True

            first = False

        self._expect_key = prev_expect

    # ── array ─────────────────────────────────────────────────────────────

    def _parse_array(self) -> None:
        self._emit("[")
        self.brackets.append("]")
        self.i += 1

        first = True
        while self.i < self.n:
            self._skip_ws()
            if self.i >= self.n:
                break

            ch = self.text[self.i]

            if ch == "]":
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                    self._out_chars -= 1
                self._emit("]")
                self.brackets.pop()
                if not self.brackets:
                    self._last_depth0_pos = self._out_chars
                self.i += 1
                self._just_emitted_value = True
                return

            if ch == "}":
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                    self._out_chars -= 1
                self._emit("]")
                self.brackets.pop()
                if not self.brackets:
                    self._last_depth0_pos = self._out_chars
                self.i += 1
                self._just_emitted_value = True
                return

            if ch == ",":
                if not first and (not self.out or self.out[-1] != ","):
                    self._emit(",")
                self.i += 1
                continue

            if ch in "/#" or (ch == "-" and self._peek_str(2) == "--"):
                self._skip_comment()
                continue

            if (
                not first
                and self._just_emitted_value
                and self.out
                and self.out[-1] not in ",[:"
                and ch not in "]"
            ):
                self._emit(",")

            self._parse_value()
            self._just_emitted_value = True
            first = False

    # ── key ───────────────────────────────────────────────────────────────

    def _parse_key(self) -> None:
        self._skip_ws()
        if self.i >= self.n:
            return

        ch = self.text[self.i]

        if ch == '"':
            self._parse_string()
        elif ch == "'":
            self._parse_single_quoted_string()
        else:
            self._parse_unquoted_key()

    def _parse_unquoted_key(self) -> None:
        self._emit('"')
        while self.i < self.n and self.text[self.i] not in " \t\r\n:{}[],\"'/":
            self._emit(self.text[self.i])
            self.i += 1
        self._emit('"')
        if self.i < self.n and self.text[self.i] == '"':
            self.i += 1

    def _parse_unquoted_value(self) -> None:
        self._emit('"')
        while self.i < self.n and self.text[self.i] not in ",}]":
            ch = self.text[self.i]
            if ch == "\\":
                self._emit("\\\\")
            elif ch == '"':
                self._emit('\\"')
            elif ord(ch) < 0x20:
                self._emit(f"\\u{ord(ch):04x}")
            else:
                self._emit(ch)
            self.i += 1
        self._emit('"')
        self._just_emitted_value = True

    # ── literals & numbers ────────────────────────────────────────────────

    def _parse_literal(self) -> None:
        lower = self.text[self.i : self.i + 9].lower()

        if lower.startswith("true"):
            self._emit("true")
            self.i += 4
        elif lower.startswith("false"):
            self._emit("false")
            self.i += 5
        elif lower.startswith("null") or lower.startswith("none"):
            self._emit("null")
            self.i += 4
        elif lower.startswith("undefined"):
            self._emit("null")
            self.i += 9
        elif lower.startswith("nan"):
            self._emit("null")
            self.i += 3
        elif lower.startswith("infinity") or lower.startswith("+infinity"):
            self._emit("null")
            self.i += 8
        elif lower.startswith("-infinity"):
            self._emit("null")
            self.i += 9
        else:
            self._parse_unquoted_value()
            return

        self._just_emitted_value = True

    def _parse_number(self) -> None:
        start = self.i
        while self.i < self.n and self.text[self.i] in "-0123456789.eE+":
            self.i += 1
        num_str = self.text[start : self.i]
        if num_str.startswith("."):
            num_str = "0" + num_str
        elif num_str.startswith("-."):
            num_str = "-0." + num_str[2:]
        elif num_str.startswith("+."):
            num_str = "+0." + num_str[2:]
        if num_str.endswith("."):
            num_str += "0"
        try:
            float(num_str)
        except ValueError:
            self._emit("0")
            self._just_emitted_value = True
            return
        self._emit(num_str)
        self._just_emitted_value = True

    # ── comments ──────────────────────────────────────────────────────────

    def _skip_comment(self) -> None:
        if self._peek_str(2) == "//":
            while self.i < self.n and self.text[self.i] != "\n":
                self.i += 1
            if self.i < self.n:
                self.i += 1
        elif self._peek_str(2) == "/*":
            self.i += 2
            while self.i + 1 < self.n:
                if self.text[self.i] == "*" and self.text[self.i + 1] == "/":
                    self.i += 2
                    return
                self.i += 1
        elif self.text[self.i] == "#":
            # # comments are silently stripped — the item is kept as-is
            while self.i < self.n and self.text[self.i] != "\n":
                self.i += 1
            if self.i < self.n:
                self.i += 1
        elif self._peek_str(2) == "--":
            while self.i < self.n and self.text[self.i] != "\n":
                self.i += 1
            if self.i < self.n:
                self.i += 1
        else:
            self.i += 1

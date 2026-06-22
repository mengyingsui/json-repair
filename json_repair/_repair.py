"""
Single-pass state machine for repairing malformed JSON.

Handles common LLM output issues:
- Unescaped double quotes inside strings (the core problem)
- Python-style triple-quoted strings (\"\"\")
- CSV-style double-quote escaping (\"\")
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

# ── Public API ────────────────────────────────────────────────────────────────


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


# ── State machine ─────────────────────────────────────────────────────────────


class _Repairer:
    """Character-by-character state machine that repairs JSON in one pass."""

    __slots__ = (
        "text",
        "n",
        "i",
        "out",
        "brackets",
        "_expect_key",
        "_just_emitted_value",
    )

    def __init__(self, text: str) -> None:
        self.text: str = text
        self.n: int = len(text)
        self.i: int = 0
        self.out: list[str] = []
        self.brackets: list[str] = []  # expected closing brackets
        self._expect_key: bool = False  # inside object, expecting a key
        self._just_emitted_value: bool = False  # last thing emitted was a value

    # ── top-level ─────────────────────────────────────────────────────────

    def repair(self) -> str:
        """Run the full repair and return the result string."""
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

    def _is_implicit_object_sequence(self) -> bool:
        """Check if the text is a comma-separated sequence of objects.

        Pattern: ``{...}, { ... }, { ... }`` without an outer ``[...]``.
        Only activates for large inputs (> 8 KB) with multiple occurrences
        of ``},`` followed by ``{``, to avoid false positives from small
        blocks where ``}, {`` appears inside string content.
        """
        if self.i >= self.n or self.text[self.i] != "{":
            return False
        # Only check blocks above a size threshold.
        remaining = self.n - self.i
        if remaining < 8192:
            return False
        # Count structural `},` → `{` patterns at depth 0 (string-aware).
        #  Depth-tracking ensures that `}, {` inside a valid `[...]` array
        #  does not trigger a false positive.
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
            # Check for }, { at depth 0 AFTER updating depth for }
            if ch == "}" and depth == 0:
                k = j + 1
                # Optional comma between objects
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

    def _parse_implicit_array(self) -> None:
        """Parse ``{...}, {...}, {...}`` as ``[{...}, {...}, {...}]``."""
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
            # Consume trailing comma between objects
            self._skip_ws()
            if self.i < self.n and self.text[self.i] == ",":
                self.i += 1
        # Remove trailing comma before closing bracket
        if not first and self.out and self.out[-1] == ",":
            self.out.pop()
        self._emit("]")

    # ── helpers ───────────────────────────────────────────────────────────

    def _peek(self, offset: int = 0) -> str:
        pos = self.i + offset
        if pos < self.n:
            return self.text[pos]
        return ""

    def _peek_str(self, length: int) -> str:
        """Return up to `length` chars starting at current position."""
        return self.text[self.i : self.i + length]

    def _emit(self, s: str) -> None:
        self.out.append(s)

    def _skip_ws(self) -> None:
        """Advance past whitespace (no output)."""
        while self.i < self.n and self.text[self.i] in " \t\r\n":
            self.i += 1

    def _skip_ws_emit(self) -> None:
        """Advance past whitespace, emitting it."""
        while self.i < self.n and self.text[self.i] in " \t\r\n":
            self._emit(self.text[self.i])
            self.i += 1

    def _close_brackets(self) -> None:
        """Append any missing closing brackets."""
        while self.brackets:
            self._emit(self.brackets.pop())

    # valid JSON escape characters (without the leading backslash)
    _VALID_ESCAPES = frozenset('"\\/bfnrt')

    def _emit_escape(self, ch: str) -> None:
        """Emit a backslash escape sequence.

        If ``ch`` is a valid JSON escape character, emit ``\\ch`` as-is.
        Otherwise emit ``\\\\ch`` (escape the backslash itself).
        """
        if ch in self._VALID_ESCAPES:
            self._emit("\\")
            self._emit(ch)
        elif ch == "u" and self._peek(1) in "0123456789abcdefABCDEF":
            # Likely a unicode escape — pass through the backslash and 'u'
            self._emit("\\")
            self._emit("u")
        else:
            # Invalid escape — double-escape the backslash
            self._emit("\\\\")
            self._emit(ch)

    # ── prefix / suffix junk ──────────────────────────────────────────────

    def _skip_prefix_junk(self) -> None:
        """Skip text before the first '{' or '['.

        Also strips markdown code fences (```json ... ```).
        """
        # Strip markdown code-fence opening
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

        # Find the first structural JSON character.
        # Prefer { or [ (object/array — 99 % of LLM JSON output).
        # Fall back to the original start for bare values like "string" or 123.
        saved = self.i
        while self.i < self.n:
            ch = self.text[self.i]
            if ch in "{[":
                break
            if ch == '"':
                # Skip over a quoted string so we don't mistake an
                # internal { or [ for a structural character.
                self.i += 1
                while self.i < self.n:
                    if self.text[self.i] == "\\":
                        self.i += 2  # skip escape sequence
                    elif self.text[self.i] == '"':
                        self.i += 1
                        break
                    else:
                        self.i += 1
            else:
                self.i += 1
        if self.i >= self.n:
            # No object/array found — restore and let _parse_value handle
            # bare strings, numbers, booleans, null
            self.i = saved

    def _skip_suffix_junk(self) -> None:
        """Strip trailing markdown fences and text after JSON."""
        result = "".join(self.out)

        # Strip trailing markdown code fence manually (avoid regex overhead)
        idx = result.rfind("\n```")
        if idx != -1:
            rest = result[idx + 4 :]
            if rest.strip() in ("", "```"):
                result = result[:idx]

        # Find the last structural close bracket at depth 0,
        #  skipping brackets that appear inside quoted strings.
        depth = 0
        last_close = -1
        in_string = False
        esc = False
        for i, ch in enumerate(result):
            if esc:
                esc = False
                continue
            if ch == "\\":
                esc = True
                continue
            if ch == '"':
                in_string = not in_string
                continue
            if in_string:
                continue
            if ch in "{[":
                depth += 1
            elif ch in "}]":
                depth -= 1
                if depth == 0:
                    last_close = i

        if last_close >= 0:
            after = result[last_close + 1 :]
            if after.strip() and not after.startswith("```"):
                result = result[: last_close + 1]

        self.out.clear()
        self.out.append(result)

    # ── value dispatch ────────────────────────────────────────────────────

    def _parse_value(self) -> None:
        """Parse any JSON value at the current position."""
        self._skip_ws()
        if self.i >= self.n:
            return

        ch = self.text[self.i]

        if ch == "{":
            self._parse_object()
        elif ch == "[":
            self._parse_array()
        elif ch == '"':
            # Check for triple-quoted string — only if a matching
            # closing \"\"\" exists later in the text.
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
        elif ch in "-.0123456789":
            self._parse_number()
        elif ch == "/":
            self._skip_comment()
            self._parse_value()  # retry after comment
        elif self._expect_key:
            # Only allow unquoted keys that start with a letter or underscore.
            #  A leading digit, hyphen, or other char signals trailing junk.
            if ch.isalpha() or ch == "_":
                self._parse_unquoted_key()
            else:
                # Trailing junk after a value — stop here so the caller
                #  can close brackets and strip suffix.
                return
        else:
            # Unknown junk — skip char and retry
            self.i += 1
            self._parse_value()

    # ── object ────────────────────────────────────────────────────────────

    def _parse_object(self) -> None:
        """Parse an object: { ... }"""
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

            if ch == "}":
                # Remove trailing comma before closing brace
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                self._emit("}")
                self.brackets.pop()
                self.i += 1
                self._expect_key = prev_expect
                self._just_emitted_value = True
                return

            if ch == ",":
                if not first:
                    self._emit(",")
                self.i += 1
                self._expect_key = True
                continue

            if ch == "/":
                self._skip_comment()
                continue

            # Skip a stray double-quote that appears after a value
            # right before a structural character (common LLM artifact).
            if ch == '"' and self._just_emitted_value:
                j = self.i + 1
                while j < self.n and self.text[j] in " \t\r\n":
                    j += 1
                if j >= self.n or self.text[j] in "},]:":
                    # This " is a stray — skip it
                    self.i += 1
                    continue

            if self._expect_key:
                # Guard: if we have already parsed something and the
                #  next char can't start a valid key, stop.
                if not first and ch not in "\"_/'" and not ch.isalpha():
                    break
                # If this looks like an unquoted key, verify it is
                #  followed by ':' within a short distance.
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
                    if j >= self.n or self.text[j] != ":":
                        # Not a real key — trailing junk
                        break
                # Missing comma before a new key
                if (
                    not first
                    and self._just_emitted_value
                    and self.out
                    and self.out[-1] not in ",{["
                ):
                    self._emit(",")
                self._parse_key()
                self._skip_ws()
                # Expect colon
                if self.i < self.n and self.text[self.i] == ":":
                    self._emit(":")
                    self.i += 1
                elif self.i < self.n and self.text[self.i] != ":":
                    # Missing colon — insert one
                    self._emit(":")
                self._expect_key = False
                self._parse_value()
                self._expect_key = True
                self._just_emitted_value = True
            else:
                # Junk guard: if not first and the next char can't start
                #  a JSON value, stop parsing.
                if not first and ch not in "\"{['tfnTFNnNiIuU-0123456789":
                    break
                # Missing comma check: value followed by another value
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
        """Parse an array: [ ... ]"""
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
                # Remove trailing comma before closing
                if len(self.out) >= 1 and self.out[-1] == ",":
                    self.out.pop()
                self._emit("]")
                self.brackets.pop()
                self.i += 1
                self._just_emitted_value = True
                return

            if ch == ",":
                if not first:
                    self._emit(",")
                self.i += 1
                continue

            if ch == "/":
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
        """Parse an object key, quoting it if necessary."""
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
        """Parse an unquoted key like `key:` and emit a quoted version."""
        self._emit('"')
        while self.i < self.n and self.text[self.i] not in " \t\r\n:{}[],\"'/":
            self._emit(self.text[self.i])
            self.i += 1
        self._emit('"')

    # ── string (double-quoted) ────────────────────────────────────────────

    def _parse_string(self) -> None:
        """Parse a double-quoted string, escaping embedded quotes."""
        self._emit('"')
        self.i += 1  # skip opening quote

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
                # Handle "" (adjacent double-quotes).
                # The first " is ALWAYS embedded (never closing when
                # immediately followed by another ").  The second " is
                # closing if followed by structural chars, otherwise
                # it is CSV-style and also consumed.
                if self._peek(1) == '"':
                    self._emit('\\"')
                    self.i += 1  # past the first "
                    # Determine fate of the second "
                    j = self.i + 1
                    while j < self.n and self.text[j] in " \t\r":
                        j += 1
                    if j < self.n and self.text[j] in ",}:]\n":
                        # Second " is a closing quote — leave for next iteration
                        continue
                    else:
                        # CSV-style — skip the second " as well
                        self.i += 1
                        # Handle """: "" (CSV) + " (embedded quote in content).
                        # The third " is also embedded — emit it escaped.
                        if self.i < self.n and self.text[self.i] == '"':
                            self._emit('\\"')
                            self.i += 1
                        continue

                # Is this a closing quote?
                if self._is_closing_quote():
                    self._emit('"')
                    self.i += 1
                    return
                else:
                    # Embedded quote — escape it
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

            # Other control characters
            if ord(ch) < 0x20:
                self._emit(f"\\u{ord(ch):04x}")
                self.i += 1
                continue

            self._emit(ch)
            self.i += 1

        # Reached end of text — close the string
        if not self.out or self.out[-1] != '"':
            self._emit('"')

    def _is_closing_quote(self) -> bool:
        """Decide whether the `\"` at `self.i` closes the current string.

        Uses a short lookahead (no state change) to examine the next
        non-whitespace character.  The heuristic is tuned for LLM output
        where embedded natural-language quotes are very common.
        """
        j = self.i + 1
        while j < self.n and self.text[j] in " \t\r":
            j += 1

        if j >= self.n:
            # End of input — close the string
            return True

        nc = self.text[j]

        # Structural characters that follow a string value/key
        if nc in ",}]:\n":
            return True

        # If followed by another quote, this one closes (the next starts a new
        # string — common in missing-comma scenarios).  Exception: \"\" inside
        # a string is CSV-style escaping (handled by the caller before calling
        # this method).
        # If another quote follows, treat the current one as closing
        # (the next starts a new string — common in missing-comma scenarios).
        # Any other character → embedded content that needs escaping.
        return nc == '"'

    # ── triple-quoted string ──────────────────────────────────────────────

    def _parse_triple_string(self) -> None:
        """Parse a Python-style triple-quoted string and convert to valid JSON.

        ``\"\"\"content\"\"\"`` is converted to ``\"content\"`` with internal
        quotes and control characters properly escaped.
        """
        self.i += 3  # skip opening """
        self._emit('"')

        while self.i < self.n:
            # Look for closing """
            if self._peek_str(3) == '"""':
                after = self.i + 3
                # If followed immediately by another double-quote, this
                # """ is likely shifted — the real closing is one position
                # later (e.g.  """"…"""" where content ends with ").
                if after < self.n and self.text[after] == '"':
                    # Don't treat this as closing — process the current "
                    # as content and let the next iteration find the real """
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
                # Quotes inside triple-quoted string content need escaping
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

        # End of text without closing """ — close the string
        self._emit('"')

    # ── single-quoted string ──────────────────────────────────────────────

    def _parse_single_quoted_string(self) -> None:
        """Parse a single-quoted string and convert to double-quoted."""
        self._emit('"')
        self.i += 1  # skip opening '

        while self.i < self.n:
            ch = self.text[self.i]

            if ch == "\\":
                # Escape sequence inside single-quoted string.
                if self._peek(1) == "'":
                    # \' → literal apostrophe
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
                # Check if this ' is a closing quote or an apostrophe
                j = self.i + 1
                while j < self.n and self.text[j] in " \t\r":
                    j += 1
                if j >= self.n or self.text[j] in ",}:]\n":
                    # Closing quote
                    self._emit('"')
                    self.i += 1
                    self._just_emitted_value = True
                    return
                else:
                    # Apostrophe inside content — emit as-is
                    # (safe since output is double-quoted)
                    self._emit("'")
                    self.i += 1
                    continue

            # Double quotes inside single-quoted string need escaping
            # since we output a double-quoted string
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

        # End of text — close
        self._emit('"')

    # ── literals & numbers ────────────────────────────────────────────────

    def _parse_literal(self) -> None:
        """Parse true / false / null (case-insensitive, Python/JS-style)."""
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
            # Unknown word — consume and skip it
            while self.i < self.n and self.text[self.i].isalpha():
                self.i += 1
            return

        self._just_emitted_value = True

    def _parse_number(self) -> None:
        """Parse a JSON number."""
        start = self.i
        while self.i < self.n and self.text[self.i] in "-0123456789.eE+":
            self.i += 1
        num_str = self.text[start : self.i]
        # Normalize leading/trailing decimal point (JSON5 → JSON)
        if num_str.startswith("."):
            num_str = "0" + num_str
        elif num_str.startswith("-."):
            num_str = "-0." + num_str[2:]
        elif num_str.startswith("+."):
            num_str = "+0." + num_str[2:]
        if num_str.endswith("."):
            num_str += "0"
        # Validate and emit
        try:
            float(num_str)
        except ValueError:
            # Invalid number — emit as 0
            self._emit("0")
            self._just_emitted_value = True
            return
        self._emit(num_str)
        self._just_emitted_value = True

    # ── comments ──────────────────────────────────────────────────────────

    def _skip_comment(self) -> None:
        """Skip a ``//`` or ``/* */`` comment."""
        if self._peek_str(2) == "//":
            while self.i < self.n and self.text[self.i] != "\n":
                self.i += 1
            if self.i < self.n:
                self.i += 1  # skip newline
        elif self._peek_str(2) == "/*":
            self.i += 2
            while self.i + 1 < self.n:
                if self.text[self.i] == "*" and self.text[self.i + 1] == "/":
                    self.i += 2
                    return
                self.i += 1
        else:
            # Single '/' — skip it
            self.i += 1

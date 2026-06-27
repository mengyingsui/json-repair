# cython: language_level=3
"""
Cython-accelerated string parsing for json_repair.

Compiles the ``_parse_string`` hot loop to C for 5–10× speedup on large
inputs.  Used via conditional import in ``_repair.py`` with a pure-Python
fallback so compilation is never required.
"""


def parse_string(text, i, n, out):
    """C-accelerated string parser.

    Emits the string content (including surrounding quotes) directly to
    *out* and returns ``(new_i, chars_emitted)`` — mirrors
    :meth:`_Repairer._parse_string` exactly.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        str _text = text
        list _out = out
        Py_ssize_t _chars = 0
        Py_ssize_t j, k
        str ch, nc
        bint _closed = False

    # emit opening quote
    _out.append('"')
    _chars += 1
    _i += 1

    while _i < _n:
        ch = _text[_i]

        # ── escape ────────────────────────────────────────────────────────
        if ch == "\\":
            _i += 1
            if _i < _n:
                nc = _text[_i]
                if nc == "u" and (_i + 1 < _n):
                    if _text[_i + 1] in "0123456789abcdefABCDEF":
                        _out.append("\\")
                        _out.append("u")
                        _chars += 2
                    else:
                        _out.append("\\\\")
                        _out.append(nc)
                        _chars += 3
                elif nc in ('"', "\\", "/", "b", "f", "n", "r", "t"):
                    _out.append("\\")
                    _out.append(nc)
                    _chars += 2
                else:
                    _out.append("\\\\")
                    _out.append(nc)
                    _chars += 3
                _i += 1
            else:
                _out.append("\\\\")
                _chars += 2
            continue

        # ── closing / embedded quote ──────────────────────────────────────
        if ch == '"':
            # CSV-style double-quote escape: ""
            if _i + 1 < _n and _text[_i + 1] == '"':
                _out.append('\\"')
                _chars += 2
                _i += 1
                j = _i + 1
                while j < _n and _text[j] in " \t\r":
                    j += 1
                if j < _n and _text[j] in ",}:]\n":
                    continue
                else:
                    _i += 1
                    if _i < _n and _text[_i] == '"':
                        _out.append('\\"')
                        _chars += 2
                        _i += 1
                    continue

            # is this a true closing quote?
            if _is_closing_quote(_text, _i, _n):
                _out.append('"')
                _chars += 1
                _closed = True
                nc = _text[_i + 1] if _i + 1 < _n else ""
                if nc and (nc.isalpha() or nc == "_"):
                    k = _i + 1
                    while k < _n and (_text[k].isalnum() or _text[k] == "_"):
                        k += 1
                    while k < _n and _text[k] in " \t\r\n":
                        k += 1
                    if k < _n and _text[k] == '"':
                        _out.pop()
                        _chars -= 1
                        while _out and _out[-1] in " \t\r\n":
                            _out.pop()
                            _chars -= 1
                        if _out and _out[-1] == ",":
                            _out.pop()
                            _chars -= 1
                        _out.append('"')
                        _chars += 1
                        return _i, _chars
                return _i + 1, _chars
            else:
                _out.append('\\"')
                _chars += 2
                _i += 1
                continue

        # ── control characters ────────────────────────────────────────────
        if ch == "\n":
            _out.append("\\n")
            _chars += 2
            _i += 1
            continue

        if ch == "\r":
            _out.append("\\r")
            _chars += 2
            _i += 1
            continue

        if ch == "\t":
            _out.append("\\t")
            _chars += 2
            _i += 1
            continue

        if ord(ch) < 0x20:
            _out.append(f"\\u{ord(ch):04x}")
            _chars += 6
            _i += 1
            continue

        # ── regular character ─────────────────────────────────────────────
        _out.append(ch)
        _chars += 1
        _i += 1

    if not _closed:
        _out.append('"')
        _chars += 1

    return _i, _chars


cdef bint _is_closing_quote(str _text, Py_ssize_t _i, Py_ssize_t _n) noexcept:
    """Inline helper — mirrors ``_Repairer._is_closing_quote``."""
    cdef:
        Py_ssize_t j, k
        str nc

    j = _i + 1
    while j < _n and _text[j] in " \t\r":
        j += 1

    if j >= _n:
        return True

    nc = _text[j]
    if nc in ",}]:\n":
        return True
    if nc == '"':
        return True
    if nc.isalpha() or nc == "_":
        k = j
        while k < _n and (_text[k].isalnum() or _text[k] == "_"):
            k += 1
        while k < _n and _text[k] in " \t\r\n":
            k += 1
        if k < _n and _text[k] == '"':
            k += 1
        while k < _n and _text[k] in " \t\r\n":
            k += 1
        if k < _n and _text[k] == ":":
            return True

    return False

# cython: language_level=3
"""
Cython-accelerated JSON repair core.

Provides C-level implementations of all hot-path parsing routines:
- ``parse_string`` — double-quoted strings
- ``parse_single_quoted_string`` — single-quoted → double-quoted
- ``parse_triple_string`` — triple-double-quoted strings
- ``fast_parse_value`` — full value/object/array dispatch
- ``fast_parse_object`` — object entry point
- ``fast_parse_array`` — array entry point

String/number/literal parsers are ``cdef`` for C-level speed.
The value/object/array dispatch is ``def`` to avoid Cython 3.2.x forward
declaration limitations with ``list``/``bint`` pointer types; the dispatch
calls are O(values) not O(characters), so the tuple-return overhead is
negligible compared to the C-level inner loops.
"""

# ── Constants ──────────────────────────────────────────────────────────────

_VALID_ESCAPES = frozenset('"\\/bfnrt')


# ── cdef helpers (pointer-based, no circular deps) ─────────────────────────

cdef inline void _emit(list out, str s, Py_ssize_t *out_chars) noexcept:
    out.append(s)
    out_chars[0] += len(s)


cdef void _emit_escape(str text, Py_ssize_t *i, Py_ssize_t n, list out,
                        Py_ssize_t *out_chars) noexcept:
    cdef str ch = text[i[0]] if i[0] < n else ""
    cdef str nc
    if ch in _VALID_ESCAPES:
        _emit(out, "\\", out_chars)
        _emit(out, ch, out_chars)
    elif ch == "u" and i[0] + 1 < n:
        nc = text[i[0] + 1]
        if nc in "0123456789abcdefABCDEF":
            _emit(out, "\\", out_chars)
            _emit(out, "u", out_chars)
        else:
            _emit(out, "\\\\", out_chars)
            _emit(out, nc, out_chars)
    else:
        _emit(out, "\\\\", out_chars)
        _emit(out, ch, out_chars)


cdef inline void _skip_ws(str text, Py_ssize_t *i, Py_ssize_t n) noexcept:
    while i[0] < n and text[i[0]] in " \t\r\n":
        i[0] += 1


cdef void _skip_comment(str text, Py_ssize_t *i, Py_ssize_t n) noexcept:
    cdef Py_ssize_t _i = i[0]
    if _i + 1 < n and text[_i] == "/" and text[_i + 1] == "/":
        while _i < n and text[_i] != "\n":
            _i += 1
        if _i < n:
            _i += 1
    elif _i + 1 < n and text[_i] == "/" and text[_i + 1] == "*":
        _i += 2
        while _i + 1 < n:
            if text[_i] == "*" and text[_i + 1] == "/":
                _i += 2
                i[0] = _i
                return
            _i += 1
    elif _i < n and text[_i] == "#":
        while _i < n and text[_i] != "\n":
            _i += 1
        if _i < n:
            _i += 1
    elif _i + 1 < n and text[_i] == "-" and text[_i + 1] == "-":
        while _i < n and text[_i] != "\n":
            _i += 1
        if _i < n:
            _i += 1
    else:
        _i += 1
    i[0] = _i


# ── closing-quote detection ────────────────────────────────────────────────

cdef bint _is_closing_quote(str _text, Py_ssize_t _i, Py_ssize_t _n) noexcept:
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


cdef bint _is_closing_single_quote(str _text, Py_ssize_t _i,
                                    Py_ssize_t _n) noexcept:
    cdef:
        Py_ssize_t j
        str nc
    j = _i + 1
    while j < _n and _text[j] in " \t\r":
        j += 1
    if j >= _n or _text[j] in ",}:]\n":
        return True
    return False


# ── string parsers (cdef, internal) ────────────────────────────────────────

cdef void _cparse_string(str _text, Py_ssize_t *i, Py_ssize_t _n,
                          list _out, Py_ssize_t *out_chars) noexcept:
    cdef:
        Py_ssize_t _i = i[0]
        Py_ssize_t j, k
        str ch, nc
        bint _closed = False

    _emit(_out, '"', out_chars)
    _i += 1

    while _i < _n:
        ch = _text[_i]

        if ch == "\\":
            _i += 1
            if _i < _n:
                _emit(_out, "\\", out_chars)
                nc = _text[_i]
                if nc == "u" and (_i + 1 < _n
                                  and _text[_i + 1] in "0123456789abcdefABCDEF"):
                    _emit(_out, "u", out_chars)
                elif nc in ('"', "\\", "/", "b", "f", "n", "r", "t"):
                    _emit(_out, nc, out_chars)
                else:
                    out_chars[0] -= 1
                    _out.pop()
                    _emit(_out, "\\\\", out_chars)
                    _emit(_out, nc, out_chars)
                _i += 1
            else:
                _emit(_out, "\\\\", out_chars)
            continue

        if ch == '"':
            if _i + 1 < _n and _text[_i + 1] == '"':
                _emit(_out, '\\"', out_chars)
                _i += 1
                j = _i + 1
                while j < _n and _text[j] in " \t\r":
                    j += 1
                if j < _n and _text[j] in ",}:]\n":
                    continue
                else:
                    _i += 1
                    if _i < _n and _text[_i] == '"':
                        _emit(_out, '\\"', out_chars)
                        _i += 1
                    continue

            if _is_closing_quote(_text, _i, _n):
                _emit(_out, '"', out_chars)
                _closed = True
                nc = _text[_i + 1] if _i + 1 < _n else ""
                if nc and (nc.isalpha() or nc == "_"):
                    k = _i + 1
                    while k < _n and (_text[k].isalnum()
                                      or _text[k] == "_"):
                        k += 1
                    while k < _n and _text[k] in " \t\r\n":
                        k += 1
                    if k < _n and _text[k] == '"':
                        _out.pop()
                        out_chars[0] -= 1
                        while _out and _out[len(_out) - 1] in " \t\r\n":
                            _out.pop()
                            out_chars[0] -= 1
                        if _out and _out[len(_out) - 1] == ",":
                            _out.pop()
                            out_chars[0] -= 1
                        _emit(_out, '"', out_chars)
                        i[0] = _i
                        return
                i[0] = _i + 1
                return
            else:
                _emit(_out, '\\"', out_chars)
                _i += 1
                continue

        if ch == "\n":
            _emit(_out, "\\n", out_chars)
            _i += 1
            continue
        if ch == "\r":
            _emit(_out, "\\r", out_chars)
            _i += 1
            continue
        if ch == "\t":
            _emit(_out, "\\t", out_chars)
            _i += 1
            continue
        if ord(ch) < 0x20:
            _emit(_out, f"\\u{ord(ch):04x}", out_chars)
            _i += 1
            continue

        _emit(_out, ch, out_chars)
        _i += 1

    if not _closed:
        _emit(_out, '"', out_chars)
    i[0] = _i


cdef void _cparse_single_quoted_string(str _text, Py_ssize_t *i,
                                        Py_ssize_t _n, list _out,
                                        Py_ssize_t *out_chars) noexcept:
    cdef:
        Py_ssize_t _i = i[0]
        Py_ssize_t j
        str ch

    _emit(_out, '"', out_chars)
    _i += 1

    while _i < _n:
        ch = _text[_i]

        if ch == "\\":
            if _i + 1 < _n and _text[_i + 1] == "'":
                _emit(_out, "'", out_chars)
                _i += 2
                continue
            _i += 1
            if _i < _n:
                _emit_escape(_text, &_i, _n, _out, out_chars)
                _i += 1
            else:
                _emit(_out, "\\\\", out_chars)
            continue

        if ch == "'":
            j = _i + 1
            while j < _n and _text[j] in " \t\r":
                j += 1
            if j >= _n or _text[j] in ",}:]\n":
                _emit(_out, '"', out_chars)
                _i += 1
                i[0] = _i
                return
            else:
                _emit(_out, "'", out_chars)
                _i += 1
                continue

        if ch == '"':
            _emit(_out, '\\"', out_chars)
            _i += 1
            continue

        if ch == "\n":
            _emit(_out, "\\n", out_chars)
            _i += 1
            continue
        if ch == "\r":
            _emit(_out, "\\r", out_chars)
            _i += 1
            continue
        if ch == "\t":
            _emit(_out, "\\t", out_chars)
            _i += 1
            continue
        if ord(ch) < 0x20:
            _emit(_out, f"\\u{ord(ch):04x}", out_chars)
            _i += 1
            continue

        _emit(_out, ch, out_chars)
        _i += 1

    _emit(_out, '"', out_chars)
    i[0] = _i


cdef void _cparse_triple_string(str _text, Py_ssize_t *i, Py_ssize_t _n,
                                 list _out, Py_ssize_t *out_chars) noexcept:
    cdef:
        Py_ssize_t _i = i[0]
        str ch

    _i += 3
    _emit(_out, '"', out_chars)

    while _i < _n:
        if _i + 2 < _n and _text[_i] == '"' and _text[_i + 1] == '"' \
                and _text[_i + 2] == '"':
            after = _i + 3
            if after < _n and _text[after] == '"':
                pass
            else:
                _i += 3
                _emit(_out, '"', out_chars)
                i[0] = _i
                return

        ch = _text[_i]

        if ch == "\\":
            _i += 1
            if _i < _n:
                _emit_escape(_text, &_i, _n, _out, out_chars)
                _i += 1
            else:
                _emit(_out, "\\\\", out_chars)
            continue

        if ch == '"':
            _emit(_out, '\\"', out_chars)
            _i += 1
            continue

        if ch == "\n":
            _emit(_out, "\\n", out_chars)
            _i += 1
            continue
        if ch == "\r":
            _emit(_out, "\\r", out_chars)
            _i += 1
            continue
        if ch == "\t":
            _emit(_out, "\\t", out_chars)
            _i += 1
            continue
        if ord(ch) < 0x20:
            _emit(_out, f"\\u{ord(ch):04x}", out_chars)
            _i += 1
            continue

        _emit(_out, ch, out_chars)
        _i += 1

    _emit(_out, '"', out_chars)
    i[0] = _i


# ── number / literal / unquoted-value parsers (cdef) ───────────────────────

cdef void _cparse_number(str _text, Py_ssize_t *i, Py_ssize_t _n,
                          list _out, Py_ssize_t *out_chars) noexcept:
    cdef:
        Py_ssize_t _i = i[0]
        Py_ssize_t start = _i
        str num_str

    while _i < _n and _text[_i] in "-0123456789.eE+":
        _i += 1
    num_str = _text[start:_i]
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
    except Exception:
        _emit(_out, "0", out_chars)
        i[0] = _i
        return
    _emit(_out, num_str, out_chars)
    i[0] = _i


cdef void _cparse_literal(str _text, Py_ssize_t *i, Py_ssize_t _n,
                           list _out, Py_ssize_t *out_chars) noexcept:
    cdef str lower = _text[i[0]:i[0] + 9].lower()
    if lower.startswith("true"):
        _emit(_out, "true", out_chars)
        i[0] += 4
    elif lower.startswith("false"):
        _emit(_out, "false", out_chars)
        i[0] += 5
    elif lower.startswith("null") or lower.startswith("none"):
        _emit(_out, "null", out_chars)
        i[0] += 4
    elif lower.startswith("undefined"):
        _emit(_out, "null", out_chars)
        i[0] += 9
    elif lower.startswith("nan"):
        _emit(_out, "null", out_chars)
        i[0] += 3
    elif lower.startswith("infinity") or lower.startswith("+infinity"):
        _emit(_out, "null", out_chars)
        i[0] += 8
    elif lower.startswith("-infinity"):
        _emit(_out, "null", out_chars)
        i[0] += 9
    else:
        _cparse_unquoted_value(_text, i, _n, _out, out_chars)


cdef void _cparse_unquoted_key(str _text, Py_ssize_t *i, Py_ssize_t _n,
                                list _out, Py_ssize_t *out_chars) noexcept:
    cdef Py_ssize_t _i = i[0]
    _emit(_out, '"', out_chars)
    while _i < _n and _text[_i] not in " \t\r\n:{}[],\"'/":
        _emit(_out, _text[_i], out_chars)
        _i += 1
    _emit(_out, '"', out_chars)
    if _i < _n and _text[_i] == '"':
        _i += 1
    i[0] = _i


cdef void _cparse_unquoted_value(str _text, Py_ssize_t *i, Py_ssize_t _n,
                                  list _out, Py_ssize_t *out_chars) noexcept:
    cdef:
        Py_ssize_t _i = i[0]
        str ch
    _emit(_out, '"', out_chars)
    while _i < _n and _text[_i] not in ",}]":
        ch = _text[_i]
        if ch == "\\":
            _emit(_out, "\\\\", out_chars)
        elif ch == '"':
            _emit(_out, '\\"', out_chars)
        elif ord(ch) < 0x20:
            _emit(_out, f"\\u{ord(ch):04x}", out_chars)
        else:
            _emit(_out, ch, out_chars)
        _i += 1
    _emit(_out, '"', out_chars)
    i[0] = _i


# ── def dispatch functions (value-based, no forward-declaration issues) ────

def _cparse_dispatch_value(str _text, Py_ssize_t _i, Py_ssize_t _n,
                            list _out, list _brackets,
                            bint _expect_key_val, bint _just_emitted_val,
                            Py_ssize_t _out_chars_val,
                            Py_ssize_t _last_depth0_pos_val):
    """Value dispatcher. Handles { → object, [ → array, strings, numbers, etc.

    Returns (i, expect_key, just_emitted, out_chars, last_depth0_pos).
    """
    cdef:
        Py_ssize_t __i = _i
        bint _expect_key = _expect_key_val
        bint _just_emitted = _just_emitted_val
        Py_ssize_t _out_chars = _out_chars_val
        Py_ssize_t _last_depth0_pos = _last_depth0_pos_val
        str ch

    _skip_ws(_text, &__i, _n)
    if __i >= _n:
        _emit(_out, "null", &_out_chars)
        _just_emitted = True
        return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos

    ch = _text[__i]

    if ch == "{":
        return _cparse_dispatch_object(_text, __i, _n, _out, _brackets,
                                        _expect_key, _just_emitted,
                                        _out_chars, _last_depth0_pos)
    elif ch == "[":
        return _cparse_dispatch_array(_text, __i, _n, _out, _brackets,
                                       _expect_key, _just_emitted,
                                       _out_chars, _last_depth0_pos)
    elif ch == '"':
        if __i + 2 < _n and _text[__i:__i + 3] == '"""':
            if '"""' in _text[__i + 3:]:
                _cparse_triple_string(_text, &__i, _n, _out, &_out_chars)
                _just_emitted = True
            else:
                _cparse_string(_text, &__i, _n, _out, &_out_chars)
                _just_emitted = True
        else:
            _cparse_string(_text, &__i, _n, _out, &_out_chars)
            _just_emitted = True
    elif ch == "'":
        _cparse_single_quoted_string(_text, &__i, _n, _out, &_out_chars)
        _just_emitted = True
    elif ch in "tfnTFNnNiIuU":
        _cparse_literal(_text, &__i, _n, _out, &_out_chars)
        _just_emitted = True
    elif ch == "-" and __i + 1 < _n and _text[__i + 1] == "-":
        _skip_comment(_text, &__i, _n)
        return _cparse_dispatch_value(_text, __i, _n, _out, _brackets,
                                       _expect_key, _just_emitted,
                                       _out_chars, _last_depth0_pos)
    elif ch in "-.0123456789":
        _cparse_number(_text, &__i, _n, _out, &_out_chars)
        _just_emitted = True
    elif ch in "/#":
        _skip_comment(_text, &__i, _n)
        return _cparse_dispatch_value(_text, __i, _n, _out, _brackets,
                                       _expect_key, _just_emitted,
                                       _out_chars, _last_depth0_pos)
    elif ch in "}]" or ch == ",":
        _emit(_out, "null", &_out_chars)
        _just_emitted = True
    elif _expect_key and (ch.isalpha() or ch == "_"):
        _cparse_unquoted_key(_text, &__i, _n, _out, &_out_chars)
        _just_emitted = True
    elif ch.isalpha() or ch == "_":
        _cparse_unquoted_value(_text, &__i, _n, _out, &_out_chars)
        _just_emitted = True
    else:
        __i += 1
        return _cparse_dispatch_value(_text, __i, _n, _out, _brackets,
                                       _expect_key, _just_emitted,
                                       _out_chars, _last_depth0_pos)

    return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos


def _cparse_dispatch_object(str _text, Py_ssize_t _i, Py_ssize_t _n,
                             list _out, list _brackets,
                             bint _expect_key_val, bint _just_emitted_val,
                             Py_ssize_t _out_chars_val,
                             Py_ssize_t _last_depth0_pos_val):
    """Object parser. Emits {...} to out, returns updated state tuple."""
    cdef:
        Py_ssize_t __i = _i
        bint _expect_key = _expect_key_val
        bint _just_emitted = _just_emitted_val
        Py_ssize_t _out_chars = _out_chars_val
        Py_ssize_t _last_depth0_pos = _last_depth0_pos_val
        bint first = True
        bint prev_expect = _expect_key
        Py_ssize_t j, k
        str ch

    _emit(_out, "{", &_out_chars)
    _brackets.append("}")
    __i += 1
    _expect_key = True

    while __i < _n:
        _skip_ws(_text, &__i, _n)
        if __i >= _n:
            break

        ch = _text[__i]

        if ch == "{" and _expect_key:
            __i += 1
            continue
        if ch == ":" and _expect_key:
            __i += 1
            continue

        if ch == "}":
            if len(_out) >= 1 and _out[len(_out) - 1] == ",":
                _out.pop()
                _out_chars -= 1
            _emit(_out, "}", &_out_chars)
            _brackets.pop()
            if not _brackets:
                _last_depth0_pos = _out_chars
            __i += 1
            _expect_key = prev_expect
            _just_emitted = True
            return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos

        if ch == ",":
            if not first and (not _out or _out[len(_out) - 1] != ","):
                _emit(_out, ",", &_out_chars)
            __i += 1
            _expect_key = True
            continue

        if ch in "/#" or (ch == "-" and __i + 1 < _n
                          and _text[__i + 1] == "-"):
            _skip_comment(_text, &__i, _n)
            continue

        if ch == '"' and _just_emitted:
            j = __i + 1
            while j < _n and _text[j] in " \t\r\n":
                j += 1
            if j >= _n or _text[j] in "},]:":
                __i += 1
                continue

        if ch == "]":
            if len(_out) >= 1 and _out[len(_out) - 1] == ",":
                _out.pop()
                _out_chars -= 1
            _emit(_out, "}", &_out_chars)
            _brackets.pop()
            if not _brackets:
                _last_depth0_pos = _out_chars
            _expect_key = prev_expect
            return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos

        if _expect_key:
            if not first and ch not in "\"_/'" and not ch.isalpha():
                break
            if ch.isalpha():
                j = __i + 1
                while j < _n and (_text[j].isalnum() or _text[j] == "_"):
                    j += 1
                while j < _n and _text[j] in " \t\r":
                    j += 1
                if j >= _n or _text[j] not in ':",':
                    break
            if (not first and _just_emitted and _out
                    and _out[len(_out) - 1] not in ",{["):
                _emit(_out, ",", &_out_chars)

            if ch == '"':
                _cparse_string(_text, &__i, _n, _out, &_out_chars)
            elif ch == "'":
                _cparse_single_quoted_string(_text, &__i, _n, _out,
                                              &_out_chars)
            else:
                _cparse_unquoted_key(_text, &__i, _n, _out, &_out_chars)
            _skip_ws(_text, &__i, _n)
            if __i < _n and _text[__i] == ":":
                _emit(_out, ":", &_out_chars)
                __i += 1
            elif __i < _n and _text[__i] != ":":
                _emit(_out, ":", &_out_chars)
            _expect_key = False
            (__i, _expect_key, _just_emitted,
             _out_chars, _last_depth0_pos) = _cparse_dispatch_value(
                _text, __i, _n, _out, _brackets,
                _expect_key, _just_emitted,
                _out_chars, _last_depth0_pos)
            _expect_key = True
            _just_emitted = True
        else:
            if not first and ch not in "\"{['tfnTFNnIiUu-0123456789":
                break
            if (not first and _just_emitted and _out
                    and _out[len(_out) - 1] not in ",{["
                    and ch not in "}],"):
                _emit(_out, ",", &_out_chars)
            (__i, _expect_key, _just_emitted,
             _out_chars, _last_depth0_pos) = _cparse_dispatch_value(
                _text, __i, _n, _out, _brackets,
                _expect_key, _just_emitted,
                _out_chars, _last_depth0_pos)
            _just_emitted = True

        first = False

    _expect_key = prev_expect
    return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos


def _cparse_dispatch_array(str _text, Py_ssize_t _i, Py_ssize_t _n,
                            list _out, list _brackets,
                            bint _expect_key_val, bint _just_emitted_val,
                            Py_ssize_t _out_chars_val,
                            Py_ssize_t _last_depth0_pos_val):
    """Array parser. Emits [...] to out, returns updated state tuple."""
    cdef:
        Py_ssize_t __i = _i
        bint _expect_key = _expect_key_val
        bint _just_emitted = _just_emitted_val
        Py_ssize_t _out_chars = _out_chars_val
        Py_ssize_t _last_depth0_pos = _last_depth0_pos_val
        bint first = True
        str ch

    _emit(_out, "[", &_out_chars)
    _brackets.append("]")
    __i += 1

    while __i < _n:
        _skip_ws(_text, &__i, _n)
        if __i >= _n:
            break

        ch = _text[__i]

        if ch == "]":
            if len(_out) >= 1 and _out[len(_out) - 1] == ",":
                _out.pop()
                _out_chars -= 1
            _emit(_out, "]", &_out_chars)
            _brackets.pop()
            if not _brackets:
                _last_depth0_pos = _out_chars
            __i += 1
            _just_emitted = True
            return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos

        if ch == "}":
            if len(_out) >= 1 and _out[len(_out) - 1] == ",":
                _out.pop()
                _out_chars -= 1
            _emit(_out, "]", &_out_chars)
            _brackets.pop()
            if not _brackets:
                _last_depth0_pos = _out_chars
            __i += 1
            _just_emitted = True
            return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos

        if ch == ",":
            if not first and (not _out or _out[len(_out) - 1] != ","):
                _emit(_out, ",", &_out_chars)
            __i += 1
            continue

        if ch in "/#" or (ch == "-" and __i + 1 < _n
                          and _text[__i + 1] == "-"):
            _skip_comment(_text, &__i, _n)
            continue

        if (not first and _just_emitted and _out
                and _out[len(_out) - 1] not in ",[:"
                and ch not in "]"):
            _emit(_out, ",", &_out_chars)

        (__i, _expect_key, _just_emitted,
         _out_chars, _last_depth0_pos) = _cparse_dispatch_value(
            _text, __i, _n, _out, _brackets,
            _expect_key, _just_emitted,
            _out_chars, _last_depth0_pos)
        _just_emitted = True
        first = False

    return __i, _expect_key, _just_emitted, _out_chars, _last_depth0_pos


# ── def entry points (Python-callable) ─────────────────────────────────────


def parse_string(text, i, n, out):
    """C-accelerated double-quoted string parser.

    Emits string content (including surrounding quotes) directly to *out*
    and returns ``(new_i, chars_emitted)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _chars = 0
        list _out = out
        str _text = text

    _cparse_string(_text, &_i, _n, _out, &_chars)
    return _i, _chars


def parse_single_quoted_string(text, i, n, out):
    """C-accelerated single-quoted → double-quoted string parser.

    Emits double-quoted content to *out* and returns ``(new_i, chars_emitted)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _chars = 0
        list _out = out
        str _text = text

    _cparse_single_quoted_string(_text, &_i, _n, _out, &_chars)
    return _i, _chars


def parse_triple_string(text, i, n, out):
    """C-accelerated triple-double-quoted string parser.

    Emits double-quoted content to *out* and returns ``(new_i, chars_emitted)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _chars = 0
        list _out = out
        str _text = text

    _cparse_triple_string(_text, &_i, _n, _out, &_chars)
    return _i, _chars


def fast_parse_value(text, i, n, out, brackets,
                      expect_key, just_emitted,
                      out_chars, last_depth0_pos):
    """C-accelerated JSON value dispatcher.

    Handles the full value/object/array dispatch chain.
    Returns ``(i, expect_key, just_emitted, out_chars, last_depth0_pos)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _out_chars = out_chars
        Py_ssize_t _last_depth0_pos = last_depth0_pos
        bint _expect_key = expect_key
        bint _just_emitted = just_emitted
        list _out = out
        list _brackets = brackets
        str _text = text
        tuple _res

    _res = _cparse_dispatch_value(_text, _i, _n, _out, _brackets,
                                   _expect_key, _just_emitted,
                                   _out_chars, _last_depth0_pos)
    return _res


def fast_parse_object(text, i, n, out, brackets,
                       expect_key, just_emitted,
                       out_chars, last_depth0_pos):
    """C-accelerated JSON object parser entry point.

    Returns ``(i, expect_key, just_emitted, out_chars, last_depth0_pos)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _out_chars = out_chars
        Py_ssize_t _last_depth0_pos = last_depth0_pos
        bint _expect_key = expect_key
        bint _just_emitted = just_emitted
        list _out = out
        list _brackets = brackets
        str _text = text
        tuple _res

    _res = _cparse_dispatch_object(_text, _i, _n, _out, _brackets,
                                    _expect_key, _just_emitted,
                                    _out_chars, _last_depth0_pos)
    return _res


def fast_parse_array(text, i, n, out, brackets,
                      expect_key, just_emitted,
                      out_chars, last_depth0_pos):
    """C-accelerated JSON array parser entry point.

    Returns ``(i, expect_key, just_emitted, out_chars, last_depth0_pos)``.
    """
    cdef:
        Py_ssize_t _i = i
        Py_ssize_t _n = n
        Py_ssize_t _out_chars = out_chars
        Py_ssize_t _last_depth0_pos = last_depth0_pos
        bint _expect_key = expect_key
        bint _just_emitted = just_emitted
        list _out = out
        list _brackets = brackets
        str _text = text
        tuple _res

    _res = _cparse_dispatch_array(_text, _i, _n, _out, _brackets,
                                   _expect_key, _just_emitted,
                                   _out_chars, _last_depth0_pos)
    return _res

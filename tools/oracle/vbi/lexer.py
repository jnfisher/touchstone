"""Lexer for the VB6 subset: physical lines -> token stream with NEWLINE terminators.

Handles line continuations, `'`/Rem comments, strings with doubled quotes, numeric literals
(&H, &O, type suffixes, exponents), type-suffixed identifiers (`Left$`), the `!` bang operator
and conditional compilation (#Const/#If/#ElseIf/#Else/#End If).
"""
import re

from .errors import VBSyntaxError


class Tok:
    __slots__ = ("kind", "text", "low", "line", "val", "tc", "space_before")

    def __init__(self, kind, text, line, val=None, tc=None, space_before=False):
        self.kind = kind          # id num str op nl date
        self.text = text
        self.low = text.lower() if kind in ("id", "op") else text
        self.line = line
        self.val = val
        self.tc = tc              # numeric literal type code or identifier type suffix
        self.space_before = space_before

    def __repr__(self):
        return "Tok(%s,%r,L%d)" % (self.kind, self.text, self.line)


SUFFIX_TC = {"%": "integer", "&": "long", "!": "single", "#": "double", "@": "currency", "$": "string"}

_ID_START = re.compile(r"[A-Za-zÀ-ɏ]")
_ID_CHARS = re.compile(r"[A-Za-z0-9_À-ɏ]*")
_NUM = re.compile(r"(\d+\.?\d*|\.\d+)([eEdD][+-]?\d+)?")
_HEX = re.compile(r"&[hH]([0-9A-Fa-f]+)")
_OCT = re.compile(r"&[oO]([0-7]+)")
_DATE = re.compile(r"#([^#]*)#")


def tokenize_line(s, lineno, toks):
    """Tokenize one physical line into toks. Returns True if it ends in a continuation."""
    i, n = 0, len(s)
    sp = True
    while i < n:
        c = s[i]
        if c in " \t\f\xa0":
            i += 1
            sp = True
            continue
        if c == "'":
            return 2 if re.search(r"[ \t]_[ \t]*$", s[i:]) else False   # comment (may continue)
        if c == '"':
            j = i + 1
            buf = []
            while True:
                if j >= n:
                    raise VBSyntaxError("unterminated string", lineno)
                if s[j] == '"':
                    if j + 1 < n and s[j + 1] == '"':
                        buf.append('"')
                        j += 2
                        continue
                    break
                buf.append(s[j])
                j += 1
            toks.append(Tok("str", s[i:j + 1], lineno, "".join(buf), space_before=sp))
            i = j + 1
            sp = False
            continue
        if c == "_" and sp:
            rest = s[i + 1:].lstrip(" \t")
            if rest == "" or rest.startswith("'"):
                return True                                # line continuation
        if _ID_START.match(c) or c == "[":
            if c == "[":
                j = s.index("]", i)
                text = s[i + 1:j]
                j += 1
            else:
                m = _ID_CHARS.match(s, i)
                j = m.end()
                text = s[i:j]
            if text.lower() == "rem" and c != "[" and (j >= n or s[j] in " \t"):
                return 2 if re.search(r"[ \t]_[ \t]*$", s[j:]) else False
            tc = None
            if j < n and c != "[":
                sc = s[j]
                nxt = s[j + 1] if j + 1 < n else ""
                if sc in "$%#@":
                    tc = SUFFIX_TC[sc]
                    j += 1
                elif sc == "&" and nxt in ("", ")", ","):
                    tc = "long"
                    j += 1
                elif sc == "!" and not (nxt.isalpha() or nxt == "_" or nxt == "["):
                    tc = "single"
                    j += 1
            toks.append(Tok("id", text, lineno, tc=tc, space_before=sp))
            i = j
            sp = False
            continue
        if c.isdigit() or (c == "." and i + 1 < n and s[i + 1].isdigit()):
            m = _NUM.match(s, i)
            txt = m.group(0)
            j = m.end()
            isfloat = ("." in txt) or m.group(2) is not None
            val = float(txt.replace("d", "e").replace("D", "e")) if isfloat else int(txt)
            tc = None
            if j < n and s[j] in "%&!#@":
                tc = SUFFIX_TC[s[j]]
                j += 1
            elif isfloat:
                tc = "double"
            toks.append(Tok("num", txt, lineno, val=val, tc=tc, space_before=sp))
            i = j
            sp = False
            continue
        if c == "&" and i + 1 < n and s[i + 1] in "hHoO":
            m = _HEX.match(s, i) or _OCT.match(s, i)
            if m:
                digits = m.group(1)
                radix = 16 if s[i + 1] in "hH" else 8
                v = int(digits, radix)
                j = m.end()
                if j < n and s[j] == "&":
                    j += 1
                    tc = "long"
                elif j < n and s[j] == "%":
                    j += 1
                    tc = "integer"
                else:
                    limit = 4 if radix == 16 else 6
                    tc = "integer" if len(digits.lstrip("0") or "0") <= limit else "long"
                # hex literals are the two's complement of their width
                if tc == "integer" and v > 0x7FFF:
                    v -= 0x10000
                elif tc == "long" and v > 0x7FFFFFFF:
                    v -= 0x100000000
                toks.append(Tok("num", s[i:j], lineno, val=v, tc=tc, space_before=sp))
                i = j
                sp = False
                continue
        if c == "#" and sp:
            m = _DATE.match(s, i)
            if m and re.search(r"\d", m.group(1)):
                toks.append(Tok("date", m.group(0), lineno, val=m.group(1), space_before=sp))
                i = m.end()
                sp = False
                continue
        two = s[i:i + 2]
        if two in ("<=", ">=", "<>", ":="):
            toks.append(Tok("op", two, lineno, space_before=sp))
            i += 2
            sp = False
            continue
        if two == "=<":
            toks.append(Tok("op", "<=", lineno, space_before=sp))
            i += 2
            sp = False
            continue
        if two == "=>":
            toks.append(Tok("op", ">=", lineno, space_before=sp))
            i += 2
            sp = False
            continue
        if c in "+-*/\\^&=<>(),.:;!#":
            toks.append(Tok("op", c, lineno, space_before=sp))
            i += 1
            sp = False
            continue
        raise VBSyntaxError("unexpected character %r" % c, lineno)
    return False


# ---------------------------------------------------------------- conditional compilation

def _eval_cc(tokens, consts, lineno):
    pos = [0]

    def peek():
        return tokens[pos[0]] if pos[0] < len(tokens) else None

    def take():
        t = tokens[pos[0]]
        pos[0] += 1
        return t

    def prim():
        t = take()
        if t.kind == "num":
            return t.val
        if t.kind == "id":
            if t.low == "not":
                return ~int(prim())
            if t.low == "true":
                return -1
            if t.low == "false":
                return 0
            return consts.get(t.low, 0)
        if t.kind == "op" and t.low == "(":
            v = orx()
            take()
            return v
        if t.kind == "op" and t.low == "-":
            return -prim()
        raise VBSyntaxError("bad conditional compilation expression", lineno)

    def cmp_():
        v = prim()
        while peek() is not None and peek().kind == "op" and peek().low in ("=", "<>", "<", ">", "<=", ">="):
            op = take().low
            w = prim()
            v = -1 if {"=": v == w, "<>": v != w, "<": v < w, ">": v > w, "<=": v <= w, ">=": v >= w}[op] else 0
        return v

    def andx():
        v = cmp_()
        while peek() is not None and peek().kind == "id" and peek().low == "and":
            take()
            v = int(v) & int(cmp_())
        return v

    def orx():
        v = andx()
        while peek() is not None and peek().kind == "id" and peek().low in ("or", "xor"):
            op = take().low
            w = int(andx())
            v = (int(v) | w) if op == "or" else (int(v) ^ w)
        return v

    return orx()


_DIRECTIVE = re.compile(r"#\s*(const|if|elseif|else|end\s+if)\b(.*)", re.I)


def lex(text, module=None):
    """Tokenize a whole module (already decoded). Returns a list of Tok ending with a NEWLINE."""
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    toks = []
    consts = {}
    stack = []          # [taking, taken_any, parent_active]
    active = True
    pending_cont = False
    try:
        skipping = False
        for idx, raw in enumerate(lines):
            lineno = idx + 1
            stripped = raw.strip()
            if skipping:
                if not re.search(r"[ \t]_[ \t]*$", raw):
                    skipping = False
                    pending_cont = False
                    toks.append(Tok("nl", "\n", lineno))
                continue
            m = None if pending_cont else _DIRECTIVE.match(stripped)
            if m:
                kw = re.sub(r"\s+", " ", m.group(1).lower())
                body = re.sub(r"\bthen\b\s*$", "", m.group(2).strip(), flags=re.I)
                ct = []
                if kw in ("const", "if", "elseif"):
                    tokenize_line(body, lineno, ct)
                if kw == "const":
                    if active and len(ct) >= 3 and ct[1].low == "=":
                        consts[ct[0].low] = _eval_cc(ct[2:], consts, lineno)
                elif kw == "if":
                    cond = bool(_eval_cc(ct, consts, lineno)) if active else False
                    stack.append([cond, cond, active])
                    active = active and cond
                elif kw == "elseif":
                    top = stack[-1]
                    if top[1] or not top[2]:
                        top[0] = False
                    else:
                        top[0] = bool(_eval_cc(ct, consts, lineno))
                        top[1] = top[0]
                    active = top[2] and top[0]
                elif kw == "else":
                    top = stack[-1]
                    top[0] = (not top[1]) and top[2]
                    top[1] = True
                    active = top[0]
                else:                                      # end if
                    top = stack.pop()
                    active = top[2]
                continue
            if not active:
                continue
            cont = tokenize_line(raw, lineno, toks)
            if cont == 2:
                skipping = True
                continue
            if cont:
                pending_cont = True
                continue
            pending_cont = False
            toks.append(Tok("nl", "\n", lineno))
    except VBSyntaxError as e:
        e.module = module
        raise
    toks.append(Tok("nl", "\n", len(lines)))
    return toks

"""Built-in VB6 functions and constants. Each function takes (interp, *args) of VB values."""
import math
import time

from .errors import VBError, OracleUnbound
from . import values as V
from .values import I16, I32, U8, Cur, F32, EMPTY, NULL, NOTHING, MISSING
from .objects import VBArray, UDTInstance, ClsInstance, ExtObject

CONSTANTS = {
    "vbcrlf": "\r\n", "vbcr": "\r", "vblf": "\n", "vbnewline": "\r\n", "vbtab": "\t", "vbnullstring": "",
    "vbnullchar": "\x00", "vbback": "\b", "vbformfeed": "\f", "vbverticaltab": "\v",
    "vbbinarycompare": I32(0), "vbtextcompare": I32(1), "vbdatabasecompare": I32(2),
    "vbtrue": I32(-1), "vbfalse": I32(0), "vbusedefault": I32(-2),
    "vbokonly": I32(0), "vbokcancel": I32(1), "vbyesno": I32(4), "vbyesnocancel": I32(3),
    "vbcritical": I32(16), "vbquestion": I32(32), "vbexclamation": I32(48), "vbinformation": I32(64),
    "vbyes": I32(6), "vbno": I32(7), "vbok": I32(1), "vbcancel": I32(2), "vbretry": I32(4),
    "vbabort": I32(3), "vbignore": I32(5), "vbdefaultbutton2": I32(256), "vbdefaultbutton1": I32(0), "vbdefaultbutton3": I32(512),
    "vbnone": I32(0), "vbtwips": I32(1), "vbpixels": I32(3), "vbnormalfocus": I32(1),
    "vbhide": I32(0), "vbmaximizedfocus": I32(3), "vbminimizedfocus": I32(2),
    "vbempty": I16(0), "vbnull": I16(1), "vbinteger": I16(2), "vblong": I16(3), "vbsingle": I16(4),
    "vbdouble": I16(5), "vbcurrency": I16(6), "vbdate": I16(7), "vbstring": I16(8), "vbobject": I16(9),
    "vberror": I16(10), "vbboolean": I16(11), "vbvariant": I16(12), "vbbyte": I16(17),
    "vbarray": I16(8192), "vbusesystem": I32(0), "vbsunday": I32(1),
    "vbupperCase".lower(): I32(1), "vblowercase": I32(2), "vbpropercase": I32(3),
    "vbwhite": I32(0xFFFFFF), "vbblack": I32(0), "vbred": I32(0xFF), "vbgreen": I32(0xFF00),
    "vbblue": I32(0xFF0000), "vbyellow": I32(0xFFFF), "vbmagenta": I32(0xFF00FF), "vbcyan": I32(0xFFFF00),
    "true": True, "false": False,
}


def _s(v):
    return V.c_str(v)


def _n(v):
    return V.int_value(v)


def _opt(args, i, default=None):
    if i < len(args) and args[i] is not MISSING:
        return args[i]
    return default


def _compare_flag(v):
    return v is not None and V.int_value(v) == 1


# ------------------------------------------------------------------ numeric

def f_abs(it, x):
    x = V.to_boolnum(x)
    if type(x) is str:
        x = V.str_to_double(x)
    t = type(x)
    if t is U8:
        return x
    if t is I16:
        return V.mk_i16(abs(int(x)))
    if t is I32:
        return V.mk_i32(abs(int(x)))
    if t is Cur:
        return V.mk_cur(abs(int(x)))
    if t is F32:
        return F32(abs(float(x)))
    if V.is_num(x):
        return abs(x)
    raise V.mismatch()


def f_sgn(it, x):
    f = V.as_float(V.to_boolnum(x)) if type(V.to_boolnum(x)) is not Cur else int(x)
    return I16(1 if f > 0 else -1 if f < 0 else 0)


def _int_like(x, fn_int, fn_float):
    x = V.to_boolnum(x)
    if type(x) is str:
        x = V.str_to_double(x)
    t = type(x)
    if t in V.INT_TYPES:
        return x
    if t is Cur:
        return V.mk_cur(fn_int(int(x)) * 10000)
    if t is F32:
        return F32(fn_float(float(x)))
    if t is float:
        return float(fn_float(x))
    raise V.mismatch()


def f_int(it, x):
    return _int_like(x, lambda n: n // 10000, math.floor)


def f_fix(it, x):
    def cfix(n):
        return abs(n) // 10000 * (1 if n >= 0 else -1)
    return _int_like(x, cfix, lambda f: float(math.trunc(f)) + 0.0)


def f_sqr(it, x):
    f = V.as_float(x)
    if f < 0:
        raise VBError(5)
    return math.sqrt(f)


def f_log(it, x):
    f = V.as_float(x)
    if f <= 0:
        raise VBError(5)
    return math.log(f)


def f_exp(it, x):
    try:
        return V.mk_dbl(math.exp(V.as_float(x)))
    except OverflowError:
        raise VBError(6)


def f_sin(it, x):
    return math.sin(V.as_float(x))


def f_cos(it, x):
    return math.cos(V.as_float(x))


def f_tan(it, x):
    return math.tan(V.as_float(x))


def f_atn(it, x):
    return math.atan(V.as_float(x))


def f_round(it, x, d=None):
    digits = 0 if d is None or d is MISSING else _n(d)
    x = V.to_boolnum(x)
    if type(x) is str:
        x = V.str_to_double(x)
    t = type(x)
    if digits < 0:
        raise VBError(5)
    if t in V.INT_TYPES:
        return x
    if t is Cur:
        q = 10 ** max(0, 4 - digits)
        return V.mk_cur(V.div_half_even(int(x), q) * q) if digits < 4 else x
    r = V.vb_round(float(x), digits)
    return V.f32(r) if t is F32 else r


def f_rnd(it, x=None):
    return it.rnd.next(None if x is None or x is MISSING else x)


def f_randomize(it, x=None):
    if x is None or x is MISSING:
        if it.randomize_hook is not None:
            it.randomize_hook(it.rnd)
        return None
    it.rnd.randomize(V.as_float(x))
    return None


def f_hex(it, x):
    n = V.int_value(x)
    if n < 0:
        n += 0x10000 if n >= -32768 and type(x) is not I32 else 0x100000000
    return "%X" % n


def f_oct(it, x):
    n = V.int_value(x)
    if n < 0:
        n += 0x100000000
    return "%o" % n


# ------------------------------------------------------------------ strings

def f_left(it, s, n):
    n = _n(n)
    if n < 0:
        raise VBError(5)
    return _s(s)[:n]


def f_right(it, s, n):
    n = _n(n)
    if n < 0:
        raise VBError(5)
    s = _s(s)
    return s[len(s) - n:] if n < len(s) else s


def f_mid(it, s, start, ln=None):
    start = _n(start)
    if start < 1:
        raise VBError(5)
    s = _s(s)
    if ln is None or ln is MISSING:
        return s[start - 1:]
    ln = _n(ln)
    if ln < 0:
        raise VBError(5)
    return s[start - 1:start - 1 + ln]


def f_len(it, x):
    if isinstance(x, (UDTInstance, ClsInstance)):
        raise VBError(13)
    if x is NULL:
        return NULL
    return I32(len(_s(x)))


def f_instr(it, *args):
    args = list(args)
    start = 1
    if len(args) >= 3 and (V.is_num(args[0]) or type(args[0]) is bool) :
        start = _n(args[0])
        args = args[1:]
    elif len(args) == 4:
        start = _n(args[0])
        args = args[1:]
    s1, s2 = _s(args[0]), _s(args[1])
    cmp = _opt(args, 2)
    if start < 1:
        raise VBError(5)
    if _compare_flag(cmp):
        s1, s2 = s1.lower(), s2.lower()
    if start > len(s1):
        return I32(0)
    if s2 == "":
        return I32(start)
    return I32(s1.find(s2, start - 1) + 1)


def f_instrrev(it, s1, s2, start=None, cmp=None):
    s1, s2 = _s(s1), _s(s2)
    if _compare_flag(cmp):
        s1, s2 = s1.lower(), s2.lower()
    end = len(s1) if start is None or start is MISSING or _n(start) == -1 else _n(start)
    if s2 == "":
        return I32(end)
    return I32(s1.rfind(s2, 0, end - 1 + len(s2)) + 1) if end >= 0 else I32(0)


def f_replace(it, expr, find, repl, start=None, count=None, cmp=None):
    s, find, repl = _s(expr), _s(find), _s(repl)
    st = 1 if start is None or start is MISSING else _n(start)
    cnt = -1 if count is None or count is MISSING else _n(count)
    if find == "":
        return s
    s = s[st - 1:]
    if _compare_flag(cmp):
        out, i, done = [], 0, 0
        low, lf = s.lower(), find.lower()
        while True:
            j = low.find(lf, i)
            if j < 0 or (cnt >= 0 and done >= cnt):
                break
            out.append(s[i:j])
            out.append(repl)
            i = j + len(find)
            done += 1
        out.append(s[i:])
        return "".join(out)
    return s.replace(find, repl, cnt if cnt >= 0 else -1)


def f_trim(it, s):
    return _s(s).strip(" ")


def f_ltrim(it, s):
    return _s(s).lstrip(" ")


def f_rtrim(it, s):
    return _s(s).rstrip(" ")


def f_ucase(it, s):
    return _s(s).upper()


def f_lcase(it, s):
    return _s(s).lower()


def f_space(it, n):
    n = _n(n)
    if n < 0:
        raise VBError(5)
    return " " * n


def f_string(it, n, ch):
    n = _n(n)
    if n < 0:
        raise VBError(5)
    c = ch if type(ch) is str else chr(_n(ch))
    return c[:1] * n if c else (_ for _ in ()).throw(VBError(5))


def f_chr(it, n):
    n = _n(n)
    if not 0 <= n <= 255:
        raise VBError(5)
    return bytes([n]).decode("cp1252", "replace")


def f_chrw(it, n):
    n = _n(n)
    if n < 0:
        n += 65536
    return chr(n)


def f_asc(it, s):
    s = _s(s)
    if s == "":
        raise VBError(5)
    try:
        return I16(s[0].encode("cp1252")[0])
    except UnicodeEncodeError:
        return I16(ord(s[0]) & 0xFF)


def f_strcomp(it, a, b, cmp=None):
    a, b = _s(a), _s(b)
    if _compare_flag(cmp):
        a, b = a.lower(), b.lower()
    return I16((a > b) - (a < b))


def f_split(it, s, delim=None, limit=None, cmp=None):
    s = _s(s)
    d = " " if delim is None or delim is MISSING else _s(delim)
    lim = -1 if limit is None or limit is MISSING else _n(limit)
    if s == "":
        parts = []
    elif d == "":
        parts = [s]
    else:
        if _compare_flag(cmp):
            parts, i, low, ld = [], 0, s.lower(), d.lower()
            while True:
                j = low.find(ld, i)
                if j < 0 or (lim > 0 and len(parts) == lim - 1):
                    break
                parts.append(s[i:j])
                i = j + len(d)
            parts.append(s[i:])
        else:
            parts = s.split(d, lim - 1) if lim > 0 else s.split(d)
    a = VBArray("string")
    a.allocate([(0, len(parts) - 1)])
    a.data = list(parts)
    return a


def f_join(it, arr, delim=None):
    d = " " if delim is None or delim is MISSING else _s(delim)
    if not isinstance(arr, VBArray):
        raise VBError(13)
    return d.join(_s(x) for x in arr)


def f_array(it, *args):
    a = VBArray("variant")
    a.allocate([(0, len(args) - 1)])
    a.data = list(args)
    return a


def f_str(it, x):
    s = V.c_str(x if not isinstance(x, str) else V.str_to_double(x))
    return s if s.startswith("-") else " " + s


def f_val(it, s):
    return V.val(s)


def f_format(it, x, fmt=None, *rest):
    if fmt is None or fmt is MISSING:
        return _s(x)
    fmt = _s(fmt)
    if type(x) is str:
        try:
            x = V.str_to_double(x)
        except VBError:
            return x
    if x is NULL:
        return ""
    low = fmt.lower()
    if low == "general number":
        return _s(V.as_float(x))
    if low == "fixed":
        return V.vb_format_num(V.as_float(x), "0.00")
    if low == "standard":
        return V.vb_format_num(V.as_float(x), "#,##0.00")
    if low == "percent":
        return V.vb_format_num(V.as_float(x) * 100, "0.00") + "%"
    if low in ("yes/no", "on/off", "true/false"):
        a, b = low.split("/")
        return (a if V.as_float(x) != 0 else b).capitalize()
    if "%" in fmt and set(fmt) <= set("0#,.%"):
        return V.vb_format_num(V.as_float(x) * 100, fmt.replace("%", "")) + "%"
    if type(x) is Cur:
        v = int(x) / 10000.0
    else:
        v = V.as_float(x)
    return V.vb_format_num(v, fmt)


def f_formatnumber(it, x, digits=None, lead=None, parens=None, group=None):
    """FormatNumber(x, digits, IncludeLeadingDigit, UseParensForNegativeNumbers, GroupDigits) in the US locale: an
    omitted (or -2) tri-state is the locale default (leading digit, no parens, grouped); -1 digits is 2."""
    def opt(a, default):
        return default if a is None or a is MISSING else _n(a)
    d = opt(digits, -1)
    d = 2 if d < 0 else d
    if type(x) is str:
        x = V.str_to_double(x)
    v = int(x) / 10000.0 if type(x) is Cur else V.as_float(x)
    lead, parens, group = opt(lead, -2), opt(parens, -2), opt(group, -2)
    pat = ("#,##0" if group != 0 else "0") + ("." + "0" * d if d else "")
    s = V.vb_format_num(abs(v), pat)
    if lead == 0 and s.startswith("0."):
        s = s[1:]
    if v < 0 and s.strip("0.,") != "":
        s = "(" + s + ")" if parens == -1 else "-" + s
    return s


def f_cstr(it, x):
    return _s(x)


def f_like(s, pat, text_compare=False):
    s, pat = _s(s), _s(pat)
    if text_compare:
        s, pat = s.lower(), pat.lower()
    return _like(s, 0, pat, 0)


def _like(s, si, p, pi):
    while pi < len(p):
        c = p[pi]
        if c == "*":
            while pi < len(p) and p[pi] == "*":
                pi += 1
            if pi == len(p):
                return True
            for k in range(si, len(s) + 1):
                if _like(s, k, p, pi):
                    return True
            return False
        if si >= len(s):
            return False
        if c == "?":
            si += 1
            pi += 1
        elif c == "#":
            if not s[si].isdigit():
                return False
            si += 1
            pi += 1
        elif c == "[":
            j = p.find("]", pi + 1)
            if j < 0:
                if s[si] != "[":
                    return False
                si += 1
                pi += 1
                continue
            body = p[pi + 1:j]
            neg = body.startswith("!")
            if neg:
                body = body[1:]
            ok = False
            k = 0
            while k < len(body):
                if k + 2 < len(body) and body[k + 1] == "-":
                    if body[k] <= s[si] <= body[k + 2]:
                        ok = True
                    k += 3
                else:
                    if body[k] == s[si]:
                        ok = True
                    k += 1
            if ok == neg:
                return False
            si += 1
            pi = j + 1
        else:
            if s[si] != c:
                return False
            si += 1
            pi += 1
    return si == len(s)


# ------------------------------------------------------------------ type tests / misc

def f_iif(it, c, a, b):
    return a if V.truth(c) else b


def f_switch(it, *args):
    for i in range(0, len(args) - 1, 2):
        if V.truth(args[i]):
            return args[i + 1]
    return NULL


def f_choose(it, idx, *args):
    i = _n(idx)
    if 1 <= i <= len(args):
        return args[i - 1]
    return NULL


def f_isnumeric(it, x):
    if type(x) is bool or V.is_num(x) or x is EMPTY:
        return True
    if type(x) is str:
        try:
            V.parse_vb_number(x)
            return True
        except (ValueError, OverflowError):
            return False
    return False


def f_isempty(it, x):
    return x is EMPTY


def f_isnull(it, x):
    return x is NULL


def f_ismissing(it, x):
    return x is MISSING


def f_isarray(it, x):
    return isinstance(x, VBArray)


def f_isobject(it, x):
    return x is NOTHING or isinstance(x, (ClsInstance, ExtObject, UDTInstance))


def f_typename(it, x):
    if isinstance(x, VBArray):
        return "Variant()"
    if isinstance(x, UDTInstance):
        return x.defn.name
    if isinstance(x, ClsInstance):
        return x.cls.name
    return V.type_name_of(x)


def f_vartype(it, x):
    m = {U8: 17, I16: 2, I32: 3, Cur: 6, F32: 4, float: 5, str: 8, bool: 11, V.DateV: 7}
    if x is EMPTY:
        return I16(0)
    if x is NULL:
        return I16(1)
    if isinstance(x, VBArray):
        return I16(8192 + 12)
    return I16(m.get(type(x), 9))


def f_lbound(it, a, dim=None):
    if not isinstance(a, VBArray):
        raise VBError(13)
    return I32(a.lbound(1 if dim is None else _n(dim)))


def f_ubound(it, a, dim=None):
    if not isinstance(a, VBArray):
        raise VBError(13)
    return I32(a.ubound(1 if dim is None else _n(dim)))


def f_timer(it):
    t = time.localtime()
    return F32(t.tm_hour * 3600 + t.tm_min * 60 + t.tm_sec)


def f_now(it):
    import datetime
    d = datetime.datetime.now() - datetime.datetime(1899, 12, 30)
    return V.DateV(d.days + d.seconds / 86400.0)


def f_doevents(it):
    return I16(0)


def _unbound(name):
    """An intrinsic the oracle does not run. A harness can provide it through
    `Interpreter.intrinsic_hooks[name.lower()] = fn(it, *args)` (MsgBox, CreateObject, ...)."""
    def f(it, *a):
        hook = it.intrinsic_hooks.get(name.lower())
        if hook is not None:
            return hook(it, *a)
        raise OracleUnbound(name)
    f.vb_unbound = name
    return f


def f_cvar(it, x):
    return x


def f_lenb(it, x):
    return I32(len(_s(x)) * 2)


def f_cdec(it, x):
    # Decimal is not modelled; Double carries the same value for the ranges MME uses
    return V.c_dbl(x)


def f_cdate(it, x):
    return V.c_date(x)


def f_cdbl(it, x):
    return V.c_dbl(x)


BUILTINS = {
    "abs": f_abs, "sgn": f_sgn, "int": f_int, "fix": f_fix, "sqr": f_sqr, "log": f_log, "exp": f_exp,
    "sin": f_sin, "cos": f_cos, "tan": f_tan, "atn": f_atn, "round": f_round, "rnd": f_rnd,
    "randomize": f_randomize, "hex": f_hex, "oct": f_oct,
    "cint": lambda it, x: V.c_int(x), "clng": lambda it, x: V.c_long(x), "csng": lambda it, x: V.c_sng(x),
    "cdbl": f_cdbl, "ccur": lambda it, x: V.c_cur(x), "cbool": lambda it, x: V.c_bool(x),
    "cbyte": lambda it, x: V.c_byte(x), "cstr": f_cstr, "cvar": f_cvar, "cdate": f_cdate, "lenb": f_lenb, "cdec": f_cdec,
    "left": f_left, "right": f_right, "mid": f_mid, "len": f_len, "instr": f_instr, "instrrev": f_instrrev,
    "replace": f_replace, "trim": f_trim, "ltrim": f_ltrim, "rtrim": f_rtrim, "ucase": f_ucase,
    "lcase": f_lcase, "space": f_space, "string": f_string, "chr": f_chr, "chrw": f_chrw, "asc": f_asc,
    "ascw": f_asc, "strcomp": f_strcomp, "split": f_split, "join": f_join, "array": f_array, "str": f_str,
    "val": f_val, "format": f_format, "formatnumber": f_formatnumber, "iif": f_iif, "switch": f_switch, "choose": f_choose,
    "isnumeric": f_isnumeric, "isempty": f_isempty, "isnull": f_isnull, "ismissing": f_ismissing,
    "isarray": f_isarray, "isobject": f_isobject, "typename": f_typename, "vartype": f_vartype,
    "lbound": f_lbound, "ubound": f_ubound, "timer": f_timer, "now": f_now, "doevents": f_doevents,
    "msgbox": _unbound("MsgBox"), "inputbox": _unbound("InputBox"), "shell": _unbound("Shell"),
    "createobject": _unbound("CreateObject"), "getobject": _unbound("GetObject"),
    "environ": _unbound("Environ"), "dir": _unbound("Dir"), "doevents_": f_doevents,
}

# Functions that must not be called with the type suffix stripped differently
SUFFIX_OK = {"left", "right", "mid", "trim", "ltrim", "rtrim", "ucase", "lcase", "space", "string",
             "chr", "hex", "oct", "format", "str", "replace"}

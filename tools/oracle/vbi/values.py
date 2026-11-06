"""VB6 value model: typed numbers, coercion, arithmetic, comparison and text conversion.

Integer/Long/Byte/Currency are int subclasses (Currency holds the value scaled by 10000),
Single is a float subclass rounded to binary32, Double is a plain float, Boolean a plain bool.
"""
import math
import struct
from decimal import Decimal

from .errors import VBError


class I16(int):
    __slots__ = ()


class I32(int):
    __slots__ = ()


class U8(int):
    __slots__ = ()


class Cur(int):
    """Currency: the int value is the amount times 10000."""
    __slots__ = ()


class F32(float):
    __slots__ = ()


class _Sentinel:
    def __init__(self, name):
        self.name = name

    def __repr__(self):
        return self.name


EMPTY = _Sentinel("Empty")
NULL = _Sentinel("Null")
NOTHING = _Sentinel("Nothing")
MISSING = _Sentinel("Missing")

CUR_MAX = 2 ** 63 - 1
CUR_MIN = -2 ** 63

NUMERIC_TYPES = (U8, I16, I32, Cur, F32, float)
INT_TYPES = (U8, I16, I32)


def is_num(v):
    t = type(v)
    return t in (U8, I16, I32, Cur, F32, float)


def overflow():
    return VBError(6)


def mismatch():
    return VBError(13)


# ------------------------------------------------------------------ constructors

def mk_i16(n):
    if -32768 <= n <= 32767:
        return I16(n)
    raise overflow()


def mk_i32(n):
    if -2147483648 <= n <= 2147483647:
        return I32(n)
    raise overflow()


def mk_u8(n):
    if 0 <= n <= 255:
        return U8(n)
    raise overflow()


def mk_cur(n):
    if CUR_MIN <= n <= CUR_MAX:
        return Cur(n)
    raise overflow()


def f32(x):
    # Rounding a finite Double to Single overflows (VB6 error 6) when it lands beyond the Single
    # range. Older Pythons raise OverflowError from struct.pack; newer ones return infinity, so
    # check the result as well.
    try:
        r = struct.unpack("f", struct.pack("f", x))[0]
    except OverflowError:
        raise overflow()
    if math.isinf(r) and not math.isinf(x):
        raise overflow()
    return F32(r)


def mk_dbl(x):
    if x != x or x in (math.inf, -math.inf):
        raise overflow()
    return x


def round_even_float(x):
    """Banker's rounding of a float to a Python int (exact, uses the binary value)."""
    if x != x or x in (math.inf, -math.inf):
        raise overflow()
    return round(x)


def div_half_even(n, d):
    q, r = divmod(n, d)           # floor division
    twice = 2 * r
    if d > 0:
        if twice > d or (twice == d and q % 2 == 1):
            q += 1
    else:
        if twice < d or (twice == d and q % 2 == 1):
            q += 1
    return q


# ------------------------------------------------------------------ numeric view

def to_boolnum(v):
    """Booleans and Empty become Integer for arithmetic."""
    t = type(v)
    if t is bool:
        return I16(-1 if v else 0)
    if v is EMPTY:
        return I16(0)
    return v


def as_float(v):
    """Numeric value as Python float (Currency descaled)."""
    t = type(v)
    if t is Cur:
        return int(v) / 10000.0
    if t is bool:
        return -1.0 if v else 0.0
    if t is float or t is F32:
        return float(v)
    if t in (U8, I16, I32):
        return float(int(v))
    if v is EMPTY:
        return 0.0
    if t is str:
        return str_to_double(v)
    if v is NULL:
        raise VBError(94)
    raise mismatch()


def parse_vb_number(s):
    """CDbl's string grammar under the US locale; also the rule behind IsNumeric.

    Returns a float, raises ValueError when the text is not a number and OverflowError when it is
    one but does not fit a Double (or a Long, for &H/&O literals). Accepted: outer whitespace, one
    sign (leading, after a currency sign, or trailing) or parentheses meaning negative, a leading
    "$", digits with "," thousands separators in the integer part, a decimal point, and an E/D
    exponent. &H/&O literals take no sign and no separators.
    """
    t = s.strip(" \t\r\n\x0b\x0c\xa0")
    if t == "":
        raise ValueError(s)
    u = t.lower()
    if u.startswith("&h") or u.startswith("&o"):
        hexa = u[1] == "h"
        d = u[2:]
        if d.endswith("&"):
            d = d[:-1]
            long_form = True
        else:
            long_form = False
        digits = "0123456789abcdef" if hexa else "01234567"
        if d == "" or any(c not in digits for c in d):
            raise ValueError(s)
        n = int(d, 16 if hexa else 8)
        sig = d.lstrip("0")
        if hexa and len(sig) <= 4 and not long_form:
            n = n - 0x10000 if n > 0x7FFF else n
        else:
            if n > 0xFFFFFFFF:
                raise OverflowError(s)
            n = n - 0x100000000 if n > 0x7FFFFFFF else n
        return float(n)
    neg = False
    signs = 0
    if u.startswith("(") and u.endswith(")"):
        neg, signs, u = True, 1, u[1:-1].strip(" ")
    if u[:1] in ("+", "-"):
        neg, signs, u = u[0] == "-", signs + 1, u[1:].lstrip(" ")
    if u.startswith("$"):
        u = u[1:].lstrip(" ")
        if u[:1] in ("+", "-"):
            neg, signs, u = u[0] == "-", signs + 1, u[1:].lstrip(" ")
    if u[-1:] in ("+", "-"):
        neg, signs, u = u[-1] == "-", signs + 1, u[:-1].rstrip(" ")
    if signs > 1 or u == "":
        raise ValueError(s)
    i, n = 0, len(u)
    mant = 0
    while i < n and (u[i] in "0123456789" or (u[i] == "," and i > 0 and u[i - 1] in "0123456789,")):
        mant += u[i] != ","
        i += 1
    ipart = u[:i].replace(",", "")
    fpart = ""
    if i < n and u[i] == ".":
        j = i + 1
        while j < n and u[j] in "0123456789":
            j += 1
        fpart = u[i + 1:j]
        mant += len(fpart)
        i = j
    if mant == 0:
        raise ValueError(s)
    exp = ""
    if i < n and u[i] in "ed":
        j = i + 1
        if j < n and u[j] in "+-":
            j += 1
        k = j
        while k < n and u[k] in "0123456789":
            k += 1
        if k == j:
            raise ValueError(s)
        exp = "e" + u[i + 1:k]
        i = k
    if i != n:
        raise ValueError(s)
    v = float((ipart or "0") + "." + (fpart or "0") + exp)
    if v in (float("inf"), float("-inf")):
        raise OverflowError(s)
    return -v if neg else v


def str_to_double(s):
    """Full-string numeric conversion used by implicit coercion (type mismatch if not numeric)."""
    try:
        return parse_vb_number(s)
    except OverflowError:
        raise VBError(6)
    except ValueError:
        raise mismatch()


def int_value(v):
    """Numeric value rounded half-even to a Python int, for \\, Mod and the logic operators."""
    v = to_boolnum(v)
    t = type(v)
    if t in INT_TYPES:
        return int(v)
    if t is Cur:
        return div_half_even(int(v), 10000)
    if t is float or t is F32:
        return round_even_float(float(v))
    if t is str:
        return round_even_float(str_to_double(v))
    if v is NULL:
        raise VBError(94)
    raise mismatch()


# ------------------------------------------------------------------ conversions (CInt etc.)

def c_int(v):
    v = to_boolnum(v)
    if type(v) is I16:
        return v
    return mk_i16(int_value(v))


def c_long(v):
    v = to_boolnum(v)
    if type(v) is I32:
        return v
    return mk_i32(int_value(v))


def c_byte(v):
    return mk_u8(int_value(v))


def c_dbl(v):
    v = to_boolnum(v)
    if type(v) is float:
        return v
    return as_float(v)


def c_sng(v):
    v = to_boolnum(v)
    if type(v) is F32:
        return v
    return f32(as_float(v))


def c_cur(v):
    v = to_boolnum(v)
    t = type(v)
    if t is Cur:
        return v
    if t in INT_TYPES:
        return mk_cur(int(v) * 10000)
    if t is str:
        v = str_to_double(v)
    x = float(v)
    r = x * 10000.0
    if r != r or abs(r) >= 9.223372036854776e18:
        raise overflow()
    return mk_cur(round(r))


def c_bool(v):
    if type(v) is bool:
        return v
    if v is EMPTY:
        return False
    if type(v) is str:
        low = v.strip().lower()
        if low == "true":
            return True
        if low == "false":
            return False
        return as_float(v) != 0
    if v is NULL:
        raise VBError(94)
    if type(v) is Cur:
        return int(v) != 0
    if is_num(v):
        return v != 0
    raise mismatch()


def cstr_double(x):
    if x == 0:
        return "0"
    sci = "%.14e" % abs(x)
    mant, exp = sci.split("e")
    exp = int(exp)
    digits = mant.replace(".", "").rstrip("0") or "0"
    sign = "-" if x < 0 else ""
    if exp < -4 or exp >= 15:
        head, tail = digits[0], digits[1:]
        m = head + ("." + tail if tail else "")
        return "%s%sE%s%02d" % (sign, m, "-" if exp < 0 else "+", abs(exp))
    n = len(digits)
    if exp < 0:
        body = "0." + "0" * (-exp - 1) + digits
    elif n <= exp + 1:
        body = digits + "0" * (exp + 1 - n)
    else:
        body = digits[:exp + 1] + "." + digits[exp + 1:]
    return sign + body


def cstr_single(x):
    if x == 0:
        return "0"
    sci = "%.6e" % abs(x)
    mant, exp = sci.split("e")
    exp = int(exp)
    digits = mant.replace(".", "").rstrip("0") or "0"
    sign = "-" if x < 0 else ""
    if exp < -4 or exp >= 7:
        head, tail = digits[0], digits[1:]
        m = head + ("." + tail if tail else "")
        return "%s%sE%s%02d" % (sign, m, "-" if exp < 0 else "+", abs(exp))
    n = len(digits)
    if exp < 0:
        body = "0." + "0" * (-exp - 1) + digits
    elif n <= exp + 1:
        body = digits + "0" * (exp + 1 - n)
    else:
        body = digits[:exp + 1] + "." + digits[exp + 1:]
    return sign + body


def cur_to_str(n):
    sign = "-" if n < 0 else ""
    a = abs(int(n))
    i, f = divmod(a, 10000)
    s = "%d" % i
    if f:
        s += "." + ("%04d" % f).rstrip("0")
    return sign + s


def c_str(v):
    t = type(v)
    if t is str:
        return v
    if t is bool:
        return "True" if v else "False"
    if t in INT_TYPES:
        return str(int(v))
    if t is float:
        return cstr_double(v)
    if t is F32:
        return cstr_single(v)
    if t is Cur:
        return cur_to_str(v)
    if v is EMPTY:
        return ""
    if v is NULL:
        raise VBError(94)
    if t is DateV:
        return fmt_date(v)
    d = getattr(v, "vb_default", None)      # an object in a String context: its default property
    if d is not None:
        return c_str(d([]))
    raise mismatch()


class DateV(float):
    __slots__ = ()


def fmt_date(v):
    import datetime
    base = datetime.datetime(1899, 12, 30)
    d = base + datetime.timedelta(days=float(v))
    if float(v) == int(v):
        return "%d/%d/%d" % (d.month, d.day, d.year)
    return "%d/%d/%d %d:%02d:%02d %s" % (d.month, d.day, d.year, (d.hour % 12) or 12, d.minute, d.second,
                                          "AM" if d.hour < 12 else "PM")


def to_string(v):
    return c_str(v)


# ------------------------------------------------------------------ default values / coercion by declared type

TYPE_CODES = {"integer", "long", "single", "double", "currency", "string", "boolean", "byte", "variant",
              "date", "object", "any"}


def default_for(tc):
    if tc == "integer":
        return I16(0)
    if tc == "long":
        return I32(0)
    if tc == "byte":
        return U8(0)
    if tc == "single":
        return F32(0.0)
    if tc == "double":
        return 0.0
    if tc == "currency":
        return Cur(0)
    if tc == "string":
        return ""
    if tc == "boolean":
        return False
    if tc == "date":
        return DateV(0.0)
    if tc == "variant":
        return EMPTY
    return NOTHING


def c_date(v):
    if type(v) is DateV:
        return v
    if type(v) is str:
        raise VBError(13)
    return DateV(as_float(v))


COERCE = {
    "integer": c_int, "long": c_long, "byte": c_byte, "single": c_sng, "double": c_dbl,
    "currency": c_cur, "string": c_str, "boolean": c_bool, "date": c_date,
}


# the VB type each coercion target keeps unchanged (coercing a value of exactly this type is the identity)
EXACT_TYPE = {"integer": I16, "long": I32, "byte": U8, "single": F32, "double": float,
              "currency": Cur, "string": str, "boolean": bool}


def coerce_to(v, tc):
    """Implicit conversion on assignment / argument passing to a declared type."""
    f = COERCE.get(tc)
    if f is None:
        return v
    return f(v)


def type_name_of(v):
    t = type(v)
    return {U8: "Byte", I16: "Integer", I32: "Long", Cur: "Currency", F32: "Single", float: "Double",
            str: "String", bool: "Boolean", DateV: "Date"}.get(t) or (
        "Empty" if v is EMPTY else "Null" if v is NULL else "Nothing" if v is NOTHING else "Object")


# ------------------------------------------------------------------ arithmetic

_RANK = {U8: 1, I16: 2, I32: 3, F32: 4, Cur: 5, float: 6}
_INTS = (U8, I16, I32)


def _prep(v):
    v = to_boolnum(v)
    if type(v) is str:
        return v
    if v is NULL:
        raise VBError(94)
    if type(v) is DateV:
        return float(v)
    if v is NOTHING or not is_num(v):
        raise mismatch()
    return v


def _result_rank(a, b):
    ra, rb = _RANK[type(a)], _RANK[type(b)]
    r = max(ra, rb)
    if (ra == 3 and rb == 4) or (ra == 4 and rb == 3):      # Long with Single widens to Double
        r = 6
    return r


def _fit(n, rank):
    if rank == 1:
        return mk_u8(n)
    if rank == 2:
        return mk_i16(n)
    return mk_i32(n)


def _cur_scaled(v):
    t = type(v)
    if t is Cur:
        return int(v)
    return int(v) * 10000


def arith(op, a, b):
    """op in + - *"""
    ta, tb = type(a), type(b)
    if ta is float and tb is float:
        z = a + b if op == "+" else a - b if op == "-" else a * b
        return mk_dbl(z)
    if ta in _INTS and tb in _INTS:
        x, y = int(a), int(b)
        n = x + y if op == "+" else x - y if op == "-" else x * y
        return _fit(n, _RANK[ta] if _RANK[ta] > _RANK[tb] else _RANK[tb])
    if (ta is float and tb in _INTS) or (tb is float and ta in _INTS):
        x, y = float(a), float(b)       # Double with an integer type is a Double operation
        z = x + y if op == "+" else x - y if op == "-" else x * y
        return mk_dbl(z)
    a = _prep(a)
    b = _prep(b)
    if type(a) is str or type(b) is str:
        if op == "+" and type(a) is str and type(b) is str:
            return a + b
        a = str_to_double(a) if type(a) is str else a
        b = str_to_double(b) if type(b) is str else b
    r = _result_rank(a, b)
    if r <= 3:
        x, y = int(a), int(b)
        n = x + y if op == "+" else x - y if op == "-" else x * y
        return _fit(n, r)
    if r == 5:
        a = c_cur(a) if type(a) is F32 else a
        b = c_cur(b) if type(b) is F32 else b
        x, y = _cur_scaled(a), _cur_scaled(b)
        if op == "+":
            return mk_cur(x + y)
        if op == "-":
            return mk_cur(x - y)
        return mk_cur(div_half_even(x * y, 10000))
    x, y = as_float(a), as_float(b)
    z = x + y if op == "+" else x - y if op == "-" else x * y
    if r == 4:
        return f32(z)
    return mk_dbl(z)


def add(a, b):
    return arith("+", a, b)


def sub(a, b):
    return arith("-", a, b)


def mul(a, b):
    return arith("*", a, b)


def div(a, b):
    ta, tb = type(a), type(b)
    if (ta is float or ta in INT_TYPES) and (tb is float or tb in INT_TYPES) and (ta is float or tb is float):
        y = float(b)
        if y == 0:
            raise VBError(11)
        return mk_dbl(float(a) / y)
    a = _prep(a)
    b = _prep(b)
    if type(a) is str:
        a = str_to_double(a)
    if type(b) is str:
        b = str_to_double(b)
    ta, tb = type(a), type(b)
    if (ta is Cur or tb is Cur) and ta in (U8, I16, I32, Cur) and tb in (U8, I16, I32, Cur):
        x, y = _cur_scaled(a), _cur_scaled(b)
        if y == 0:
            raise VBError(11)
        return mk_cur(div_half_even(x * 10000, y))
    single = (ta is F32 or tb is F32) and ta in (U8, I16, F32) and tb in (U8, I16, F32)
    x, y = as_float(a), as_float(b)
    if y == 0:
        raise VBError(11)
    z = x / y
    if single:
        return f32(z)
    return mk_dbl(z)


def power(a, b):
    x, y = as_float(_prep(a)), as_float(_prep(b))
    if x == 0 and y < 0:
        raise VBError(11)
    try:
        return mk_dbl(math.pow(x, y))
    except OverflowError:
        raise overflow()
    except ValueError:
        raise VBError(5)


def _both_int_typed(a, b):
    return type(a) in (U8, I16) and type(b) in (U8, I16)


def int_div(a, b):
    a, b = to_boolnum(a), to_boolnum(b)
    small = _both_int_typed(a, b)
    x, y = int_value(a), int_value(b)
    if not (-2147483648 <= x <= 2147483647) or not (-2147483648 <= y <= 2147483647):
        raise overflow()
    if y == 0:
        raise VBError(11)
    q = abs(x) // abs(y)
    if (x < 0) != (y < 0):
        q = -q
    return mk_i16(q) if small else mk_i32(q)


def mod(a, b):
    a, b = to_boolnum(a), to_boolnum(b)
    small = _both_int_typed(a, b)
    x, y = int_value(a), int_value(b)
    if not (-2147483648 <= x <= 2147483647) or not (-2147483648 <= y <= 2147483647):
        raise overflow()
    if y == 0:
        raise VBError(11)
    r = math.fmod(x, y)
    r = int(r)
    return mk_i16(r) if small else mk_i32(r)


def neg(a):
    a = _prep(a)
    if type(a) is str:
        return -str_to_double(a)
    t = type(a)
    if t is U8:
        return mk_i16(-int(a))
    if t is I16:
        return mk_i16(-int(a))
    if t is I32:
        return mk_i32(-int(a))
    if t is Cur:
        return mk_cur(-int(a))
    if t is F32:
        return F32(-float(a))
    return -a


def concat(a, b):
    if a is NULL or b is NULL:
        if a is NULL and b is NULL:
            return NULL
        return c_str(b) if a is NULL else c_str(a)
    return c_str(a) + c_str(b)


# ------------------------------------------------------------------ comparison

def compare(a, b):
    """Return -1/0/1, or None when unordered (never for our types)."""
    ta, tb = type(a), type(b)
    if (ta is float or ta in INT_TYPES) and (tb is float or tb in INT_TYPES):
        return (a > b) - (a < b)        # exact: integer types fit a double
    a, b = to_boolnum(a), to_boolnum(b)
    ta, tb = type(a), type(b)
    if ta is str and tb is str:
        return (a > b) - (a < b)
    if ta is str or tb is str:
        a = str_to_double(a) if ta is str else a
        b = str_to_double(b) if tb is str else b
        ta, tb = type(a), type(b)
    if a is NULL or b is NULL:
        raise VBError(94)
    if ta is Cur and tb is Cur:
        x, y = int(a), int(b)
    elif ta is Cur and tb in INT_TYPES:
        x, y = int(a), int(b) * 10000
    elif tb is Cur and ta in INT_TYPES:
        x, y = int(a) * 10000, int(b)
    elif ta is Cur or tb is Cur:
        x, y = as_float(a), as_float(b)
    elif ta in INT_TYPES and tb in INT_TYPES:
        x, y = int(a), int(b)
    else:
        x, y = as_float(a), as_float(b)
    return (x > y) - (x < y)


def logic(op, a, b):
    """And / Or / Xor / Eqv / Imp."""
    if type(a) is bool and type(b) is bool:
        if op == "and":
            return a and b
        if op == "or":
            return a or b
        if op == "xor":
            return a != b
        if op == "eqv":
            return a == b
        return (not a) or b
    x, y = to_boolnum(a), to_boolnum(b)
    small = type(x) in (U8, I16) and type(y) in (U8, I16)
    xi, yi = int_value(x), int_value(y)
    if op == "and":
        n = xi & yi
    elif op == "or":
        n = xi | yi
    elif op == "xor":
        n = xi ^ yi
    elif op == "eqv":
        n = ~(xi ^ yi)
    else:
        n = (~xi) | yi
    return mk_i16(n) if small else mk_i32(n)


def not_(a):
    if type(a) is bool:
        return not a
    if hasattr(a, "bounds") and hasattr(a, "elemtype"):
        # the `(Not Not arr) <> 0` idiom: Not Not of an array is its descriptor address, nonzero only
        # when allocated; model the address as 1.
        return mk_i32(~(0 if a.bounds is None else 1))
    x = to_boolnum(a)
    small = type(x) in (U8, I16)
    n = ~int_value(x)
    return mk_i16(n) if small else mk_i32(n)


def truth(v):
    """Condition value of If/While etc."""
    t = type(v)
    if t is bool:
        return v
    if t in INT_TYPES:
        return v != 0
    return c_bool(v)


# ------------------------------------------------------------------ Val / Str / Format / Round

def val(s):
    if type(s) is not str:
        s = c_str(s)
    chars = [c for c in s if c not in " \t\n\r\xa0"]
    if chars and chars[0] == "&":
        rest = chars[1:]
        if not rest or rest[0] not in "hHoO":
            return 0.0
        radix = 16 if rest[0] in "hH" else 8
        digs = []
        for c in rest[1:]:
            try:
                int(c, radix)
            except ValueError:
                break
            digs.append(c)
        if not digs:
            return 0.0
        after = rest[1 + len(digs)] if len(rest) > 1 + len(digs) else ""
        v = int("".join(digs), radix)
        limit = 4 if radix == 16 else 6
        if after == "&" or len(digs) > limit:
            v &= 0xFFFFFFFF
            return float(v - 0x100000000 if v > 0x7FFFFFFF else v)
        v &= 0xFFFF
        return float(v - 0x10000 if v > 0x7FFF else v)
    i = 0
    out = ""
    if chars and chars[0] in "+-":
        if chars[0] == "-":
            out = "-"
        i = 1
    nd = 0
    while i < len(chars) and chars[i].isdigit() and chars[i].isascii():
        out += chars[i]
        nd += 1
        i += 1
    if i < len(chars) and chars[i] == ".":
        out += "."
        i += 1
        while i < len(chars) and chars[i].isdigit() and chars[i].isascii():
            out += chars[i]
            nd += 1
            i += 1
    if nd == 0:
        return 0.0
    if i < len(chars) and chars[i] in "eEdD":
        j = i + 1
        exp = "e"
        if j < len(chars) and chars[j] in "+-":
            exp += chars[j]
            j += 1
        st = j
        while j < len(chars) and chars[j].isdigit() and chars[j].isascii():
            exp += chars[j]
            j += 1
        if j > st:
            out += exp
    try:
        x = float(out)
    except (ValueError, OverflowError):
        return 0.0
    if x in (math.inf, -math.inf):
        raise overflow()
    return x + 0.0


def vb_round(x, digits=0):
    if digits < 0:
        raise VBError(5)
    if digits == 0:
        return float(round_even_float(x))
    scale = 10.0 ** digits
    scaled = x * scale
    if scaled in (math.inf, -math.inf) or scaled != scaled:
        return x
    return round_even_float(scaled) / scale + 0.0


def _decimal_parts(x):
    if x == 0:
        return "0", ""
    sci = "%.14e" % x
    mant, exp = sci.split("e")
    exp = int(exp)
    d = mant.replace(".", "")
    if exp < 0:
        return "0", "0" * (-exp - 1) + d
    n = exp + 1
    ip = d[:n].ljust(n, "0")
    return ip, d[n:]


def vb_format_num(x, pattern):
    if "." in pattern:
        ipat, fpat = pattern.split(".", 1)
    else:
        ipat, fpat = pattern, None
    if pattern == "" or any(c not in "0#," for c in ipat) or (fpat is not None and any(c not in "0#" for c in fpat)):
        raise VBError(5)
    min_int = ipat.count("0")
    comma = "," in ipat
    min_frac = fpat.count("0") if fpat is not None else 0
    max_frac = len(fpat) if fpat is not None else 0
    ip, fp = _decimal_parts(abs(x))
    if len(fp) > max_frac:
        up = fp[max_frac] >= "5"
        fp = fp[:max_frac]
        if up:
            digits = list(ip + fp)
            k = len(digits) - 1
            while k >= 0:
                if digits[k] == "9":
                    digits[k] = "0"
                    k -= 1
                else:
                    digits[k] = chr(ord(digits[k]) + 1)
                    break
            else:
                digits.insert(0, "1")
            s = "".join(digits)
            ip, fp = s[:len(s) - len(fp)], s[len(s) - len(fp):]
    while len(fp) > min_frac and fp.endswith("0"):
        fp = fp[:-1]
    fp = fp.ljust(min_frac, "0")
    ip = ip.lstrip("0")
    nonzero = bool(ip) or any(c != "0" for c in fp)
    ip = ip.rjust(min_int, "0")
    if comma and ip:
        ip = _group(ip)
    out = ("-" if x < 0 and nonzero else "") + ip
    if fpat is not None:
        out += "." + fp
    return out


def _group(d):
    n = len(d)
    return "".join(("," if i > 0 and (n - i) % 3 == 0 else "") + c for i, c in enumerate(d))


# ------------------------------------------------------------------ Rnd

class Rnd:
    """VB6 Rnd/Randomize: 24-bit LCG, initial state 0x50000."""

    MASK = 0xFFFFFF

    def __init__(self, state=0x50000):
        self.state = state & self.MASK

    def _value(self):
        return F32(self.state / 16777216.0)

    def _advance(self):
        self.state = (self.state * 0x43FD43FD + 0xC39EC3) & self.MASK

    def next(self, arg=None):
        if arg is not None:
            n = struct.unpack("f", struct.pack("f", as_float(arg)))[0]
            if n == 0:
                return self._value()
            if n < 0:
                bits = struct.unpack("I", struct.pack("f", n))[0]
                self.state = ((bits & self.MASK) + ((bits >> 24) & 0xFF)) & self.MASK
        self._advance()
        return self._value()

    def randomize(self, n):
        b = struct.unpack("Q", struct.pack("d", float(n)))[0]
        v = (b >> 32) ^ (b & 0xFFFFFFFF)
        mixed = ((v >> 16) ^ v) & 0xFFFF
        self.state = (self.state & 0xFF) | (mixed << 8)

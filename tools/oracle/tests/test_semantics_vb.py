"""The semantics table again, but through VB source: each op becomes a tiny VB function, so the
parser, the typed-variable coercion and the operator wiring are covered, not just the helpers."""
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from vbi.interp import Interpreter  # noqa: E402
from vbi.errors import VBError  # noqa: E402
from vbi import values as V  # noqa: E402

FIXTURE = os.path.join(HERE, "..", "..", "..", "fixtures", "vb6-semantics.json")

# op -> (parameter declarations, VB expression). Parameter types follow the table header.
D2 = "ByVal a As Double, ByVal b As Double"
T = {
    "CInt": ("ByVal a As Double", "CInt(a)"),
    "CLng": ("ByVal a As Double", "CLng(a)"),
    "CSng": ("ByVal a As Double", "CSng(a)"),
    "CCur": ("ByVal a As Double", "CCur(a)"),
    "Int": ("ByVal a As Double", "Int(a)"),
    "Fix": ("ByVal a As Double", "Fix(a)"),
    "Abs": ("ByVal a As Double", "Abs(a)"),
    "Sgn": ("ByVal a As Double", "Sgn(a)"),
    "Sqr": ("ByVal a As Double", "Sqr(a)"),
    "Log": ("ByVal a As Double", "Log(a)"),
    "Exp": ("ByVal a As Double", "Exp(a)"),
    "IntDiv": (D2, "a \\ b"),
    "Mod": (D2, "a Mod b"),
    "Div": (D2, "a / b"),
    "Pow": (D2, "a ^ b"),
    "AddInt": ("ByVal a As Integer, ByVal b As Integer", "a + b"),
    "SubInt": ("ByVal a As Integer, ByVal b As Integer", "a - b"),
    "MulInt": ("ByVal a As Integer, ByVal b As Integer", "a * b"),
    "IntDivInt": ("ByVal a As Integer, ByVal b As Integer", "a \\ b"),
    "AddLng": ("ByVal a As Long, ByVal b As Long", "a + b"),
    "SubLng": ("ByVal a As Long, ByVal b As Long", "a - b"),
    "MulLng": ("ByVal a As Long, ByVal b As Long", "a * b"),
    "Eq": (D2, "a = b"), "Ne": (D2, "a <> b"), "Lt": (D2, "a < b"),
    "Le": (D2, "a <= b"), "Gt": (D2, "a > b"), "Ge": (D2, "a >= b"),
    "And": (D2, "a And b"), "Or": (D2, "a Or b"), "Xor": (D2, "a Xor b"),
    "Not": ("ByVal a As Double", "Not a"),
    "AddSng": ("ByVal a As Single, ByVal b As Single", "a + b"),
    "MulSng": ("ByVal a As Single, ByVal b As Single", "a * b"),
    "DivSng": ("ByVal a As Single, ByVal b As Single", "a / b"),
    "AddCur": ("ByVal a As Currency, ByVal b As Currency", "a + b"),
    "MulCur": ("ByVal a As Currency, ByVal b As Currency", "a * b"),
    "DivCur": ("ByVal a As Currency, ByVal b As Currency", "a / b"),
    "Str": ("ByVal a As Double", "Str(a)"),
    "Val": ("ByVal a As String", "Val(a)"),
    "Len": ("ByVal a As String", "Len(a)"),
    "CStr": ("ByVal a As Variant", "CStr(a)"),
    "CBool": ("ByVal a As Variant", "CBool(a)"),
    "Left": ("ByVal s As String, ByVal n As Long", "Left(s, n)"),
    "Right": ("ByVal s As String, ByVal n As Long", "Right$(s, n)"),
    "Format": ("ByVal a As Double, ByVal p As String", "Format(a, p)"),
}


def build_source():
    lines = ['Attribute VB_Name = "modOps"', "Option Explicit"]
    for op, (params, expr) in T.items():
        lines.append("Public Function Op%s(%s) As Variant" % (op, params))
        lines.append("    Op%s = %s" % (op, expr))
        lines.append("End Function")
    # assignment conversions go through typed variables rather than a conversion function
    lines += ["Public Function OpAssignInteger(ByVal a As Double) As Variant",
              "    Dim r As Integer", "    r = a", "    OpAssignInteger = r", "End Function",
              "Public Function OpAssignLong(ByVal a As Double) As Variant",
              "    Dim r As Long", "    r = a", "    OpAssignLong = r", "End Function"]
    return "\n".join(lines) + "\n"


def plain(v):
    t = type(v)
    if t is V.Cur:
        return int(v) / 10000.0
    if t in (V.I16, V.I32, V.U8):
        return int(v)
    if t is V.F32:
        return float(v)
    if t is bool:
        return -1 if v else 0
    return v


def to_arg(op, x):
    if isinstance(x, str) or isinstance(x, bool):
        return x
    if op in ("AddInt", "SubInt", "MulInt", "IntDivInt"):
        return V.I16(x)
    if op in ("AddLng", "SubLng", "MulLng"):
        return V.I32(x)
    if op in ("AddSng", "MulSng", "DivSng"):
        return V.f32(x)
    if op in ("AddCur", "MulCur", "DivCur"):
        return V.c_cur(float(x))
    return float(x)


class SemanticsThroughVB(unittest.TestCase):
    def test_ops(self):
        with open(FIXTURE, encoding="utf-8") as f:
            cases = json.load(f)["cases"]
        it = Interpreter()
        it.load_source(build_source(), "modOps")
        failures = []
        checked = 0
        for i, c in enumerate(cases):
            op = c["op"]
            fname = "Op" + op
            if fname.lower() not in it.procs:
                continue
            checked += 1
            args = [to_arg(op, x) for x in c["args"]]
            if op == "Concat":
                continue
            try:
                got = plain(it.call(fname, *args))
                err = None
            except VBError as e:
                got, err = None, e.number
            if "error" in c:
                ok = err == c["error"]
            else:
                want = c["result"]
                ok = err is None and got == want
            if not ok:
                failures.append("#%d %s %s -> %r err %r, want %r" % (
                    i, op, c["args"], got, err, c.get("result", c.get("error"))))
        self.assertGreater(checked, 420)
        self.assertEqual(failures, [], "\n" + "\n".join(failures[:30]))


if __name__ == "__main__":
    unittest.main()

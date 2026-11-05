"""Runs every case of fixtures/vb6-semantics.json (the contract shared with the Rust crate)."""
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from vbi import values as V  # noqa: E402
from vbi import builtins as B  # noqa: E402
from vbi.errors import VBError  # noqa: E402
from vbi.values import I16, I32, Rnd  # noqa: E402

FIXTURE = os.path.join(HERE, "..", "..", "..", "fixtures", "vb6-semantics.json")


class _It:
    def __init__(self):
        self.rnd = Rnd()
        self.randomize_hook = None


def _d(x):
    return x if isinstance(x, bool) else float(x)


def _b(v):
    return -1 if v else 0


def _plain(v):
    t = type(v)
    if t is V.Cur:
        return int(v) / 10000.0
    if t in (I16, I32, V.U8):
        return int(v)
    if t is V.F32:
        return float(v)
    if t is bool:
        return -1 if v else 0
    return v


def random_number(it, lo, hi):
    lo, hi = I16(lo), I16(hi)
    x = V.add(V.mul(V.add(V.sub(hi, lo), I16(1)), it.rnd.next()), lo)
    return V.c_int(B.f_int(it, x))


def apply_op(op, args):
    it = _It()
    c = B.BUILTINS
    a = args
    if op == "CInt":
        return V.c_int(_d(a[0]))
    if op == "CLng":
        return V.c_long(_d(a[0]))
    if op == "AssignInteger":
        return V.coerce_to(_d(a[0]), "integer")
    if op == "AssignLong":
        return V.coerce_to(_d(a[0]), "long")
    if op == "CSng":
        return V.c_sng(_d(a[0]))
    if op == "CCur":
        return V.c_cur(_d(a[0]))
    if op == "CBool":
        return _b(V.c_bool(a[0] if isinstance(a[0], bool) else _d(a[0])))
    if op in ("Int", "Fix", "Abs", "Sgn", "Sqr", "Log", "Exp"):
        return c[op.lower()](it, _d(a[0]))
    if op == "Round":
        return c["round"](it, _d(a[0]), *([I32(a[1])] if len(a) > 1 else []))
    if op == "IntDiv":
        return V.int_div(_d(a[0]), _d(a[1]))
    if op == "IntDivInt":
        return V.int_div(I16(a[0]), I16(a[1]))
    if op == "Mod":
        return V.mod(_d(a[0]), _d(a[1]))
    if op in ("AddInt", "SubInt", "MulInt"):
        f = {"Add": V.add, "Sub": V.sub, "Mul": V.mul}[op[:3]]
        return f(I16(a[0]), I16(a[1]))
    if op in ("AddLng", "SubLng", "MulLng"):
        f = {"Add": V.add, "Sub": V.sub, "Mul": V.mul}[op[:3]]
        return f(I32(a[0]), I32(a[1]))
    if op == "Div":
        return V.div(_d(a[0]), _d(a[1]))
    if op == "Pow":
        return V.power(_d(a[0]), _d(a[1]))
    if op in ("Eq", "Ne", "Lt", "Le", "Gt", "Ge"):
        r = V.compare(_d(a[0]), _d(a[1]))
        return _b({"Eq": r == 0, "Ne": r != 0, "Lt": r < 0, "Le": r <= 0, "Gt": r > 0, "Ge": r >= 0}[op])
    if op in ("And", "Or", "Xor"):
        return V.logic(op.lower(), _d(a[0]), _d(a[1]))
    if op == "Not":
        return V.not_(_d(a[0]))
    if op in ("AddSng", "SubSng", "MulSng", "DivSng"):
        x, y = V.f32(a[0]), V.f32(a[1])
        return {"Add": V.add, "Sub": V.sub, "Mul": V.mul, "Div": V.div}[op[:3]](x, y)
    if op in ("AddCur", "MulCur", "DivCur"):
        x, y = V.c_cur(_d(a[0])), V.c_cur(_d(a[1]))
        return {"Add": V.add, "Mul": V.mul, "Div": V.div}[op[:3]](x, y)
    if op == "CStr":
        return V.c_str(a[0] if isinstance(a[0], bool) else _d(a[0]))
    if op == "Str":
        return c["str"](it, _d(a[0]))
    if op == "Concat":
        out = ""
        for p in a:
            out = V.concat(out, p if isinstance(p, (str, bool)) else _d(p))
        return out
    if op == "Val":
        return V.val(a[0])
    if op in ("Left", "Right"):
        return c[op.lower()](it, a[0], I32(a[1]))
    if op == "Mid":
        return c["mid"](it, a[0], *[I32(x) for x in a[1:]])
    if op == "Len":
        return c["len"](it, a[0])
    if op == "IIf":
        cond = a[0] if isinstance(a[0], bool) else float(a[0])
        return c["iif"](it, cond, a[1], a[2])
    if op == "Format":
        return c["format"](it, _d(a[0]), a[1])
    if op == "RndScript":
        out = []
        for step in a[0]:
            k = step[0]
            if k == "Rnd":
                out.append(float(it.rnd.next(*([V.f32(step[1])] if len(step) > 1 else []))))
            elif k == "Randomize":
                it.rnd.randomize(float(step[1]))
                out.append(None)
            else:
                out.append(int(random_number(it, step[1], step[2])))
        return out
    raise AssertionError("unknown op " + op)


class SemanticsTable(unittest.TestCase):
    def test_table(self):
        with open(FIXTURE, encoding="utf-8") as f:
            data = json.load(f)
        self.assertGreater(len(data["cases"]), 400)
        failures = []
        for i, case in enumerate(data["cases"]):
            try:
                got = _plain(apply_op(case["op"], case["args"]))
                err = None
            except VBError as e:
                got, err = None, e.number
            if "error" in case:
                ok = err == case["error"]
            else:
                want = case["result"]
                ok = err is None and got == want and (type(got) is type(want) or isinstance(want, (int, float))
                                                       and isinstance(got, (int, float)))
            if not ok:
                failures.append("#%d %s %s -> got %r err %r, want %r" % (
                    i, case["op"], case["args"], got, err, case.get("result", case.get("error"))))
        self.assertEqual(failures, [], "\n" + "\n".join(failures[:40]))


if __name__ == "__main__":
    unittest.main()

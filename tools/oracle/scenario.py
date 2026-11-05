"""Scenario files: expansion of grids and seeded samples into concrete cases.

A scenario (fixtures/scenarios/<family>.json) holds family-wide setup plus `cases` (explicit),
`grids` (cartesian products) and `samples` (seeded random draws). Expansion is pure and uses its
own generator, so the same scenario always yields the same cases on any Python version.
"""
import copy

MASK = (1 << 64) - 1


class SplitMix:
    """splitmix64: tiny, fixed, identical everywhere."""

    def __init__(self, seed):
        self.s = seed & MASK

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def below(self, n):
        return self.next() % n


def _draw(rng, spec):
    n = spec.get("n")
    if n is not None:
        one = {k: v for k, v in spec.items() if k != "n"}
        return [_draw(rng, one) for _ in range(n)]
    if "int" in spec:
        lo, hi = spec["int"]
        return lo + rng.below(hi - lo + 1)
    if "choice" in spec:
        c = spec["choice"]
        return copy.deepcopy(c[rng.below(len(c))])
    raise ValueError("unknown draw spec %r" % (spec,))


def _zip(f, a, b):
    if isinstance(a, list) and isinstance(b, list):
        return [f(x, y) for x, y in zip(a, b)]
    if isinstance(a, list):
        return [f(x, b) for x in a]
    if isinstance(b, list):
        return [f(a, y) for y in b]
    return f(a, b)


_OPS = {
    "$add": lambda a, b: a + b,
    "$sub": lambda a, b: a - b,
    "$mul": lambda a, b: a * b,
    "$min": min,
    "$max": max,
}


def subst(node, env):
    if isinstance(node, str):
        if node.startswith("$") and node[1:] in env:
            return copy.deepcopy(env[node[1:]])
        return node
    if isinstance(node, list):
        return [subst(x, env) for x in node]
    if isinstance(node, dict):
        if len(node) == 1:
            (k, v), = node.items()
            if k in _OPS:
                vals = [subst(x, env) for x in v]
                out = vals[0]
                for x in vals[1:]:
                    out = _zip(_OPS[k], out, x)
                return out
            if k == "$merge":
                out = {}
                for x in v:
                    out.update(subst(x, env))
                return out
        return {k: subst(v, env) for k, v in node.items()}
    return node


def expand(scn):
    """Concrete cases in a stable order: explicit, then grids, then samples."""
    cases = []
    for c in scn.get("cases", []):
        cases.append(copy.deepcopy(c))
    for g in scn.get("grids", []):
        names = list(g["axes"].keys())
        combos = [[]]
        for nm in names:
            combos = [c + [v] for c in combos for v in g["axes"][nm]]
        for i, combo in enumerate(combos):
            env = dict(zip(names, combo))
            case = subst(g["case"], env)
            case.setdefault("id", "%s-%04d" % (g["name"], i))
            cases.append(case)
    for s in scn.get("samples", []):
        rng = SplitMix(s["seed"])
        for i in range(s["count"]):
            env = {nm: _draw(rng, sp) for nm, sp in s["draw"].items()}
            case = subst(s["case"], env)
            case.setdefault("id", "%s-%04d" % (s["name"], i))
            cases.append(case)
    ids = [c["id"] for c in cases]
    if len(set(ids)) != len(ids):
        raise ValueError("duplicate case ids in scenario %s" % scn.get("family"))
    return cases

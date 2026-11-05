"""Writes fixtures/scenarios/ability_text.json (FAM-03 text half: ability names and ability text).

    python3 -I tools/oracle/gen/ability_text.py [DB.sqlite]

Deterministic (fixed strides, no randomness). The case "table" carries the inputs the Rust port
needs in place of a database: the Spells table (compact, as in the spells family), the Name column
of Spells, Monsters, Items and Classes, and the TBInfo rows that ability 148 values point at.
Everything else is a call of GetAbilityName, GetAbilityList, GetAbilityStats or GetAbilDiffText:
every (ability, value) pair in the stock Items, Spells, Races, Classes and Monsters tables, the
variants that change the text, representative values for every ability number, and diff grids.
"""
import json
import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DEFAULT_DB = os.path.expanduser(
    "~/.cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite")
MISSING = {"t": "missing"}
FIELDS = ["Number", "ReqLevel", "EnergyCost", "MinBase", "MaxBase", "Targets", "Dur", "AttType", "Magery", "Cap",
          "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc"]
TABLES = [("Items", 20), ("Spells", 10), ("Races", 10), ("Classes", 10), ("Monsters", 10)]
# Abilities whose text is built from a database name or number.
NAMEY = (12, 42, 43, 59, 73, 122, 124, 146, 148, 151, 153, 160, 185)
# Abilities whose text nests a PullSpellEQ (and so depends on calc level, bQuickSpell and nSpellNest).
NESTING = (43, 148, 151, 153)


def load(db):
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    pairs = {}
    for t, n in TABLES:
        for i in range(n):
            for r in c.execute('select "Abil-%d" a, "AbilVal-%d" v from %s' % (i, i, t)):
                if r["a"]:
                    pairs.setdefault(r["a"], set()).add(r["v"])
    spells = []
    for r in c.execute("select * from Spells order by Number"):
        flat = []
        for i in range(10):
            if r["Abil-%d" % i]:
                flat += [r["Abil-%d" % i], r["AbilVal-%d" % i]]
        spells.append([r[k] for k in FIELDS] + [r["Learnable"], 1 if r["Learned From"] else 0, flat])
    names = {}
    for key, t in (("spells", "Spells"), ("monsters", "Monsters"), ("items", "Items"), ("classes", "Classes")):
        names[key] = [[r[0], r[1]] for r in c.execute("select Number, Name from %s order by Number" % t)]
    want = set(pairs.get(148, ()))
    tb = []
    for r in c.execute("select Number, Action from TBInfo order by Number"):
        a = r[1] or ""
        if r[0] in want or ("cast " in a.lower() and len(a) < 400):
            tb.append([r[0], a])
    return pairs, spells, names, tb


def main():
    pairs, spells, names, tb = load(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB)
    cases = [{"id": "table", "fields": FIELDS + ["Learnable", "LearnedFrom", "AbilitiesFlat"],
              "spells": spells, "names": names, "textblocks": tb}]
    grids = []

    def grid(name, axes, case):
        grids.append({"name": name, "axes": axes, "case": case})

    def stride(vals, k, off=0):
        return [v for i, v in enumerate(sorted(vals)) if i % k == off]

    stats = lambda a, extra=None, **kw: dict(
        {"proc": "GetAbilityStats", "args": [a, "$v", MISSING] + (extra or [])}, **kw)

    # --- GetAbilityName / GetAbilityList
    grid("nm", {"n": [-3, -1] + list(range(0, 215)) + list(range(215, 1000, 29)) + list(range(1000, 1126))
                     + [32767, -32768]}, {"proc": "GetAbilityName", "args": ["$n"]})
    grid("nmf", {"n": [-1] + list(range(0, 201)) + list(range(1001, 1005)) + list(range(1100, 1121, 4)) + [32767]},
         {"proc": "GetAbilityName", "args": ["$n", True]})
    for hide in (False, True):
        cases.append({"id": "list-%d" % hide, "proc": "GetAbilityList", "args": [],
                      "globals": {"bHideRecordNumbers": hide}})

    # --- GetAbilityStats over every stock (ability, value) pair
    for a in sorted(pairs):
        grid("as%d" % a, {"v": sorted(pairs[a])}, stats(a))
    # hidden record numbers: the abilities that print names
    for a in NAMEY:
        grid("hide%d" % a, {"v": stride(pairs.get(a, ()), 3)}, stats(a, globals={"bHideRecordNumbers": True}))
    # bCalcSpellLevel off / PercentColumn on
    for a in NESTING:
        vs = sorted(pairs.get(a, ()))
        grid("calc%d" % a, {"v": stride(vs, 2)}, stats(a, [False]))
        grid("pct%d" % a, {"v": stride(vs, 5, 1)}, stats(a, [True, True]))
        # quick spell with nSpellNest 1 (called from inside PullSpellEQ) and 0; the nesting guard near 20
        grid("quick%d" % a, {"v": stride(vs, 3, 1)}, stats(a, globals={"bQuickSpell": True, "nSpellNest": 1}))
        grid("quick0%d" % a, {"v": stride(vs, 6, 2)}, stats(a, globals={"bQuickSpell": True}))
        grid("deep%d" % a, {"v": stride(vs, 9, 3), "d": [18, 19, 25]}, stats(a, globals={"nSpellNest": "$d"}))
        # the global level box
        grid("lvl%d" % a, {"v": stride(vs, 8, 1), "t": ["40", "0", "x", "300"]},
             stats(a, forms={"frmMain": {"chkGlobalFilter": {"Value": 1},
                                         "txtGlobalLevel": {"$array": {"0": {"Text": "$t"}}}}}))
        grid("lvlq%d" % a, {"v": stride(vs, 10, 4)},
             stats(a, globals={"bQuickSpell": True, "nSpellNest": 1},
                   forms={"frmMain": {"chkGlobalFilter": {"Value": 1},
                                      "txtGlobalLevel": {"$array": {"0": {"Text": "25"}}}}}))
    # --- representative values for every ability number (values nobody in the stock tables uses)
    nums = list(range(0, 201)) + list(range(1001, 1005)) + list(range(1100, 1121))
    grid("rep", {"a": nums, "v": [0, 3, -3, 32767]}, {"proc": "GetAbilityStats", "args": ["$a", "$v", MISSING]})
    grid("rep7", {"v": [1, 9, 10, 15, -1, -15, 123, 32767, -32768, 5]}, stats(7))
    grid("rep42", {"v": [1, 2, 600, 1312, 1500, 5000, -1]}, stats(42))
    grid("rep43h", {"v": [1, 2, 600, 1312, 1500, 5000, -1]}, stats(43, globals={"bHideRecordNumbers": True}))
    grid("repnm", {"a": [59, 12, 146, 185, 1115, 42, 73, 124], "v": [0, 1, 4, 74, 75, 76, 999, 1389, 1412]},
         {"proc": "GetAbilityStats", "args": ["$a", "$v", MISSING]})
    grid("repnmh", {"a": [59, 12, 146, 185, 1115, 42, 73, 124], "v": [0, 1, 4, 74, 75, 999, 1389, 1412]},
         {"proc": "GetAbilityStats", "args": ["$a", "$v", MISSING], "globals": {"bHideRecordNumbers": True}})
    # textblocks that cast (multi-part, mixed, absent)
    castnums = [n for n, a in tb if "cast " in a.lower()]
    grid("tbcast", {"v": castnums + [0, -4, 30000, 32767]}, stats(148))
    grid("tbcastq", {"v": castnums[::2]}, stats(148, globals={"bQuickSpell": True}))
    grid("tbcastc", {"v": castnums[1::2]}, stats(148, [False]))

    # --- GetAbilDiffText
    diffs = [(5, 3), (3, 5), (0, 7), (-4, 2), (7, 7), (40000, 1), (2147483647, -5), (20, 0)]
    allab = sorted(set(pairs) | {0, 114, 1114, 101, 1115, 135, 2, 7, 22})
    for a in allab:
        for j, (v1, v2) in enumerate(diffs):
            if a in (43, 59, 135, 114, 1114, 7, 42, 2, 148) or (a + j) % 5 == 0:
                cases.append({"id": "df-%d-%d" % (a, j), "proc": "GetAbilDiffText",
                              "args": [a, v1, v2, MISSING if j % 2 else "", j % 3 * 10, j % 2 == 0 and a % 2 == 0]})
    for a in (43,):
        vs = sorted(pairs[a])
        grid("df43", {"v": stride(vs, 3), "p": [0, 25]}, {"proc": "GetAbilDiffText", "args": [43, "$v", "$v", "", "$p", False]})
        grid("df43h", {"v": stride(vs, 12, 1)}, {"proc": "GetAbilDiffText", "args": [43, "$v", "$v", "", 0, False],
                                                  "globals": {"bHideRecordNumbers": True, "bQuickSpell": True}})
    grid("df43x", {"v": [0, -3, 1, 99999, 2147483647]}, {"proc": "GetAbilDiffText", "args": [43, "$v", 0, "", 10, False]})
    grid("df59", {"v": sorted(pairs[59]) + [0, 1, 999]}, {"proc": "GetAbilDiffText", "args": [59, "$v", "$v", "", 0, False]})
    grid("df135", {"v": sorted(pairs[135])[::2]}, {"proc": "GetAbilDiffText", "args": [135, "$v", 3, "", 0, False]})
    grid("df135o", {"v": sorted(pairs[135])[::2]}, {"proc": "GetAbilDiffText", "args": [135, "$v", 3, "", 0, True]})
    cases.append({"id": "df-bigabil", "proc": "GetAbilDiffText", "args": [40000, 1, 0, "", 0, False]})
    cases.append({"id": "df-negabil", "proc": "GetAbilDiffText", "args": [-3, 1, 0, "", 0, False]})
    cases.append({"id": "df-ovf-sub", "proc": "GetAbilDiffText", "args": [2, -2147483647, 5, "", 0, False]})

    scn = {
        "family": "ability_text", "database": True,
        "description": "Ability text (FAM-03 text half, stock): GetAbilityName, GetAbilityList, GetAbilityStats and "
                       "GetAbilDiffText over every stock (ability, value) pair, the variants that change the text, "
                       "and representative values for every ability number.",
        "notes": [
            "Generated by tools/oracle/gen/ability_text.py. The case 'table' carries the stock Spells table (as in the "
            "spells family), the Name column of Spells, Monsters, Items and Classes ([number, name or null]) and the TBInfo "
            "rows ability 148 values point at, so the Rust test needs no database.",
            "GetAbilityStats is always called with lv omitted (Nothing); the ListView rows it would add are UI and not ported.",
            "Grid ids are position-based: 'as43-0007' is the eighth value of the 'as43' grid."],
        "tolerance": {"default": 0},
        "cases": cases, "grids": grids}
    with open(os.path.join(ROOT, "fixtures", "scenarios", "ability_text.json"), "w") as f:
        json.dump(scn, f, separators=(",", ":"))
        f.write("\n")
    print(len(cases), "explicit cases,", len(grids), "grids")


main()

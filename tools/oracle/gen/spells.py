"""Writes fixtures/scenarios/spells.json (FAM-10, spell damage equations) from the stock database.

    python3 -I tools/oracle/gen/spells.py [DB.sqlite]

Deterministic (fixed strides, no randomness). The scenario starts with a "table" case that carries
the whole stock Spells table in a compact form (only the fields the Rust port reads; trailing and
interior empty ability slots dropped), then grids over spell numbers, levels, bonuses, overrides and
optional arguments.

PullSpellEQ's detail text calls GetAbilityStats, GetSpellName and GetMonsterName, which are other
cards' (P3-06/P3-07). The oracle runs MME's own versions; the scenario also calls them directly for
every (ability, value) the text cases need, and the Rust test hands those recorded answers back
to the port. Cases named "as-*", "sn-*" and "mn-*" are those recordings.
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
# Abilities whose GetAbilityStats text depends on the nested PullSpellEQ (and so on bQuickSpell / calc level).
NESTING = (43, 148, 151, 153)
# Abilities PullSpellEQ lists after the damage text through GetAbilityStats without a value to print.
END_TWO = {23, 51, 52, 80, 97, 98, 100, 108, 109, 110, 111, 112, 113, 119, 138, 144, 178}


def load(db):
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    rows = {}
    for r in c.execute("select * from Spells order by Number"):
        ab = [(r["Abil-%d" % i], r["AbilVal-%d" % i]) for i in range(10)]
        rows[r["Number"]] = {
            "f": [r[k] for k in FIELDS], "ab": [(a, v) for a, v in ab if a],
            "learn": r["Learnable"], "lf": 1 if r["Learned From"] else 0}
    return rows


def compact(rs):
    out = []
    for n in sorted(rs):
        r = rs[n]
        flat = [x for p in r["ab"] for x in p]
        out.append(r["f"] + [r["learn"], r["lf"], flat])
    return out


def shape(r):
    f = dict(zip(FIELDS, r["f"]))
    return (f["Cap"] > 0, f["ReqLevel"] > 0, f["MinInc"] != 0 and f["MinIncLVLs"] > 0,
            f["MaxInc"] != 0 and f["MaxIncLVLs"] > 0, f["DurInc"] != 0 and f["DurIncLVLs"] > 0,
            f["EnergyCost"] > 0, r["learn"] or r["lf"])


def special(rs, per_ability=3):
    """Spells that between them show every ability, every row shape and the odd values."""
    pick, seen_ab, seen_shape = set(), {}, set()
    for n in sorted(rs):
        r = rs[n]
        for a, _ in r["ab"]:
            if seen_ab.get(a, 0) < per_ability:
                seen_ab[a] = seen_ab.get(a, 0) + 1
                pick.add(n)
        sh = shape(r)
        if sh not in seen_shape:
            seen_shape.add(sh)
            pick.add(n)
    return pick


def stride(rs, k, off=0):
    return {n for i, n in enumerate(sorted(rs)) if i % k == off}


def main():
    rs = load(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB)
    nums = sorted(rs)
    sp = special(rs, 1)
    gaps = [n for n in range(1, nums[-1] + 1) if n not in rs][:6]
    absent = [0, -1] + gaps + [nums[-1] + 1, 99999]

    A = sorted(sp | stride(rs, 23))                       # lookups and flags
    B = sorted(sp | stride(rs, 40, 3))                   # min/max damage and duration
    C = sorted(n for n in sp if any(a in (1, 8, 17, 18) for a, _ in rs[n]["ab"]))[:90]
    D = sorted(sp | stride(rs, 90, 5))                   # GetCurrentSpellMinMax
    E = sorted(sp | stride(rs, 30, 1))                    # PullSpellEQ, damage-only mode
    F = sorted(sp | stride(rs, 28, 2))                   # PullSpellEQ text
    # Text cases also run the spells an EndCast with no value points at (a range of spell numbers).
    closure = set(F)
    todo = list(F)
    while todo:
        n = todo.pop()
        for a, v in rs[n]["ab"]:
            if a == 151 and v == 0:
                for m in range(rs[n]["f"][3], rs[n]["f"][4] + 1):
                    if m in rs and m not in closure:
                        closure.add(m)
                        todo.append(m)
    F = sorted(closure)

    cases = [{"id": "table", "fields": FIELDS + ["Learnable", "LearnedFrom", "AbilitiesFlat"],
              "spells": compact(rs)}]
    grids = []

    def grid(name, axes, case):
        grids.append({"name": name, "axes": axes, "case": case})

    def one(cid, proc=None, args=None, **kw):
        c = {"id": cid}
        if proc:
            c["proc"] = proc
            c["args"] = args
        c.update(kw)
        cases.append(c)

    # --- flags and lookups
    grid("at", {"n": A + absent}, {"proc": "GetSpellAttackType", "args": ["$n"]})
    grid("area", {"n": A + absent}, {"proc": "SpellIsAreaAttack", "args": ["$n"]})
    grid("bless", {"n": A + absent}, {"proc": "SpellIsBlessSpell", "args": ["$n"]})
    grid("dmg", {"n": A + absent}, {"proc": "SpellDoesDamage", "args": ["$n"]})
    grid("dmgnd", {"n": A}, {"proc": "SpellDoesDamage", "args": ["$n", True]})
    # --- max level
    for i, v in enumerate([0, -5, 100, 255, 255.4, 256, 258.5, 260.4, 271.2, 1000.5, 32767.2, 40000, 2147483647.5, 3e9]):
        one("maxlvl-%02d" % i, "GetMaxLevel", [], globals={"gAvgLevelMaxAllStats": v})
    # --- damage and duration at a cast level
    grid("min", {"n": B + absent[:3], "l": [0, 12, 255]}, {"proc": "GetSpellMinDamage", "args": ["$n", "$l"]})
    grid("max", {"n": B + absent[:3], "l": [0, 12, 255]}, {"proc": "GetSpellMaxDamage", "args": ["$n", "$l"]})
    grid("dur", {"n": B + absent[:3], "l": [0, 12, 255]}, {"proc": "GetSpellDuration", "args": ["$n", "$l"]})
    ref = lambda v: {"ref": "integer", "value": v}
    xs = [[-5, 0, False, False], [30, 0, True, False], [30, 500, False, False], [30, 143, False, True],
          [60, 900, False, False], [32767, 0, False, True], [-30000, 250, True, True], [0, 0, False, True]]
    for proc, tag in (("GetSpellMinDamage", "minx"), ("GetSpellMaxDamage", "maxx")):
        for n in C:
            for j, x in enumerate(xs):
                if (n + j) % 3 == 0 or n in (4, 7, 8):
                    one("%s-%d-%d" % (tag, n, j), proc, [n, x[0], ref(x[1]), x[2], x[3]])
    for n in C[:30]:
        one("durx-%d" % n, "GetSpellDuration", [n, 20, True])
        one("durx0-%d" % n, "GetSpellDuration", [n, 0, True])
    # --- GetCurrentSpellMinMax: the cursor is placed by a lookup first, as MME's callers do
    cur = lambda n, use, lvl, bonus=0, omin=0, omax=0, hdr=False: {
        "steps": [{"call": "GetSpellAttackType", "args": [n]},
                  {"call": "GetCurrentSpellMinMax",
                   "args": [{"ref": "boolean", "value": use}, lvl, {"ref": "boolean", "value": hdr}, omin, omax, bonus],
                   "as": "r"}]}
    for n in D:
        for j, (use, lvl, bonus, omin, omax) in enumerate(
                [(True, 20, 0, 0, 0), (False, 0, 0, 0, 0), (False, 30, 0, 0, 0), (True, 0, 25, 0, 0),
                 (True, 7, 100, 6, 9)]):
            one("cur-%d-%d" % (n, j), **cur(n, use, lvl, bonus, omin, omax))
    for n in D[:40]:
        one("cur-hdr-%d" % n, **cur(n, False, 0, 10, 0, 0, True))
        one("cur-neg-%d" % n, **cur(n, True, -4, 0, 0, 0))
        one("cur-ovmax-%d" % n, **cur(n, False, 0, 0, 0, 12))
    for n in absent[:4]:
        one("cur-absent-%d" % n, **cur(n, True, 20))
    for i, v in enumerate([0, 271.2, 3e9]):
        for n in (4, 7, 13):
            one("cur-maxlvl-%d-%d" % (n, i), globals={"gAvgLevelMaxAllStats": v}, **cur(n, False, 0))

    # --- PullSpellEQ, damage-only mode ("min:max:dur")
    def pull(n, calc, lvl, mm=False, mon=False, pct=False, nested=False, noshow=False, omin=0, omax=0, bonus=0):
        args = [calc, lvl, n, MISSING, mm, mon, pct, nested, noshow, omin, omax, bonus]
        while args and (args[-1] is False or args[-1] == 0):
            args.pop()
        return args

    for n in E:
        for j, (calc, lvl, bonus) in enumerate([(True, 0, 0), (True, 25, 0), (False, 0, 0), (True, 40, 30)]):
            one("mm-%d-%d" % (n, j), "PullSpellEQ", pull(n, calc, lvl, mm=True, bonus=bonus))
    for n in E[::4]:
        one("mmmon-%d" % n, "PullSpellEQ", pull(n, True, 99, mm=True, mon=True))
        one("mmov-%d" % n, "PullSpellEQ", pull(n, True, 10, mm=True, omin=3, omax=8, bonus=20))
        one("mmov0-%d" % n, "PullSpellEQ", pull(n, False, 0, mm=True, omax=8))
    for lvl in (0, 33, 77):
        for txt in ("35", "abc", ""):
            for n in (4, 13, 1):
                one("mmglobal-%d-%d-%s" % (n, lvl, txt or "e"), "PullSpellEQ", pull(n, True, lvl, mm=True),
                    forms={"frmMain": {"chkGlobalFilter": {"Value": 1},
                                       "txtGlobalLevel": {"$array": {"0": {"Text": txt}}}}})
    for n in (4, 13):
        one("mmglobaloff-%d" % n, "PullSpellEQ", pull(n, True, 0, mm=True),
            forms={"frmMain": {"chkGlobalFilter": {"Value": 0},
                               "txtGlobalLevel": {"$array": {"0": {"Text": "35"}}}}})
    for n in absent[:5]:
        one("mmabsent-%d" % n, "PullSpellEQ", pull(n, True, 20, mm=True))
    for n in (4, 7):
        one("mmcursor-%d" % n, steps=[{"call": "GetSpellAttackType", "args": [n]},
                                      {"call": "PullSpellEQ", "args": [True, 20, 0, MISSING, True], "as": "r"}])

    # --- PullSpellEQ text, with the recorded ability / name answers
    keys = set()          # ("as", ability, value, calc, quick) / ("sn", n) / ("mn", n)

    def need(n, calc, quick, omin=0, omax=0):
        r = rs[n]
        lo, hi = (omin, omax) if omin or omax else (r["f"][3], r["f"][4])
        for a, v in r["ab"]:
            if a in (122,):
                keys.add(("sn", v))
            elif a in (137, 164):
                pass
            elif a == 12:
                for m in ([v] if v else range(lo, hi + 1)):
                    keys.add(("mn", m))
            elif a == 151 and v == 0:
                for m in range(lo, hi + 1):
                    keys.add(("sn", m))
            elif a in NESTING:
                for cc in {calc, True}:
                    keys.add(("as", a, v, cc, quick))
            else:
                keys.add(("as", a, v, True, False))
        return r

    text = []
    for n in F:
        for j, (calc, lvl, bonus) in enumerate([(False, 0, 0), (True, 20, 0), (True, 1, 0), (True, 255, 25)]):
            text.append(("txt-%d-%d" % (n, j), n, calc, lvl, dict(bonus=bonus), False))
    for n in F[::3]:
        text.append(("txtn-%d" % n, n, True, 15, dict(nested=True), False))
        text.append(("txtns-%d" % n, n, False, 15, dict(noshow=True, pct=True), False))
        text.append(("txtmon-%d" % n, n, True, 0, dict(mon=True), False))
        text.append(("txtov-%d" % n, n, True, 9, dict(omin=2, omax=5), False))
    for n in F[::2]:
        text.append(("txtq-%d" % n, n, True, 20, dict(), True))
    for cid, n, calc, lvl, kw, quick in text:
        need(n, calc, quick, kw.get("omin", 0), kw.get("omax", 0))
        c = {"id": cid, "proc": "PullSpellEQ",
             "args": pull(n, calc, lvl, **kw)}
        if quick:
            c["globals"] = {"bQuickSpell": True}
        cases.append(c)
    # nesting guards and the cursor form
    one("txt-nest19", "PullSpellEQ", pull(4, True, 20), globals={"nSpellNest": 19})
    one("txt-nest18", "PullSpellEQ", pull(4, True, 20), globals={"nSpellNest": 18})
    one("txt-quicknest", "PullSpellEQ", pull(4, True, 20), globals={"nSpellNest": 1, "bQuickSpell": True})
    one("txt-quicktop", "PullSpellEQ", pull(4, True, 20), globals={"nSpellNest": 0, "bQuickSpell": True})
    for n in absent[:5]:
        one("txtabsent-%d" % n, "PullSpellEQ", pull(n, True, 20))
    for n in (4, 7, 13):
        need(n, True, False)
        one("txtcursor-%d" % n, steps=[{"call": "GetSpellAttackType", "args": [n]},
                                       {"call": "PullSpellEQ", "args": [True, 20, 0, MISSING, False], "as": "r"}])
    # the recordings
    for k in sorted(keys):
        if k[0] == "as":
            _, a, v, calc, quick = k
            c = {"id": "as-%d-%d-%d-%d" % (a, v, calc, quick), "proc": "GetAbilityStats",
                 "args": [a, v, MISSING, calc]}
            if quick:
                c["globals"] = {"bQuickSpell": True, "nSpellNest": 1}
        elif k[0] == "sn":
            c = {"id": "sn-%d" % k[1], "proc": "GetSpellName", "args": [k[1], False]}
        else:
            c = {"id": "mn-%d" % k[1], "proc": "GetMonsterName", "args": [k[1], False]}
        cases.append(c)

    scn = {
        "family": "spells", "database": True,
        "description": "Spell damage equations (FAM-10, stock): spell lookups and flags, damage and duration at a cast level, "
                       "GetCurrentSpellMinMax and PullSpellEQ over the whole stock Spells table.",
        "notes": [
            "Generated by tools/oracle/gen/spells.py. The case 'table' carries every stock Spells row (fields named in 'fields'; "
            "abilities as a flat [ability, value, ...] list without empty slots) so the Rust test needs no database.",
            "Cases 'as-*', 'sn-*' and 'mn-*' record GetAbilityStats, GetSpellName and GetMonsterName (other cards' procedures); "
            "the Rust test feeds them to PullSpellEQ as its text provider. Ability text for abilities other than 43/148/151/153 "
            "does not depend on the calc-level flag or bQuickSpell, so those are recorded once.",
            "PullSpellEQ is always called with lv omitted (Nothing); the ListView rows it would add are UI and not ported."],
        "tolerance": {"default": 0},
        "cases": cases, "grids": grids}
    with open(os.path.join(ROOT, "fixtures", "scenarios", "spells.json"), "w") as f:
        json.dump(scn, f, separators=(",", ":"))
        f.write("\n")
    print(len(sp), len(A), len(B), len(D), len(E), len(F), len(cases), "explicit cases,", sum(len(g["axes"][next(iter(g["axes"]))]) *
                                             (len(g["axes"].get("l", [0])) if "l" in g["axes"] else 1)
                                             for g in grids), "grid cases")


main()

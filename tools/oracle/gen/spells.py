"""Writes fixtures/scenarios/spells.json (FAM-10, spell damage equations) from the stock database.

    python3 -I tools/oracle/gen/spells.py [DB.sqlite]

Deterministic (fixed strides, no randomness). The scenario starts with a "table" case that carries
the whole stock Spells table in a compact form (only the fields the Rust port reads; trailing and
interior empty ability slots dropped), then grids over spell numbers, levels, bonuses, overrides and
optional arguments.

PullSpellEQ's detail text calls GetAbilityStats, GetSpellName and GetMonsterName (ability_text.rs in
the Rust port). The table case also carries the Name columns of Spells, Monsters, Items and Classes
and the TBInfo rows spells' cast-textblock abilities point at, so the Rust test runs the real text
port. The scenario also calls those procedures directly for every (ability, value) the text cases
need; cases named "as-*", "sn-*" and "mn-*" are those calls, and the Rust test checks the port against
them as well.
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


def load_text(db):
    """The name columns and TBInfo rows GetAbilityStats / PullSpellEQ read, so the Rust test can run the real text ports."""
    c = sqlite3.connect(db)
    names = {}
    for key, t in (("spells", "Spells"), ("monsters", "Monsters"), ("items", "Items"), ("classes", "Classes")):
        names[key] = [[r[0], r[1]] for r in c.execute("select Number, Name from %s order by Number" % t)]
    want = set()
    for i in range(10):
        for (v,) in c.execute('select "AbilVal-%d" from Spells where "Abil-%d" = 148' % (i, i)):
            want.add(v)
    tb = [[r[0], r[1] or ""] for r in c.execute("select Number, Action from TBInfo order by Number") if r[0] in want]
    return names, tb


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
    db = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB
    rs = load(db)
    names, tb = load_text(db)
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

    # Invented rows probing integer handling (INT-* in the card). Stock rows stay as they are; these are appended.
    LONG_MAX, LONG_MIN = 2147483647, -2147483648
    syn_rows = []

    def mk(num, abil, req=0, cap=0, mn=0, mx=0, dur=0, mni=0, mnl=0, mxi=0, mxl=0, dri=0, drl=0, en=0):
        syn_rows.append({"Number": num, "ReqLevel": req, "EnergyCost": en, "MinBase": mn, "MaxBase": mx, "Dur": dur,
                         "Cap": cap, "MaxIncLVLs": mxl, "MaxInc": mxi, "MinIncLVLs": mnl, "MinInc": mni,
                         "DurIncLVLs": drl, "DurInc": dri, "ab": abil})
        return num

    S = {}
    # damage at the Long edge, with growth per level (Currency sums beyond Long, Fix, bonus multiply)
    S["big"] = mk(80001, [(1, 0)], mn=LONG_MAX, mx=LONG_MAX, dur=LONG_MAX, mni=LONG_MAX, mnl=1, mxi=LONG_MAX, mxl=1,
                  dri=LONG_MAX, drl=1)
    S["bigcap"] = mk(80002, [(1, 0), (19, 0)], req=1, cap=9999, mn=LONG_MAX, mx=LONG_MAX, dur=LONG_MAX, mni=LONG_MAX,
                     mnl=1, mxi=LONG_MAX, mxl=1, dri=LONG_MAX, drl=1, en=1)
    S["neg"] = mk(80003, [(1, 0)], req=-3, cap=-5, mn=LONG_MIN, mx=-5, dur=-10, mni=LONG_MIN, mnl=1, mxi=-7, mxl=3,
                  dri=-1, drl=2, en=-5)
    S["reqbig"] = mk(80004, [(1, 0)], req=40000, mn=3, mx=9, mni=1, mnl=2)
    S["capbig"] = mk(80005, [(1, 0)], req=2, cap=40000, mn=3, mx=9, mni=1, mnl=2, dri=1, drl=3, dur=4)
    S["req32767"] = mk(80006, [(1, 0)], req=32767, cap=32767, mn=3, mx=9, mni=1, mnl=2)
    S["frac"] = mk(80007, [(1, 0)], req=1, mn=1, mx=2, dur=3, mni=1, mnl=3, mxi=2, mxl=7, dri=1, drl=3, en=3)
    S["fracneg"] = mk(80008, [(1, 0)], req=1, mn=1, mx=2, dur=3, mni=-1, mnl=3, mxi=-2, mxl=7, dri=-1, drl=3, en=500)
    S["lvlneg"] = mk(80009, [(1, 0)], req=1, mn=10, mx=20, dur=30, mni=1, mnl=-3, mxi=5, mxl=-7, dri=2, drl=-3, en=501)
    S["nolvl"] = mk(80010, [(1, 0)], mn=7, mx=700, dur=5, en=1)
    S["rnd"] = mk(80011, [(1, 0)], req=1, mn=0, mx=0, mni=1, mnl=2, mxi=1, mxl=2, dri=1, drl=2)    # .5 per level: CLng ties
    # EndCast (151, no value) and Summon (12, no value) ranges at the edges of Long
    S["end_edge"] = mk(80020, [(151, 0)], mn=LONG_MAX - 1, mx=LONG_MAX)
    S["end_neg"] = mk(80021, [(151, 0)], mn=LONG_MIN, mx=LONG_MIN + 1)
    S["end_eq"] = mk(80022, [(151, 0)], mn=LONG_MAX, mx=LONG_MAX)
    S["end_grow"] = mk(80023, [(151, 0)], req=1, mn=LONG_MAX - 1, mx=LONG_MAX - 1, mni=1, mnl=1, mxi=1, mxl=1)
    S["end_growmax"] = mk(80024, [(151, 0)], req=1, mn=LONG_MAX - 1, mx=LONG_MAX, mxi=1, mxl=1)
    S["end_maxlow"] = mk(80025, [(151, 0)], mn=5, mx=LONG_MIN)
    S["end_small"] = mk(80026, [(151, 0)], mn=4, mx=6)
    S["end_half"] = mk(80027, [(151, 0)], req=1, mn=10, mx=10, mni=1, mnl=2, mxi=1, mxl=2)
    S["sum_edge"] = mk(80030, [(12, 0)], mn=LONG_MAX - 1, mx=LONG_MAX)
    S["sum_neg"] = mk(80031, [(12, 0)], mn=LONG_MIN, mx=LONG_MIN + 1)
    S["sum_eq"] = mk(80032, [(12, 0)], mn=LONG_MAX, mx=LONG_MAX)
    S["sum_grow"] = mk(80033, [(12, 0)], req=1, mn=LONG_MAX - 1, mx=LONG_MAX - 1, mni=1, mnl=1, mxi=1, mxl=1)
    S["sum_growmax"] = mk(80034, [(12, 0)], req=1, mn=LONG_MAX - 1, mx=LONG_MAX, mxi=1, mxl=1)
    S["sum_maxlow"] = mk(80035, [(12, 0)], mn=5, mx=LONG_MIN)
    S["sum_small"] = mk(80036, [(12, 0)], mn=4, mx=6)
    # DR (7) divides the Currency by 10; Case Else abilities print the strings
    S["dr_big"] = mk(80040, [(7, 0)], req=1, mn=LONG_MAX, mx=LONG_MAX, mni=LONG_MAX, mnl=1, mxi=7, mxl=3)
    S["dr_small"] = mk(80041, [(7, 0)], req=1, mn=-15, mx=33, mni=7, mnl=3, mxi=1, mxl=3)
    S["dr_nolvl"] = mk(80042, [(7, 0)], mn=-15, mx=33)
    S["other"] = mk(80043, [(23, 0), (50, 0), (140, 0), (148, 0)], req=1, mn=LONG_MAX, mx=LONG_MIN, mni=3, mnl=2)
    # ability numbers and values outside Integer (GetAbilityStats takes Integer, error 6 inside PullSpellEQ)
    S["abv_hi"] = mk(80050, [(1, 40000)], mn=1, mx=2)
    S["abv_lo"] = mk(80051, [(1, -40000)], mn=1, mx=2)
    S["abv_edge"] = mk(80052, [(1, 32767), (1, -32768)], mn=1, mx=2)
    S["abv_164"] = mk(80053, [(164, 40000), (151, 70000)], mn=1, mx=2)
    S["abv_164ok"] = mk(80054, [(164, 40000), (1, 5)], mn=1, mx=2)
    S["abv_122"] = mk(80055, [(122, LONG_MAX), (122, LONG_MIN)], mn=1, mx=2)
    S["abv_12"] = mk(80056, [(12, LONG_MAX)], mn=1, mx=2)
    S["abv_end2"] = mk(80057, [(23, 40000)], mn=1, mx=2)
    S["abv_148"] = mk(80058, [(148, 40000)], mn=1, mx=2)
    S["abn_hi"] = mk(80059, [(40000, 0)], mn=1, mx=2)
    S["abn_neg"] = mk(80060, [(-1, 0), (1, 0)], mn=1, mx=2)
    S["abn_hi_mm"] = mk(80061, [(1, 0), (40000, 7)], mn=1, mx=2)      # only reached without min-max mode
    S["abn_first"] = mk(80062, [(40000, 7), (1, 0)], mn=1, mx=2)      # min-max mode stops at the first slot
    # energy costs for the "x times/round" suffix
    for k, en in enumerate((1, 3, 143, 499, 500, 501, 0, -5, LONG_MAX)):
        S["en%d" % en] = mk(80070 + k, [(1, 0)], mn=1, mx=2, en=en)
    for r in syn_rows:
        rs[r["Number"]] = {"f": [r[k] if k in r else 0 for k in FIELDS], "ab": r["ab"], "learn": 0, "lf": 0}
        rs[r["Number"]]["f"][0] = r["Number"]
    db_extra = []
    for r in syn_rows:
        row = {"Number": r["Number"], "Name": "Syn %d" % r["Number"]}
        row.update({k: r[k] for k in FIELDS if k in r and k != "Number"})
        for i, (a_, v_) in enumerate(r["ab"]):
            row["Abil-%d" % i], row["AbilVal-%d" % i] = a_, v_
        db_extra.append(row)
    names["spells"] += [[r["Number"], "Syn %d" % r["Number"]] for r in syn_rows]

    cases = [{"id": "table", "fields": FIELDS + ["Learnable", "LearnedFrom", "AbilitiesFlat"],
              "spells": compact(rs), "names": names, "textblocks": tb}]
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

    # --- integer handling on the invented rows
    syn_nums = [r["Number"] for r in syn_rows]
    for n in syn_nums:
        for j, (calc, lvl, bonus) in enumerate([(True, 0, 0), (True, 1, 0), (True, 255, 0), (True, 9999, 0),
                                                (False, 0, 0), (False, 9999, 32767), (True, 255, 32767), (True, -7, 0)]):
            one("iq-%d-%d" % (n, j), "PullSpellEQ", pull(n, calc, lvl, bonus=bonus))
        for j, (calc, lvl, bonus) in enumerate([(True, 255, 0), (False, 0, 0), (True, 9999, 25)]):
            one("iqm-%d-%d" % (n, j), "PullSpellEQ", pull(n, calc, lvl, mm=True, bonus=bonus))
        if n in syn_nums[::3]:
            one("iqf-%d" % n, "PullSpellEQ", pull(n, True, 40, mon=True))
            one("iqn-%d" % n, "PullSpellEQ", pull(n, True, 20, nested=True, noshow=True))
        one("iqq-%d" % n, "PullSpellEQ", pull(n, True, 255), globals={"bQuickSpell": True})
        for j, (omin, omax) in enumerate([(LONG_MAX, LONG_MAX), (LONG_MIN, LONG_MIN), (LONG_MAX - 1, LONG_MAX), (5, 0)]):
            one("iqo-%d-%d" % (n, j), "PullSpellEQ", pull(n, True, 255, omin=omin, omax=omax, bonus=25 * (j % 2)))
        one("iqo-%d-m" % n, "PullSpellEQ", pull(n, True, 255, mm=True, omin=LONG_MIN, omax=LONG_MAX))
    # max level taken from gAvgLevelMaxAllStats when no level is given (Integer assignment of a Long)
    for n in (S["big"], S["frac"], S["rnd"], S["end_grow"], S["dr_big"], S["nolvl"]):
        for i, v in enumerate([0, 100, 271.2, 32767.4, 32768, 40000, 2147483645, 2147483646, 3e9]):
            one("iqg-%d-%d" % (n, i), "PullSpellEQ", pull(n, False, 0), globals={"gAvgLevelMaxAllStats": v})
            one("iqgm-%d-%d" % (n, i), "PullSpellEQ", pull(n, False, 0, mm=True), globals={"gAvgLevelMaxAllStats": v})
    # the global level box: Val() then an Integer assignment (banker's rounding, error 6 beyond Integer)
    for n in (S["frac"], S["bigcap"], S["capbig"], S["nolvl"]):
        for i, txt in enumerate(["40000", "-5", "2.5", "3.5", "32767.4", "32767.5", "-32768.5", "-32769", "1e3", "1e9",
                                 " 12abc", "&H7FFF", "&HFFFF", "0", "256"]):
            one("iqt-%d-%d" % (n, i), "PullSpellEQ", pull(n, True, 0),
                forms={"frmMain": {"chkGlobalFilter": {"Value": 1},
                                   "txtGlobalLevel": {"$array": {"0": {"Text": txt}}}}})
    # arguments outside Integer fail at the call (error 6), before PullSpellEQ runs
    for i, (lvl, bonus) in enumerate([(40000, 0), (-32769, 0), (32767, 0), (-32768, 0), (5, 32768), (5, -32769), (5, 32767)]):
        for n in (S["frac"], S["big"]):
            one("iqa-%d-%d" % (n, i), "PullSpellEQ", pull(n, True, lvl, bonus=bonus))
    # the other level-scaled procedures at the same edges
    for n in syn_nums[:14]:
        for l in (0, 255, 9999):
            one("ig-min-%d-%d" % (n, l), "GetSpellMinDamage", [n, l])
            one("ig-max-%d-%d" % (n, l), "GetSpellMaxDamage", [n, l])
            one("ig-dur-%d-%d" % (n, l), "GetSpellDuration", [n, l])
        for i, (use, lvl, bonus) in enumerate([(True, 9999, 32767), (False, 0, 25)]):
            one("ig-cur-%d-%d" % (n, i), **cur(n, use, lvl, bonus))
    for l in (40000, -32769):
        one("ig-min-lvl-%d" % l, "GetSpellMinDamage", [S["frac"], l])

    scn = {
        "family": "spells", "database": True,
        "description": "Spell damage equations (FAM-10, stock): spell lookups and flags, damage and duration at a cast level, "
                       "GetCurrentSpellMinMax and PullSpellEQ over the whole stock Spells table.",
        "notes": [
            "Generated by tools/oracle/gen/spells.py. The case 'table' carries every stock Spells row (fields named in 'fields'; "
            "abilities as a flat [ability, value, ...] list without empty slots) so the Rust test needs no database.",
            "Cases 'as-*', 'sn-*' and 'mn-*' call GetAbilityStats, GetSpellName and GetMonsterName directly; "
            "the Rust test compares ability_text's answers with them and runs PullSpellEQ over the real text port. Ability text for abilities other than 43/148/151/153 "
            "does not depend on the calc-level flag or bQuickSpell, so those are recorded once.",
            "PullSpellEQ is always called with lv omitted (Nothing); the ListView rows it would add are UI and not ported."],
        "tolerance": {"default": 0},
        "cases": cases, "grids": grids,
        "db_extra": {"Spells": db_extra}}
    with open(os.path.join(ROOT, "fixtures", "scenarios", "spells.json"), "w") as f:
        json.dump(scn, f, separators=(",", ":"))
        f.write("\n")
    print(len(sp), len(A), len(B), len(D), len(E), len(F), len(cases), "explicit cases,", sum(len(g["axes"][next(iter(g["axes"]))]) *
                                             (len(g["axes"].get("l", [0])) if "l" in g["axes"] else 1)
                                             for g in grids), "grid cases")


main()

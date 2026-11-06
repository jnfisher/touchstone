#!/usr/bin/env python3
"""Writes fixtures/scenarios/lairs.json (FAM-11 statistics half, card P3-15).

    python3 -I tools/oracle/gen/lairs.py [DB.sqlite]

Deterministic: selection and invented numbers come from sha256 of fixed tags and a splitmix
counter, never the clock or Python's `random`.

What the cases are:
- `real-*`: "Summoned By" strings of stock monsters that have lairs, verbatim (deduplicated; the long
  ones cut after a whole entry), run through CountLairsByMaxRegen and GetLairAveragesFromLocs.
- `hand-*`: strings and lair tables written to reach one branch each.
- `calc-*`: CalculateAverageLairs over monster rows (real and invented).
- `sort-*`, `med-*`, `mad-*`, `sd-*`, `out-*`, `mode-*`: numeric grids for the helpers.

GetLairInfo is P3-16's: the oracle answers it (and GetDamageOutput) from the table below, so the
numbers per lair group are invented but fixed. They sit once under the scenario's "data" key.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "lairs.json")
PAT = re.compile(r"\[([\d\-]+)\]\[(\d+)\]Group\(lair\): (\d+)/(\d+)", re.A)
MASK = 2**64 - 1


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


class Rng:
    def __init__(self, seed):
        self.s = seed & MASK

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def pick(self, xs):
        return xs[self.next() % len(xs)]

    def between(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)


def lair_row(gi):
    """Invented GetLairInfo answer for a group index (zero fields left out)."""
    def h(tag, m):
        return H(gi, tag) % m
    nm = 1 + h("n", 5)
    mobs = nm + [0, 0.25, 0.5, 0.75][h("f", 4)]
    if h("zero", 12) == 0:
        mobs = 0
    nmob = 1 + h("ml", 4)
    mob_list = ",".join(str(1 + h("mob%d" % k, 1100)) for k in range(nmob))
    if h("dup", 5) == 0:
        mob_list += "," + mob_list.split(",")[0]
    walk = 0 if h("w0", 7) == 0 else (300.0 if h("wo", 17) == 0 else 3 + h("w", 40) + [0, 0.5, 0.25][h("wf", 3)])
    maj = [0, 0, 85, 90, 100][h("am", 5)]
    magic = [0, 0, 0, 1, 2, 3][h("ml2", 6)]
    immu = [0, 0, 0, 1, 2][h("im", 5)]
    top = max(int(mobs), 1)
    r = {
        "sMobList": mob_list, "nMobs": mobs,
        "nAvgExp": 100 * (1 + h("xp", 400)) + [0, 0.5, 0.25][h("xf", 3)],
        "nAvgDmg": h("dm", 60) + [0, 0.25, 0.5][h("df", 3)],
        "nAccyMajority": maj, "nAccyMax": maj + (h("ax", 3) * 5 if maj else h("ax", 40)),
        "nAvgHP": 50 + h("hp", 2000), "nAvgAC": h("ac", 70), "nAvgDR": h("dr", 45), "nAvgMR": h("mr", 101),
        "nAvgRCOL": h("rc", 60) if h("rcb", 3) == 0 else 0, "nAvgRFIR": h("rf", 60) if h("rfb", 3) == 0 else 0,
        "nAvgRSTO": h("rs", 60) if h("rsb", 3) == 0 else 0, "nAvgRLIT": h("rl", 60) if h("rlb", 3) == 0 else 0,
        "nAvgRWAT": h("rw", 60) if h("rwb", 3) == 0 else 0,
        "nAvgDodge": h("dg", 90), "nAvgBSDefense": h("bs", 101), "nAvgWalk": walk,
        "nAvgDelay": h("dl", 12) + [0, 0.5][h("dlf", 2)],
        "nDamageMitigated": h("mit", 40), "nDamageOut": h("do", 400), "nFirstRoundDamageOut": h("fr", 400),
        "nMinRoundDamageOut": h("mn", 200), "nSurpriseDamageOut": h("sd", 500), "nSurpriseMinDamageOut": h("sm", 200),
        "nSurpriseChance": h("sc", 100), "nAvgDmgLair": h("dlr", 50) + [0, 0.5][h("dlf2", 2)],
        "nAvgDmgClearRound": h("dcr", 50) + [0, 0.25][h("dcf", 2)],
        "nAvgDmgClear": 0 if h("dc0", 4) == 0 else 20 + h("dc", 3000) + [0, 0.5][h("dcf2", 2)],
        "nMagicLVL": magic, "nMaxMagicLVL": magic + h("mm", 3) if magic else 0,
        "nSpellImmuLVL": immu, "nMaxSpellImmuLVL": immu + h("mi", 3) if immu else 0,
        "nNumUndeads": h("un", top + 1) if h("unb", 3) == 0 else 0,
        "nNumAntiMagic": h("aa", top + 1) if h("aab", 6) == 0 else 0,
        "nNumAnimals": h("an", top + 1) if h("anb", 3) == 0 else 0,
        "nNumLiving": h("lv", top + 1) if h("lvb", 2) == 0 else 0,
    }
    return {k: v for k, v in r.items() if v != 0}


def cut(s, limit):
    """The first whole comma-separated entries that fit in limit characters."""
    if len(s) <= limit:
        return s
    return s[:limit].rsplit(",", 1)[0]


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)
    mons = c.execute('select Number, "Summoned By" from Monsters where "Summoned By" like \'%(lair)%\' order by Number').fetchall()
    by_str = {}
    for n, s in mons:
        by_str.setdefault(s, []).append(n)
    short = sorted((s for s in by_str if len(s) <= 1500), key=lambda s: H("s", s))
    long_ = sorted((s for s in by_str if len(s) > 1500), key=lambda s: H("l", s))
    real_avg = short[:70] + [cut(s, 3500) for s in long_[:6]]
    real_cnt = short[:60] + [cut(s, 3500) for s in long_[:6]]

    cases = []

    def add(cid, **kw):
        d = {"id": cid}
        d.update(kw)
        cases.append(d)

    # ---- shared lair table: most groups of the chosen strings; the rest answer "no such lair"
    gis = sorted({g for s in real_avg for g, *_ in PAT.findall(s)})
    table = {g: lair_row(g) for g in gis if H("tab", g) % 10 < 8}

    # ---- CountLairsByMaxRegen
    ranges = [(0, 100000), (1, 1), (2, 5), (6, 6), (1, 3), (10, 99), (0, 0), (3, 2)]
    for i, s in enumerate(real_cnt):
        lo, hi = ranges[H("r", s) % len(ranges)]
        add("cnt-real-%d" % i, globals={"nNMRVer": 1.83}, proc="CountLairsByMaxRegen", args=[s, lo, hi])
    mixed = "Group: 1/10,[6]Group(lair): 1/2345,[7-8-9][6]Group(lair): 2/9,[1-2-3][0]Group(lair): 1/1,[4-5-6][12]Group(lair): 3/4,x[2]Group(lair): 1/1"
    for ver in (1.0, 1.8, 1.81, 1.82, 1.83, 1.84, 2.0):
        for j, (lo, hi) in enumerate([(0, 1000), (6, 6), (7, 12), (0.5, 6.5), (12, 6), (-5, 1)]):
            add("cnt-ver-%s-%d" % (ver, j), globals={"nNMRVer": ver}, proc="CountLairsByMaxRegen", args=[mixed, lo, hi])
    for j, s in enumerate(["", "Group: 1/10", "[1-2][3]Group(lair): 1/2", "[][3]Group(lair): 1/2", "[1-2][]Group(lair): 1/2",
                           "[1-2][3]Group(lair): 1/", "[1-2][3]group(lair): 1/2", "[a][3]Group(lair): 1/2",
                           "[1-2][3]Group(lair):  1/2", "[1-2][3]Group(lair): 1/2[4-5][6]Group(lair): 7/8",
                           "[1-2][99999999999]Group(lair): 1/2", "[-][5]Group(lair): 1/2", "[1-2][007]Group(lair): 1/2",
                           "[1-2][3][4]Group(lair): 1/2"]):
        for ver in (1.82, 1.83):
            add("cnt-edge-%d-%s" % (j, ver), globals={"nNMRVer": ver}, proc="CountLairsByMaxRegen", args=[s, 0, 1e12])

    # ---- GetLairAveragesFromLocs
    def avg(cid, loc, lairs=None, dmg=None, ver=1.83, summoned="$loc", cfg="cfg-a"):
        env = {"summoned_by": summoned}
        if lairs:
            env["lairs"] = lairs
        if dmg:
            env["damage_out"] = dmg
        add(cid, globals={"nNMRVer": ver, "sGlobalAttackConfig": cfg}, lair_env=env,
            proc="GetLairAveragesFromLocs", args=[loc])

    for i, s in enumerate(real_avg):
        avg("avg-real-%d" % i, s, dmg=[None, {"nAverageDamage": 55.5, "nSurpriseDamage": 20, "nSurpriseDamageChance": 10}][i % 2])

    def L(**kw):
        base = {"sMobList": "1", "nMobs": 1, "nAvgExp": 100, "nAvgHP": 50}
        base.update(kw)
        return base

    def loc(*pairs):
        return ",".join("[%s][%d]Group(lair): 1/%d" % (g, r, 100 + i) for i, (g, r) in enumerate(pairs))

    one = loc(("1-1-1", 5))
    two = loc(("1-1-1", 5), ("2-2-2", 3))
    three = loc(("1-1-1", 5), ("2-2-2", 3), ("3-3-3", 8))
    avg("hand-single", one, {"1-1-1": L()})
    avg("hand-two-ties", two, {"1-1-1": L(nAvgDmg=1, nAvgAC=1, nAvgDR=2, nAvgMR=3, nAvgHP=7, nAvgExp=1, nMobs=2, nAvgDodge=1),
                               "2-2-2": L(nAvgDmg=2, nAvgAC=2, nAvgDR=3, nAvgMR=4, nAvgHP=8, nAvgExp=2, nMobs=3, nAvgDodge=2)})
    avg("hand-two-ties2", two, {"1-1-1": L(nAvgDmg=2, nAvgAC=2, nAvgDR=2, nAvgHP=2, nAvgExp=3, nMobs=1.5, nAvgDelay=2.25),
                                "2-2-2": L(nAvgDmg=3, nAvgAC=3, nAvgDR=3, nAvgHP=3, nAvgExp=4, nMobs=1.5, nAvgDelay=2.5)})
    avg("hand-skip-nomobs", three, {"1-1-1": L(nMobs=0), "2-2-2": L(nAvgWalk=10)})   # third group unknown
    avg("hand-all-skipped", two, {"1-1-1": L(nMobs=0)})
    avg("hand-zero-regen", loc(("1-1-1", 0), ("2-2-2", 4)), {"1-1-1": L(), "2-2-2": L(nAvgWalk=7)})
    avg("hand-only-zero-regen", loc(("1-1-1", 0)), {"1-1-1": L()})
    avg("hand-no-lairs", "Group: 1/5,Group: 1/6", {})
    avg("hand-empty", "", {}, summoned="")
    avg("hand-summoned-differs", one, {"1-1-1": L()}, summoned="Group: 1/1,Group: 1/2,[1-1-1][5]Group(lair): 1/3")
    for ver in (1.0, 1.82, 1.829):
        avg("hand-old-ver-%s" % ver, one, {"1-1-1": L()}, ver=ver, summoned="Group: 1/1,Group: 1/2")
    # accuracy majority across lairs (threshold 51 percent, ties go to the higher value, zeros do not vote)
    for name, votes in {"3of4": [80, 80, 80, 90], "2of4": [80, 80, 90, 90], "2of3": [80, 80, 90], "tie2": [80, 90],
                        "none": [0, 0, 0], "one": [95], "zeros": [0, 70, 0], "three-way": [70, 80, 90, 90, 80, 70],
                        "51": [70] * 3 + [80] * 2}.items():
        gs = ["%d-0-0" % k for k in range(len(votes))]
        avg("hand-accy-" + name, loc(*[(g, 2) for g in gs]),
            {g: L(nAccyMajority=v, nAccyMax=v + k) for k, (g, v) in enumerate(zip(gs, votes))})
    # magic / spell-immunity levels: mode with ties to the higher level
    for name, lv in {"tie": [1, 2], "modeTie": [0, 0, 1, 1, 3], "single": [3], "mostly0": [0, 0, 0, 2], "all0": [0, 0]}.items():
        gs = ["%d-0-1" % k for k in range(len(lv))]
        avg("hand-magic-" + name, loc(*[(g, 1) for g in gs]),
            {g: L(nMagicLVL=v, nMaxMagicLVL=v + 1, nSpellImmuLVL=3 - v, nMaxSpellImmuLVL=4 - v) for g, v in zip(gs, lv)},
            dmg={"nAverageDamage": 10, "nSurpriseDamage": 5})
    # defence flags and the immune markers
    flag_rows = {
        "anti-half": L(nMobs=4, nNumAntiMagic=2), "anti-under": L(nMobs=4, nNumAntiMagic=1),
        "anti-frac": L(nMobs=2.5, nNumAntiMagic=2), "anti-frac2": L(nMobs=2.5, nNumAntiMagic=1),
        "undead-90": L(nMobs=10, nNumUndeads=9), "undead-89": L(nMobs=10, nNumUndeads=8),
        "living-all": L(nMobs=3, nNumLiving=3), "living-some": L(nMobs=3, nNumLiving=2),
        "animal-90": L(nMobs=10, nNumAnimals=9), "animal-low": L(nMobs=1.5, nNumAnimals=1),
        "all-flags": L(nMobs=10, nNumAnimals=10, nNumLiving=10, nNumUndeads=10, nNumAntiMagic=10),
        "magic-only": L(nMagicLVL=2, nAvgAC=11, nAvgDR=12, nAvgMR=13, nAvgDodge=14),
        "immu-only": L(nSpellImmuLVL=1), "plain": L(),
    }
    for name, r in flag_rows.items():
        r = dict(r, nDamageOut=77, nFirstRoundDamageOut=66, nMinRoundDamageOut=55, nSurpriseDamageOut=44,
                 nSurpriseMinDamageOut=33, nSurpriseChance=22)
        avg("hand-flag-" + name, one, {"1-1-1": r}, dmg={"nAverageDamage": 9, "nSurpriseDamage": 8})
    for name, dmg in {"imm-avg": {"nAverageDamage": -9998, "nSurpriseDamage": 5},
                      "imm-sur": {"nAverageDamage": 5, "nSurpriseDamage": -9998},
                      "imm-both": {"nAverageDamage": -9998, "nSurpriseDamage": -9998},
                      "none": {"nAverageDamage": -9999, "nSurpriseDamage": -9999},
                      "ok": {"nAverageDamage": 3, "nSurpriseDamage": 3}}.items():
        r = L(nMagicLVL=1, nDamageOut=77, nFirstRoundDamageOut=66, nMinRoundDamageOut=55, nSurpriseDamageOut=44,
              nSurpriseMinDamageOut=33, nSurpriseChance=22)
        avg("hand-dmg-" + name, two, {"1-1-1": r, "2-2-2": r}, dmg=dmg)
    # mob lists
    for name, lists in {"dedupe": ["5,6", "6,7", "5"], "single": ["5"], "empty": ["", ""], "spaces": [" 5, 6", "6 ,7"],
                        "blank-item": ["5,,6", " "], "nonnum": ["abc", "5"], "all-same": ["9,9", "9"],
                        "decimal": ["1.5", "2"], "one-then-empty": ["12", ""]}.items():
        gs = ["%d-9-9" % k for k in range(len(lists))]
        avg("hand-mobs-" + name, loc(*[(g, 1) for g in gs]), {g: L(sMobList=m) for g, m in zip(gs, lists)})
    # walk-time outlier removal inside the average
    walks = [10, 11, 12, 13, 200]
    gs = ["%d-4-4" % k for k in range(len(walks))]
    avg("hand-walk-outlier", loc(*[(g, 1) for g in gs]), {g: L(nAvgWalk=w) for g, w in zip(gs, walks)})
    avg("hand-walk-zeros", loc(*[(g, 1) for g in gs]), {g: L(nAvgWalk=w) for g, w in zip(gs, [0, 0, 8, 0, 9])})
    avg("hand-walk-equal", loc(*[(g, 1) for g in gs[:3]]), {g: L(nAvgWalk=6.5) for g in gs[:3]})
    # clear damage average only counts lairs that can be cleared
    avg("hand-clear", three, {"1-1-1": L(nAvgDmgClear=100, nAvgDmgClearRound=4), "2-2-2": L(nAvgDmgClear=0, nAvgDmgClearRound=6),
                              "3-3-3": L(nAvgDmgClear=250.5, nAvgDmgClearRound=7.5)})
    # run-time errors: partial results
    avg("hand-regen-over-integer", loc(("1-1-1", 40000), ("2-2-2", 3)), {"1-1-1": L(), "2-2-2": L()})
    avg("hand-regen-huge", loc(("1-1-1", 5)).replace("[5]", "[99999999999999999999]"), {"1-1-1": L()})
    avg("hand-exp-overflow", two, {"1-1-1": L(nAvgExp=900000000000000), "2-2-2": L(nAvgExp=900000000000000)})
    avg("hand-exp-times-regen-overflow", loc(("1-1-1", 32000)), {"1-1-1": L(nAvgExp=900000000000000)})
    avg("hand-ac-integer", two, {"1-1-1": L(nAvgAC=32767), "2-2-2": L(nAvgAC=32767)})
    avg("hand-poss-spawns", one + ",Group: 1/1,Group: 1/2,group: 1/3", {"1-1-1": L()},
        summoned="Group: 1/1,Group: 1/2,group: 1/3,Group:Group:")
    avg("hand-big-regen", loc(("1-1-1", 32767), ("2-2-2", 1)), {"1-1-1": L(nAvgExp=1000.5, nAvgHP=100000), "2-2-2": L()})
    avg("hand-fraction-mobs", three, {"1-1-1": L(nMobs=0.3333), "2-2-2": L(nMobs=0.3333), "3-3-3": L(nMobs=0.3334)})
    avg("hand-attack-config-empty", one, {"1-1-1": L()}, cfg="")
    rng = Rng(1503)
    for k in range(30):                               # random mixes of invented groups
        n = rng.between(1, 9)
        pairs = [("%d-%d-%d" % (rng.between(1, 6), rng.between(0, 3), rng.between(0, 5)), rng.pick([0, 1, 1, 2, 3, 5, 8, 20])) for _ in range(n)]
        gs = sorted({g for g, _ in pairs})
        rows = {g: lair_row("r%d:%s" % (k, g)) for g in gs if rng.next() % 9}
        avg("hand-rand-%d" % k, loc(*pairs), rows, dmg=rng.pick([None, {"nAverageDamage": 12, "nSurpriseDamage": 7},
                                                                  {"nAverageDamage": -9998, "nSurpriseDamage": 7}]))

    # ---- CalculateAverageLairs
    def calc(cid, rows, ver=1.83, maxmon=None, read=True):
        top = max([r["Number"] for r in rows] + [0]) if maxmon is None else maxmon
        zeros = [0] * (top + 1)
        add(cid, globals={"nNMRVer": ver, "nMonsterPossy": zeros, "nMonsterSpawnChance": zeros},
            lair_env={"monsters": rows}, steps=[{"call": "CalculateAverageLairs"}],
            read={"possy": "%g.nMonsterPossy", "chance": "%g.nMonsterSpawnChance"})

    real_rows = [{"Number": by_str[s][0], "Summoned By": s} for s in short[:35]]
    real_rows.sort(key=lambda r: r["Number"])
    calc("calc-real", real_rows)
    hand_rows = [
        {"Number": 1, "Summoned By": two}, {"Number": 2, "Summoned By": "Group: 1/5"},
        {"Number": 3, "Summoned By": "[1-1-1][0]Group(lair): 1/5"}, {"Number": 4, "Summoned By": one + ",Group(lair): 1/9"},
        {"Number": 5, "Summoned By": loc(("9-9-9", 1))}, {"Number": 6, "Summoned By": loc(("1-1-1", 2), ("1-1-1", 2), ("2-2-2", 7))},
        {"Number": 7, "Summoned By": "[6]Group(lair): 1/2345,[7]Group(lair): 1/1"}, {"Number": 8, "Summoned By": "GROUP(LAIR): 1/2"},
        {"Number": 9, "Summoned By": "Group(lair): 1/3,Group(lair): 2/4,Group(lair): 1/5"},
        {"Number": 10, "Summoned By": loc(("1-1-1", 32767))}, {"Number": 11, "Summoned By": loc(("2-2-2", 1))},
    ]
    shared = {"1-1-1": L(nMobs=3), "2-2-2": L(nMobs=1), "9-9-9": L(nMobs=0), }
    for ver in (1.0, 1.7, 1.8, 1.81, 1.82, 1.83, 1.9):
        calc("calc-hand-%s" % ver, hand_rows, ver)
    calc("calc-small-array", hand_rows, 1.83, maxmon=5)
    calc("calc-empty", [])
    calc("calc-nolair", [{"Number": 3, "Summoned By": "Group: 1/5"}])
    table.update({k: v for k, v in shared.items()})
    for j in range(1, 8):
        table["%d-0-0" % j] = L(nMobs=j / 2.0)
    for j, (m, r) in enumerate([(3, 5), (7, 11), (1, 1), (0.5, 40), (2.25, 9), (8, 100), (0.0001, 3)]):
        table["1%d-7-7" % j] = L(nMobs=m)
    add("calc-chance", globals={"nNMRVer": 1.83, "nMonsterPossy": [0] * 8, "nMonsterSpawnChance": [0] * 8},
        lair_env={"monsters": [{"Number": j + 1, "Summoned By": loc(("1%d-7-7" % j, r))} for j, (m, r) in enumerate(
            [(3, 5), (7, 11), (1, 1), (0.5, 40), (2.25, 9), (8, 100), (0.0001, 3)])]},
        steps=[{"call": "CalculateAverageLairs"}], read={"possy": "%g.nMonsterPossy", "chance": "%g.nMonsterSpawnChance"})

    # ---- Dictionary helpers
    for name, items in {"empty": [], "one": [[3, 1]], "tie": [[1, 2], [5, 2]], "tie-rev": [[5, 2], [1, 2]],
                        "max": [[0, 1], [2, 9], [3, 4]], "neg": [[-1, 3], [-4, 3]], "late": [[1, 1], [2, 1], [3, 5], [4, 5]]}.items():
        add("mode-" + name, proc="DICT_ModeFromCounts", args=[{"dict": items}, 0], )
        add("mode-def-" + name, proc="DICT_ModeFromCounts", args=[{"dict": items}, 42])
        add("bump-new-" + name, proc="DICT_BumpCount", args=[{"dict": items}, 7])
        if items:
            add("bump-old-" + name, proc="DICT_BumpCount", args=[{"dict": items}, items[0][0]])
        add("add-new-" + name, proc="DICT_AddToCount", args=[{"dict": items}, 9, 3])
        add("add-zero-" + name, proc="DICT_AddToCount", args=[{"dict": items}, 9, 0])
        if items:
            add("add-old-" + name, proc="DICT_AddToCount", args=[{"dict": items}, items[-1][0], -2])

    # ---- numeric helpers
    rng = Rng(77)
    fixed = [[], [5], [2, 1], [1, 1], [3, 1, 2], [4, 4, 4, 4], [1, 2, 3, 4], [4, 3, 2, 1], [5, 1, 5, 1, 5], [0, 0, 8, 0, 9],
             [10, 11, 12, 13, 200], [1, 2, 3, 4, 5, 6, 7, 8, 9, 100], [-3, -1, -2, 0, 2, 1, 3], [0.5, 0.25, 0.125],
             [7, 7, 7, 7, 7, 7, 8], [1e9, -1e9, 0], [3, 3], [0, 0], [0, 10], [1, 100], [2, 2, 2, 50]]
    arrays = [list(map(float, a)) for a in fixed]
    for k in range(40):
        n = rng.between(1, 30)
        span = rng.pick([3, 10, 100, 1000])
        a = [rng.between(0, span) / rng.pick([1, 1, 2, 4]) for _ in range(n)]
        if k % 5 == 0:
            a[rng.between(0, n - 1)] = float(rng.between(300, 900))
        if k % 7 == 0:
            a = [x if rng.next() % 3 else 0.0 for x in a]
        arrays.append([float(x) for x in a])
    arrays.append([float(i % 5) for i in range(200)])
    arrays.append([float(200 - i) for i in range(200)])
    for i, a in enumerate(arrays):
        arg = {"ref": "array:double", "values": a}
        add("sort-%d" % i, proc="QuickSort", args=[arg, 0, len(a) - 1])
        add("med-%d" % i, proc="GetMedian", args=[arg])
        add("mad-%d" % i, proc="GetMedianAbsDev", args=[arg, float(rng.between(0, 20)) / 2])
        add("sd-%d" % i, proc="GetStdDev", args=[arg])
        add("out-%d" % i, proc="RemoveOutliers", args=[arg])
    for lo_hi in [(0, 0), (1, 2), (2, 4)]:           # sub-ranges: only first..last is sorted
        add("sort-part-%d-%d" % lo_hi, proc="QuickSort", args=[{"ref": "array:double", "values": [9.0, 8.0, 7.0, 6.0, 5.0]}, *lo_hi])

    scn = {
        "family": "lairs",
        "description": "Lair statistics (FAM-11, stock): GetLairAveragesFromLocs, CountLairsByMaxRegen, "
                       "CalculateAverageLairs, the Dictionary count helpers and the median/outlier helpers.",
        "notes": ["GetLairInfo and GetDamageOutput are answered by the oracle from data.lairs and the case's lair_env "
                  "(P3-16 ports the real ones); every GetDamageOutput call is recorded as an event.",
                  "Lair groups, monster numbers and the Summoned By text are real; the per-group numbers are invented."],
        "database": False,
        "shims": ["lairs"],
        "tolerance": {"default": 0},
        "data": {"lairs": table},
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "table", len(table), "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

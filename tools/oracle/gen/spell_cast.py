"""Writes fixtures/scenarios/spell_cast.json (FAM-09, spell casting) from the stock database.

    python3 -I tools/oracle/gen/spell_cast.py [DB.sqlite]

Deterministic (fixed strides and a splitmix-style counter, no clock). The case "table" carries the
Spells rows any other case names (compact; only fields the Rust port reads) and the Classes rows'
magery fields, so the Rust test needs no database. Case ids "as-*" record GetAbilityStats(n, 0)
for CalculateSpellCast's level-increase text (P3-07's procedure); the Rust test checks the real port against them.
"""
import json
import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DEFAULT_DB = os.path.expanduser(
    "~/.cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite")
MISSING = {"t": "missing"}
FIELDS = ["Number", "Name", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists",
          "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc",
          "DurIncLVLs", "DurInc", "Learnable", "LearnedFromLen", "CastedByLen", "Classes"]
HIDDEN = {23, 51, 52, 80, 97, 98, 100, 108, 109, 110, 111, 112, 113, 115, 119, 122, 138, 144, 151, 164, 178, 137}


def load(db):
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    rows = {}
    for r in c.execute("select * from Spells order by Number"):
        ab = [(r["Abil-%d" % i], r["AbilVal-%d" % i]) for i in range(10)]
        rows[r["Number"]] = {
            "f": [r["Number"], r["Name"] or "", r["ReqLevel"], r["EnergyCost"], r["ManaCost"], r["MinBase"],
                  r["MaxBase"], r["Diff"], r["TypeOfResists"], r["Targets"], r["Dur"], r["AttType"], r["Magery"],
                  r["MageryLVL"], r["Cap"], r["MaxIncLVLs"], r["MaxInc"], r["MinIncLVLs"], r["MinInc"],
                  r["DurIncLVLs"], r["DurInc"], r["Learnable"], len(r["Learned From"] or ""),
                  len(r["Casted By"] or ""), r["Classes"] or ""],
            "ab": ab}
    classes = [[r["Number"], r["MageryType"], r["MageryLVL"]] for r in c.execute("select * from Classes order by Number")]
    return rows, classes


def attrs(r):
    d = dict(zip(FIELDS, r["f"]))
    d["ab"] = r["ab"]
    return d


def signature(r):
    d = attrs(r)
    kinds = tuple(sorted({(a, v == 0) for a, v in d["ab"] if a in (1, 8, 17, 18, 144)}))
    inc = (d["MinInc"] != 0 and d["MinIncLVLs"] > 0, d["MaxInc"] != 0 and d["MaxIncLVLs"] > 0,
           d["DurInc"] != 0 and d["DurIncLVLs"] > 0)
    return (d["AttType"], kinds, d["TypeOfResists"], d["Dur"] > 1, d["Cap"] > 0, d["ReqLevel"] > 0, inc,
            0 if d["EnergyCost"] == 0 else 1 if d["EnergyCost"] > 500 else 2 if d["EnergyCost"] < 1000 else 3,
            d["Magery"] == 5, d["Diff"] >= 200, d["Diff"] < 0, d["Targets"] in (2, 8), d["ManaCost"] > 0)


class Rng:
    def __init__(self, seed):
        self.s = seed & (2**64 - 1)

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & (2**64 - 1)
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & (2**64 - 1)
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & (2**64 - 1)
        return z ^ (z >> 31)

    def pick(self, xs):
        return xs[self.next() % len(xs)]


def trim(args):
    args = list(args)
    while args and (args[-1] is False or args[-1] == 0) and not isinstance(args[-1], dict):
        args.pop()
    return args


def main():
    rs, classes = load(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB)
    nums = sorted(rs)
    needed = set()
    cases = []
    rng = Rng(910)

    def one(cid, **kw):
        c = {"id": cid}
        c.update(kw)
        cases.append(c)

    # ---- spell pools
    seen_sig, seen_ab, cast_pool = set(), {}, []
    for n in nums:
        r = rs[n]
        d = attrs(r)
        sig = (d["AttType"], tuple(sorted({a for a, _ in r["ab"] if a in (1, 8, 17, 18, 144)})), d["TypeOfResists"], d["Dur"] > 1, d["Cap"] > 0 and d["ReqLevel"] > 0, d["EnergyCost"] > 0, d["Diff"] >= 200)
        inc = (d["MinInc"] != 0 and d["MinIncLVLs"] > 0) or (d["MaxInc"] != 0 and d["MaxIncLVLs"] > 0) \
            or (d["DurInc"] != 0 and d["DurIncLVLs"] > 0)
        new_ab = [a for a, _ in r["ab"] if a and inc and seen_ab.get(a, 0) < 1]
        if sig not in seen_sig or new_ab:
            seen_sig.add(sig)
            for a, _ in r["ab"]:
                if a and inc:
                    seen_ab[a] = 1
            cast_pool.append(n)
    for n in nums[::160]:
        if n not in cast_pool:
            cast_pool.append(n)
    print("pool", len(cast_pool))
    cast_pool.sort()

    profiles = [
        {},
        {"nSpellDmgBonus": 25, "nSpellcasting": 150, "nMaxMana": 200, "nSpellOverhead": 2, "nManaRegen": 12},
        {"nSpellDmgBonus": 100, "nSpellcasting": 60, "nMaxMana": 80, "nManaRegen": 0},
        {"nSpellDmgBonus": -10, "nSpellcasting": 320, "nMaxMana": 1500.5, "nSpellOverhead": 10, "nManaRegen": 33.5},
        {"nSpellDmgBonus": 7, "nSpellcasting": -20, "nMaxMana": 40, "nManaRegen": 4.25},
        {"nSpellDmgBonus": 3, "nSpellcasting": 45, "nMaxMana": 9000, "nSpellOverhead": 0.5, "nManaRegen": 250},
    ]
    # (cast level, MR, anti-magic, cold, fire, stone, lightning, water)
    reqs = [
        (0, 0, False, 0, 0, 0, 0, 0),
        (30, 0, False, 0, 0, 0, 0, 0),
        (0, 80, False, 0, 0, 0, 0, 0),
        (12, 120, True, 25, 25, 25, 25, 25),
        (200, 40, False, -50, -50, -50, -50, -50),
        (-3, 51, False, 10, 20, 30, 40, 50),
        (5, 200, True, 0, 0, 0, 0, 0),
        (1, 50, False, 100, 100, 100, 100, 100),
        (255, 52, True, 5, 5, 5, 5, 5),
    ]

    def cast(cid, p, n, q):
        args = [{"udt": "tCharacterProfile", "fields": p}] + trim([n] + list(q))
        # the spell number is required even when the later arguments trim away
        one(cid, proc="CalculateSpellCast", args=args)
        needed.add(n)

    for i, n in enumerate(cast_pool):
        d = attrs(rs[n])
        ks = (0, 2, 3, 4, 5, 6, 7) if d["AttType"] in (0, 1, 2, 3, 5) and any(a in (1, 17) for a, _ in d["ab"]) else (0, 1, 2, 3, 6)[: 2 + i % 3]
        for k in ks:
            cast("c-%d-%d" % (n, k), profiles[(i + k) % len(profiles)], n, reqs[k])
    # specific edges
    edge_spells = [n for n in cast_pool if any(a == 17 for a, _ in rs[n]["ab"])][:6]
    for n in edge_spells:
        for j, mr in enumerate([1, 49, 50, 51, 52, 53, 98, 99, 100, 196, 300, 70000]):
            for am in (False, True):
                if (n + j) % 4 == 0:
                    cast("mr-%d-%d-%d" % (n, j, am), profiles[1], n, (10, mr, am))
    for i, n in enumerate(cast_pool[::24]):
        for j, p in enumerate([{"nSpellcasting": 32767}, {"nSpellcasting": 1, "nMaxMana": 3e9, "nManaRegen": 1},
                               {"nSpellcasting": 100, "nMaxMana": 10, "nSpellDmgBonus": 32767},
                               {"nSpellDmgBonus": 100, "nSpellcasting": 98, "nMaxMana": 1000, "nManaRegen": 3}]):
            cast("pe-%d-%d" % (n, j), p, n, (40000, 0) if j == 0 else (20, 30))
    for n in (0, -1, 99999, 3000):
        cast("absent-%d" % n, profiles[1], n, (10, 50))
    for lv in (-1000, 0, 1, 32767, 32768, 40000):
        for n in cast_pool[::60]:
            cast("lv-%d-%d" % (n, lv), profiles[0], n, (lv,))

    # ---- ability-name recordings for the level-increase text
    ab_needed = set()
    for n in needed:
        for a, _ in (rs[n]["ab"] if n in rs else []):
            if a > 0 and a not in HIDDEN:
                ab_needed.add(a)
    for a in sorted(ab_needed):
        one("as-%d" % a, proc="GetAbilityStats", args=[a, 0, MISSING, False])

    # ---- resist helpers
    dmgs = [0, 1, 2, 7, 13, 100, 250, 1000, 33333, {"t": "cur", "v": 12.5}, {"t": "cur", "v": 7.25},
            {"t": "cur", "v": 0.0001}, {"t": "cur", "v": 999999.9999}, -50]
    mrs = [-5, 0, 1, 10, 49, 50, 51, 52, 53, 75, 100, 101, 150, 196, 300, 5000]
    for t in range(220):
        d, m = rng.pick(dmgs), rng.pick(mrs)
        a = [d, m, rng.pick([0, 1, 2, 2, MISSING]), rng.pick([True, True, False, MISSING]),
             rng.pick([True, False, False, MISSING]), rng.pick([True, False, MISSING]),
             rng.pick([0, 0, 10, 25, 50, 99, 100, 101, 150, MISSING])]
        while a and a[-1] == MISSING:
            a.pop()
        one("rd-%d" % t, proc="CalculateResistDamage", args=a)
    for d in (2147483647, {"t": "cur", "v": 900000000000000}):
        one("rd-big-%s" % (d if not isinstance(d, dict) else "cur"), proc="CalculateResistDamage", args=[d, 100, 2, True, True])
    pairs = [(0, 0), (-3, 5), (10, 10), (10, 9.5), (10, 8.9), (10, 8.5), (10, 5), (10, 0), (10, -0.5), (10, -5),
             (10, -15), (3, 1.5), (7, 3.5), (100, 33.3), (1, 0), (0.5, -0.2), (200, 400), (1e9, -5e9), (5, 2.5),
             (1e12, 0), (1e12, -1e5), (100, 100), (100, 101.5), (100, 98.9), (100, 98), (40, 12.5), (-1, -1)]
    for i, (b, f) in enumerate(pairs):
        one("rp-%d" % i, proc="ResistPct_SignedOfBase", args=[b, f])
        one("np-%d" % i, proc="NegResistPct_ShareOfTotal", args=[b, f])
    for t in range(60):
        b = rng.pick([1, 2, 3, 5, 9, 17, 50, 120, 1000, 12345.5])
        f = rng.pick([-1, -2.5, -7, -33, -100, -999, 0, 0.4, 3, 77, 500])
        one("rpr-%d" % t, proc="ResistPct_SignedOfBase", args=[b, f])
        one("npr-%d" % t, proc="NegResistPct_ShareOfTotal", args=[b, f])
    for a in range(-1, 9):
        one("at-%d" % a, proc="SpellAttackTypeEnum", args=[a])
        one("ats-%d" % a, proc="SpellAttackTypeEnum", args=[a, True])
    one("at-big", proc="SpellAttackTypeEnum", args=[32767])

    # ---- cast chance
    sc_list = [0, 1, 20, 50, 98, 150, 200, 400, 32767, -5]
    df_list = [0, 100, 200, 199, -25, -100, -400, 1, 50, 32767, -32768]
    chance_spells = cast_pool[::6]
    for t in range(160):
        sc, df = rng.pick(sc_list), rng.pick(df_list)
        kai = rng.pick([False, True])
        sp = rng.pick([0, 0, 0] + chance_spells + [99999, -2]) if df == 0 else 0
        a = trim([df, sc, kai, sp])
        if sp:
            needed.add(sp)
        one("cc-%d" % t, proc="GetSpellCastChance", args=a or [0])
    one("cc-none", proc="GetSpellCastChance", args=[])
    for n in cast_pool[::8]:
        needed.add(n)
        one("ccs-%d" % n, proc="GetSpellCastChance", args=[0, 120, False, n])

    # ---- rounds to oom
    costs = [0, 1.5, 5, 10, 25, 33, 100, 250, 400, 1000, 5000.5]
    maxm = [0, 50, 100, 200, 500, 1000, 5000]
    regen = [0, 0.5, 3, 10, 12, 30, 100, 1000, -2]
    chn = [MISSING, 0, 100, 99, 75, 50, 20, 1, -4, 32767]
    dur = [MISSING, 0, 1, 2, 10, 30, 100, 1000, 1000000, 800000000]
    for t in range(320):
        a = [rng.pick(costs), rng.pick(maxm), rng.pick(regen), rng.pick(chn), rng.pick(dur)]
        while a and a[-1] == MISSING:
            a.pop()
        one("oom-%d" % t, proc="CalcRoundsToOOM", args=a)
    for t in range(50):
        one("oomb-%d" % t, proc="CalcRoundsToOOM",
            args=[rng.pick([4, 12, 20, 60]), rng.pick([100, 300, 1000]), rng.pick([0, 1, 4, 20, 50]),
                  rng.pick([100, 80, 50, 25]), rng.pick([1, 1, 5, 20, 60, 120])])

    # ---- mana recovery
    mm = [0, 1, 10, 100, 250, 1000, 20000, 2000000000, -10]
    rg = [0, 1, 3, 10, 50, 500, 1500000000, -4]
    md = [MISSING, 0, 1, 5, 20]
    pc = [MISSING, 0, 1, 50, 95, 100, 150, -5]
    for t in range(110):
        a = [rng.pick(mm), rng.pick(rg), rng.pick(md), rng.pick(pc)]
        while a and a[-1] == MISSING:
            a.pop()
        one("mr-%d" % t, proc="CalcManaRecoveryRounds", args=a)

    # ---- spell lookups: in game / usable
    ig_pool = nums[::23]
    ig_pool = sorted(set(ig_pool) | set(cast_pool[::6]))
    ig_n = ig_pool + [0, -1, 99999]
    for n in ig_n:
        needed.add(n) if n in rs else None
        for nmr in (1.7, 1.8):
            for kai in (False, True):
                if (n + int(nmr * 10) + kai) % 2 == 0 or n in (0, -1, 99999):
                    one("ig-%d-%s-%d" % (n, nmr, kai), proc="SpellIsInGame", args=[n],
                        globals={"nNMRVer": nmr, "bDisableKaiAutolearn": kai})
    use_pool = sorted(set(ig_pool[::2]) | {n for n in nums if any(a in (97, 98, 110, 111, 112, 113) for a, _ in rs[n]["ab"])}
                      | set(nums[::31]))
    for t in range(420):
        n = rng.pick(use_pool)
        needed.add(n)
        cls = rng.pick([0, 1, 2, 3, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 16, -1])
        lvl = rng.pick([0, 0, 1, 5, 12, 30, 100, -3])
        al = rng.pick([0, 0, 1, 2, 3, 4, -1])
        a = trim([n, cls, lvl, al, rng.pick([False, False, True])])
        one("iu-%d" % t, proc="SpellIsUsable", args=a,
            globals={"nNMRVer": rng.pick([1.6, 1.7, 1.8, 1.8]), "bOnlyInGame": rng.pick([False, True]),
                     "bDisableKaiAutolearn": rng.pick([False, True])})
    for n in (0, -1, 99999):
        one("iu-absent-%d" % n, proc="SpellIsUsable", args=[n, 3])

    spells_out = [rs[n]["f"] + [[x for p in rs[n]["ab"] for x in p]] for n in sorted(needed) if n in rs]
    cases.insert(0, {"id": "table", "fields": FIELDS + ["AbilitiesFlat"], "spells": spells_out,
                     "classes": classes})
    scn = {
        "family": "spell_cast", "database": True,
        "description": "Spell casting (FAM-09, stock): CalculateSpellCast with resist, anti-magic, elemental, level-cap, "
                       "mana and area spells; CalculateResistDamage, GetSpellCastChance, CalcRoundsToOOM, "
                       "CalcManaRecoveryRounds, SpellIsInGame, SpellIsUsable and the small helpers.",
        "notes": [
            "Generated by tools/oracle/gen/spell_cast.py. The case 'table' carries the Spells rows other cases name (fields in "
            "'fields'; Learned From and Casted By as lengths, abilities as a flat [ability, value, ...] list without empty "
            "slots) and the Classes magery fields [number, MageryType, MageryLVL], so the Rust test needs no database.",
            "Cases 'as-*' record GetAbilityStats(n, 0) (P3-07's procedure) for the 'LVL Increases' text; the Rust test replays them.",
            "The CalculateSpellCast profile argument sets only the fields the procedure reads (nSpellDmgBonus, nSpellcasting, "
            "nMaxMana, nSpellOverhead, nManaRegen)."],
        "tolerance": {"default": 0},
        "cases": cases}
    with open(os.path.join(ROOT, "fixtures", "scenarios", "spell_cast.json"), "w") as f:
        json.dump(scn, f, separators=(",", ":"))
        f.write("\n")
    print(len(cases), "cases,", len(spells_out), "table spells")


main()

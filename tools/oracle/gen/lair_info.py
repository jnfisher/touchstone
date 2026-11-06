#!/usr/bin/env python3
"""Writes fixtures/scenarios/lair_info.json (FAM-11 cache half, card P3-16).

    python3 -I tools/oracle/gen/lair_info.py [DB.sqlite]

LoadLairInfo builds MME's per-group lair cache from the Lairs and Monsters tables; GetLairInfo then
answers for (group index, max regen) with the damage the loaded character (or party) does, rounds to kill
and clear, and the damage taken. Both run unchanged in the oracle (no lair shims): a case copies a
character from the pipeline scenario, so the oracle's own RefreshAll sets frmMain up, then
SetCurrentAttackTypeConfig, LoadLairInfo and a list of GetLairInfo calls run.

Cases:
- `solo-*`: a loaded character (global filter on, no party): warrior (weapon), mage (spell), mystic
  (martial arts), rogue (the warrior with backstab); each call list repeats some groups, so the cached-damage path is taken.
- `nofilter-*`: global filter off, the default defender.
- `party<N>-*`: lair mode with a party of N, anti-magic count 0 / some / all.
- `startup-*`: bStartup True, so GetLairInfo returns what LoadLairInfo stored.
- `hand-*`: group strings that are not in the table, a regen of 0, a four-part index, indexes that differ only in case, a negative regen.

The ~100 real (group, regen) pairs are stratified: every lair trait (undead, anti-magic, animal, non-living,
magic level, spell immunity, resists, BS defence, mixed melee accuracy) first, rarest first, then a stride.
The damage-vs-character and damage-vs-party caches are family-wide arrays of invented, fixed values for the
monsters of the chosen lairs (-1 elsewhere: not calculated, so GetMonsterAvgDmgFromDB answers).
Deterministic: sha256-seeded choices only. The "table" key holds the rows the Rust test builds its World from.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "lair_info.json")
PIPE = json.load(open(os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")))
PAT = re.compile(r"\[([\d\-]+)\]\[(\d+)\]Group\(lair\): (\d+)/(\d+)", re.A)


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    lairs = {r["GroupIndex"]: dict(r) for r in c.execute("select * from Lairs order by GroupIndex")}
    mons = {r["Number"]: dict(r) for r in c.execute("select * from Monsters order by Number")}
    top = max(mons)

    pairs = {}
    for (s,) in c.execute("select \"Summoned By\" from Monsters where \"Summoned By\" like '%(lair)%' order by Number"):
        for g, r, _, _ in PAT.findall(s):
            pairs.setdefault(g, set()).add(int(r))

    def abil(m):
        return {m["Abil-%d" % i]: m["AbilVal-%d" % i] for i in range(10) if m["Abil-%d" % i]}

    def traits(g):
        t = set()
        L = lairs[g]
        ms = [mons[int(x)] for x in L["MobList"].split(",") if x.strip().isdigit() and int(x) in mons]
        n = len(L["MobList"].split(","))
        t.add("mobs%d" % (1 if n == 1 else 2 if n <= 3 else 3 if n <= 7 else 4))
        accs = set()
        for m in ms:
            a = abil(m)
            if m["Undead"] == 1:
                t.add("undead")
            if 51 in a:
                t.add("antimagic")
            if 78 in a:
                t.add("animal")
            if 109 in a:
                t.add("nonliving")
            if 28 in a:
                t.add("magic%d" % min(a[28], 4))
            if 139 in a:
                t.add("immune%d" % min(a[139], 3))
            for k, nm in ((3, "rcol"), (5, "rfir"), (65, "rsto"), (66, "rlit"), (147, "rwat")):
                if k in a:
                    t.add(nm)
            if m["BSDefense"]:
                t.add("bsdef")
            for i in range(5):
                if 1 <= m["AttType-%d" % i] <= 3 and round(m["AttTrue%%-%d" % i]) > 0:
                    accs.add(m["AttAcc-%d" % i])
            if any(m["AttType-%d" % i] == 2 for i in range(5)):
                t.add("spellatk")
        if len(accs) > 1:
            t.add("mixedacc")
        if not accs:
            t.add("nomelee")
        if max(pairs.get(g, {0})) >= 6:
            t.add("bigregen")
        return t

    groups = sorted(g for g in lairs if g in pairs)
    TR = {g: traits(g) for g in groups}
    by_trait = {}
    for g in groups:
        for t in TR[g]:
            by_trait.setdefault(t, []).append(g)
    chosen = []
    for t in sorted(by_trait, key=lambda t: (len(by_trait[t]), t)):
        gs = sorted(by_trait[t], key=lambda g: H("t", t, g))
        for g in gs[:3]:
            if g not in chosen:
                chosen.append(g)
    for g in groups[::4]:
        if len(chosen) < 100 and g not in chosen:
            chosen.append(g)
    chosen = sorted(chosen[:100])

    def regen_of(g, k=0):
        rs = sorted(pairs[g])
        return rs[H("regen", g, k) % len(rs)]

    PAIRS = [(g, regen_of(g)) for g in chosen]
    UNKNOWN = sorted(g for g in pairs if g not in lairs)

    # ---- the data the Rust test rebuilds the lairs from
    mob_nums = sorted({int(x) for g in chosen for x in lairs[g]["MobList"].split(",") if x.strip().isdigit() and int(x) in mons})

    def mon_row(n):
        m = mons[n]
        a = abil(m)
        return [n, m["Undead"], m["BSDefense"], m["AvgDmg"],
                [[m["Abil-%d" % i], m["AbilVal-%d" % i]] for i in range(10)],
                [[m["AttType-%d" % i], m["AttAcc-%d" % i], m["Att%%-%d" % i], m["AttTrue%%-%d" % i]] for i in range(5)]]

    LAIR_FIELDS = ["GroupIndex", "MobList", "Mobs", "TotalLairs", "AvgDelay", "AvgWalk", "AvgExp", "AvgDmg", "AvgHP",
                   "AvgAC", "AvgDR", "AvgMR", "AvgDodge"]
    table = {
        "lair_fields": LAIR_FIELDS,
        "lairs": [[lairs[g][k] for k in LAIR_FIELDS] for g in chosen],
        "monster_fields": ["Number", "Undead", "BSDefense", "AvgDmg", "Abilities(ability,value)[10]",
                           "Attacks(type,acc,pct,truepct)[5]"],
        "monsters": [mon_row(n) for n in mob_nums],
    }

    # ---- invented damage caches
    vs_char = [-1] * (top + 1)
    vs_party = [-1] * (top + 1)
    for n in mob_nums:
        base = mons[n]["AvgDmg"]
        if H("vc", n) % 5:
            vs_char[n] = round(base * (0.3 + (H("vcf", n) % 14) / 10.0), 1)
        if H("vp", n) % 4:
            vs_party[n] = round(base * (0.2 + (H("vpf", n) % 18) / 10.0) + (H("vpa", n) % 3) * 0.5, 1)

    # ---- characters
    PC = {x["id"]: x for x in PIPE["cases"]}
    SETUP_FORMS = json.loads(json.dumps(PIPE["forms"]))

    def char_forms(base, **more):
        f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
        f.update(more)
        return f

    def char_globals(base, **more):
        g = json.loads(json.dumps(PC[base]["globals"]))
        g.update(more)
        return g

    CH = {
        "warrior": ("ladder-c1-l50", {"nGlobalAttackTypeMME": 1, "bGlobalAttackBackstab": False}),
        "rogue": ("ladder-c1-l50", {"nGlobalAttackTypeMME": 1, "bGlobalAttackBackstab": True}),
        "mage": ("ladder-c12-l50", {"nGlobalAttackTypeMME": 2}),
        "mystic": ("ladder-c15-l50", {}),
    }

    def party_forms(size, anti):
        lt = {0: str(size), 1: "60", 2: "20", 3: "50", 4: "50", 5: "400", 6: str(anti), 7: "12", 8: "250", 9: "1"}
        return {"optMonsterFilter": arr({1: {"Value": True}}),
                "txtMonsterLairFilter": arr({k: {"Text": v} for k, v in lt.items()}),
                "txtMonsterDamage": {"Text": "55"},
                "txtMonsterDamageOUT": arr({0: {"Text": "140"}, 1: {"Text": "60"}})}

    cases = []

    def case(cid, who, calls, forms=None, glob_=None, startup=False):
        base, gl = CH[who]
        g = char_globals(base, **gl)
        g.update(glob_ or {})
        if startup:
            g["bStartup"] = True
        steps = [{"call": "RefreshAll"}, {"call": "SetCurrentAttackTypeConfig"}]
        steps.append({"call": "LoadLairInfo"})
        if startup:
            g["bStartup"] = True
        for i, (gi, rg) in enumerate(calls):
            args = [gi] if rg is None else [gi, rg]
            steps.append({"call": "GetLairInfo", "args": args, "as": "r%d" % i})
        cases.append({"id": cid, "forms": {"frmMain": char_forms(base, **(forms or {}))}, "globals": g, "steps": steps,
                      "read": {"config": "%g.sGlobalAttackConfig"}})

    def pick(n, off=0):
        step = max(1, len(PAIRS) // n)
        return [PAIRS[(off + k * step) % len(PAIRS)] for k in range(n)]

    def with_repeats(lst):
        """The list, then a few of its pairs again (cached damage) and again with another regen."""
        again = lst[::5]
        other = [(g, regen_of(g, 1)) for g, _ in lst[2::7]]
        return lst + again + other

    for k, who in enumerate(("warrior", "mage", "mystic")):
        case("solo-" + who, who, with_repeats(pick(34, k * 3)))
        case("nofilter-" + who, who, with_repeats(pick(14, k * 5 + 1)), forms={"chkGlobalFilter": {"Value": 0}})
    for who in ("warrior", "mage"):
        for size, anti in ((2, 0), (3, 1), (4, 2), (6, 6)):
            case("party%d-%s-anti%d" % (size, who, anti), who, with_repeats(pick(22, size + anti)), forms=party_forms(size, anti))
    case("solo-rogue", "rogue", with_repeats(pick(20, 11)))
    case("party3-rogue", "rogue", with_repeats(pick(12, 13)), forms=party_forms(3, 1))
    # a one-person party in lair mode: nParty is 1, so the cached path applies
    case("party1-warrior", "warrior", with_repeats(pick(12, 7)), forms=party_forms(1, 0))
    case("party-nofilter-mage", "mage", with_repeats(pick(12, 9)), forms=dict(party_forms(3, 1), chkGlobalFilter={"Value": 0}))
    # party size above 6 is capped at 6; below 1 is 1
    case("party-over-warrior", "warrior", pick(10, 2), forms=party_forms(9, 0))
    case("party-zero-warrior", "warrior", pick(10, 3), forms=party_forms(0, 0))

    # startup: LoadLairInfo's own values
    for k in range(3):
        case("startup-%d" % k, "warrior", [(g, regen_of(g, k)) for g in chosen[k::3]], startup=True)

    # hand-built group strings
    hand = [("99-99-99", 3), ("0-0-0", 1), (UNKNOWN[0] if UNKNOWN else "98-98-98", 2), (chosen[0], 0), (chosen[0], None),
            (chosen[0] + "-4", None), (chosen[0] + "-4", 7), ("", 3), ("1-2", 5), ("abc", 4), (chosen[1], 1), (chosen[1], 32767),
            (chosen[2], 2), (chosen[2], 2), (chosen[3], 15), (chosen[4], -3), ("AB-12-5", 2), ("ab-12-5", 3), ("Ab-12-5-4", None)]
    case("hand-warrior", "warrior", hand)
    case("hand-startup", "warrior", hand, startup=True)
    case("hand-mage-party", "mage", hand, forms=party_forms(4, 1))

    scn = {
        "family": "lair_info",
        "description": "Lair cache (FAM-11, stock): LoadLairInfo, GetLairInfo, SetLairInfo and GetLairInfoIndex with the real "
                       "GetDamageOutput, CalcCombatRounds and GetPreCalculatedMonsterDamage, for loaded characters and parties.",
        "notes": [
            "Characters are the pipeline scenario's ladder-c1-l50 (warrior: weapon attack; rogue: the same with backstab), "
            "ladder-c12-l50 (mage, spell attack) and ladder-c15-l50 (mystic, martial arts): RefreshAll runs in the oracle, then "
            "SetCurrentAttackTypeConfig, LoadLairInfo and the GetLairInfo calls; each GetLairInfo result is an output r<i> "
            "(LairInfoType, lower-cased member names). 'config' is sGlobalAttackConfig afterwards.",
            "The Lairs and Monsters tables are the stock database. The Rust test builds its World from data.table: the chosen "
            "lairs and the monsters in their mob lists only; the oracle loads all 421 lairs, which cannot change a chosen one.",
            "nMonsterDamageVsChar and nMonsterDamageVsParty are family-wide arrays of invented, fixed values for the monsters "
            "of the chosen lairs; -1 means 'not calculated', so GetMonsterAvgDmgFromDB answers (the stored AvgDmg).",
            "CalculateMonsterPartyDamage (frmMain) calls CalculateMonsterDamageVsCharALL, which the oracle cannot run (progress "
            "bar form); the Rust test checks it against the per-monster function.",
            "Call lists repeat some pairs, so the cached-damage path and the second call with another max regen are covered.",
        ],
        "database": True,
        "frm": PIPE["frm"],
        "tolerance": {"default": 0},
        "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "nMonsterDamageVsChar": vs_char,
                    "nMonsterDamageVsParty": vs_party, "nMonsterDamageVsDefault": [-1] * (top + 1),
                    # SetCurrentAttackTypeConfig clears these when the attack set-up changed
                    **{n: [-1] * (top + 1) for n in (
                        "nCharDamageVsMonster", "nCharFirstRoundDamageVsMonster", "nCharMinRoundDamageVsMonster",
                        "nCharSurpriseDamageVsMonster", "nCharSurpriseMinDamageVsMonster", "nCharSurpriseChanceVsMonster")}},
        "forms": SETUP_FORMS,
        "data": {"table": table},
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "chosen", len(chosen), "monsters", len(mob_nums), "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

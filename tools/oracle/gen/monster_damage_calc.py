#!/usr/bin/env python3
"""Writes fixtures/scenarios/monster_damage_calc.json (FAM-13 menu actions; card P4-30).

    python3 -I tools/oracle/gen/monster_damage_calc.py [DB.sqlite]

modMain CalculateMonsterDamageVsCharALL (the Options menu's "Calculate monster damage" for the character, and with True
for the party) and ClearMonsterDamageVsCharALL, run over a small Monsters table (scenario key `db_only`: ALL walks the whole
table and every monster is a simulation, so the stock table would take hours in the interpreter). The progress-bar form
is a FormShim. Each case sets a character up with the oracle's own RefreshAll (a case of the pipeline scenario), as the
explorer's character is, then runs the menu action and reads back the three arrays' module globals: the damage array
it filled, sMonsterDamageVsCharDefenseConfig, sGlobalCharDefenseDescription and the "already asked" flags.

The table holds melee-only monsters (no spell attacks, mid-fight spells, hit spells or death spell, and no item with a
combat bonus), so the Rust test can rebuild the World from the scenario's `table` key without spells or items; the
simulations of spell attackers are pinned by monster_glue_char. Every case starts with MME's initial generator state,
which is also where the server's calculation starts. Deterministic: sha256-seeded choices only.
"""
import glob
import hashlib
import json
import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "monster_damage_calc.json")
PIPE = json.load(open(os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")))
ITEM_ABS = (4, 22, 105, 106)
N_MELEE = 14
N_QUIET = 3
MAX_NUM = 450     # keeps the arrays (one per case in the fixture) short


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    mons = {r["Number"]: dict(r) for r in c.execute("select * from Monsters order by Number")}
    items = {r["Number"]: dict(r) for r in c.execute("select * from Items order by Number")}
    top = None

    def item_bonus(n):
        i = items.get(n) if n else None
        return i is not None and any(i["Abil-%d" % k] in ITEM_ABS for k in range(20))

    def attacks(m):
        return [x for x in range(5) if 0 < m["AttType-%d" % x] < 4]

    def spelly(m):
        return (any(m["AttType-%d" % x] == 2 or m["AttHitSpell-%d" % x] or m["MidSpell-%d" % x] for x in range(5))
                or m["DeathSpell"] or m["CreateSpell"])

    def plain(m):
        return not spelly(m) and not item_bonus(m["Weapon"]) and not any(item_bonus(m["DropItem-%d" % x]) for x in range(10))

    melee = sorted(n for n, m in mons.items() if n <= MAX_NUM and m["Name"] and attacks(m) and plain(m) and m["AvgDmg"] > 0)
    quiet = sorted(n for n, m in mons.items() if n <= MAX_NUM and m["Name"] and not attacks(m) and not any(m["MidSpell-%d" % x] for x in range(5))
                   and plain(m))

    def pick(pool, k, salt):
        # spread over the pool by hash order, then number order
        return sorted(sorted(pool, key=lambda n: H(salt, n))[:k])

    chosen = sorted(set(pick(melee, N_MELEE, "m") + pick(quiet, N_QUIET, "q")))
    top = max(chosen)

    # ---- characters (the pipeline scenario's, as monster_detail uses them)
    PC = {x["id"]: x for x in PIPE["cases"]}
    SETUP_FORMS = json.loads(json.dumps(PIPE["forms"]))
    CH = {
        "warrior": ("ladder-c1-l50", {"nGlobalAttackTypeMME": 1, "bGlobalAttackBackstab": False}),
        "mage": ("ladder-c12-l50", {"nGlobalAttackTypeMME": 2}),
        "mystic": ("ladder-c15-l50", {}),
    }

    def lair_forms(size, anti=0, ac="12", dr="3", mr="20", dodge="10"):
        lt = {0: str(size), 1: ac, 2: dr, 3: mr, 4: dodge, 5: "400", 6: str(anti), 7: "12", 8: "250", 9: "1"}
        return {"optMonsterFilter": arr({1: {"Value": True}}),
                "txtMonsterLairFilter": arr({k: {"Text": v} for k, v in lt.items()}),
                "txtMonsterDamage": {"Text": "55"},
                "txtMonsterDamageOUT": arr({0: {"Text": "140"}, 1: {"Text": "60"}})}

    SIM = {"sim": {"class": "clsMonsterAttackSim", "global": "clsMonAtkSim"}}
    READ = {
        "chr": "%g.nMonsterDamageVsChar", "party": "%g.nMonsterDamageVsParty",
        "config": "%g.sMonsterDamageVsCharDefenseConfig", "desc": "%g.sGlobalCharDefenseDescription",
        "dont_char": "%g.bDontPromptCalcCharMonsterDamage", "party_calc": "%g.bMonsterDamageVsPartyCalculated",
        "dont_party": "%g.bDontPromptCalcPartyMonsterDamage", "rnd": "%rnd",
    }
    cases = []

    def case(cid, who, steps, forms=None, glob_=None):
        base, gl = CH[who]
        g = json.loads(json.dumps(PC[base]["globals"]))
        g.update(gl)
        g.update(glob_ or {})
        f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
        f.update(forms or {})
        pre = [{"call": "RefreshAll"}, {"call": "SetCurrentAttackTypeConfig"}]
        cases.append({"id": cid, "forms": {"frmMain": f}, "globals": g, "objects": SIM, "steps": pre + steps})

    ALL_CHAR = {"call": "CalculateMonsterDamageVsCharALL", "args": []}
    ALL_PARTY = {"call": "CalculateMonsterDamageVsCharALL", "args": [True]}
    CLEAR_CHAR = {"call": "ClearMonsterDamageVsCharALL", "args": []}
    CLEAR_PARTY = {"call": "ClearMonsterDamageVsCharALL", "args": [True]}

    for who in CH:
        case("char-" + who, who, [ALL_CHAR])
    case("char-warrior-r250", "warrior", [ALL_CHAR], glob_={"nGlobalMonsterSimRounds": 250})
    case("char-warrior-r30", "warrior", [ALL_CHAR], glob_={"nGlobalMonsterSimRounds": 30})
    case("char-warrior-evil", "warrior", [ALL_CHAR], forms={"cmbGlobalAlignment": {"ListIndex": 3}})
    # the party: size 1 (a solo run on the party boxes), a mixed party and an all-anti-magic party
    for size, anti, tag in ((1, 0, "solo"), (3, 0, "plain"), (4, 2, "mixed"), (5, 5, "allanti"), (6, 1, "one-anti")):
        case("party-%s" % tag, "warrior", [ALL_PARTY], forms=lair_forms(size, anti))
    case("party-ac", "mage", [ALL_PARTY], forms=lair_forms(3, 1, ac="40", dr="9", mr="60", dodge="30"))
    # both, in either order: the second run goes on from where the first left the generator
    case("both-char-party", "warrior", [ALL_CHAR, ALL_PARTY], forms=lair_forms(3, 1))
    case("both-party-char", "warrior", [ALL_PARTY, ALL_CHAR], forms=lair_forms(3, 1))
    # clear: the arrays go back to -1 and the flags to False (clear after calculate; clear on its own)
    case("clear-both", "warrior", [ALL_CHAR, ALL_PARTY, CLEAR_CHAR, CLEAR_PARTY], forms=lair_forms(3, 1))
    case("clear-char-only", "warrior", [ALL_CHAR, ALL_PARTY, CLEAR_CHAR], forms=lair_forms(3, 1))
    case("clear-party-only", "warrior", [ALL_CHAR, ALL_PARTY, CLEAR_PARTY], forms=lair_forms(3, 1))
    case("clear-fresh", "warrior", [CLEAR_CHAR, CLEAR_PARTY])
    for cs in cases:
        cs["read"] = dict(READ)
    # ---- the table the Rust test builds its World from
    rows = []
    for n in chosen:
        m = mons[n]
        atk = [[m["AttName-%d" % x] or "", m["AttType-%d" % x], m["AttAcc-%d" % x], m["Att%%-%d" % x], m["AttTrue%%-%d" % x],
                m["AttMin-%d" % x], m["AttMax-%d" % x], m["AttEnergy-%d" % x], m["AttHitSpell-%d" % x]] for x in range(5)]
        rows.append([n, m["Align"], m["Energy"], m["Weapon"], atk, m["AvgDmg"], m["HP"], m["ArmourClass"], m["DamageResist"],
                     m["MagicRes"], m["Name"] or ""])

    frm = json.loads(json.dumps(PIPE["frm"]))
    frm["stub_procs"] = [p for p in frm["stub_procs"] if p != "SetCharDefenseDescription"]
    scn = {
        "family": "monster_damage_calc",
        "description": "Options menu: CalculateMonsterDamageVsCharALL (character and party) and ClearMonsterDamageVsCharALL over a "
                       "small melee-only Monsters table, with the arrays, the defense description and the 'already asked' flags.",
        "notes": [
            "db_only: the Monsters table is the 'table' key's monsters; the damage arrays are dimensioned to the highest number "
            "in the table, -1 as ClearMonsterDamageVsCharALL leaves them.",
            "frmProgressBar is a FormShim; the calculation's DoEvents, Load/Unload and focus calls are recorded no-ops.",
            "data.table (copied into the fixture header): monsters [Number, Align, Energy, Weapon, [5 x [AttName, AttType, AttAcc, "
            "Att%, AttTrue%, AttMin, AttMax, AttEnergy, AttHitSpell]], AvgDmg, HP, ArmourClass, DamageResist, MagicRes, Name].",
        ],
        "database": True,
        "db_only": {"Monsters": chosen},
        "frm": frm,
        "tolerance": {"default": 0},
        "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "nGlobalMonsterSimRounds": 100,
                    "nMonsterDamageVsChar": [-1] * (top + 1), "nMonsterDamageVsParty": [-1] * (top + 1),
                    "nMonsterDamageVsDefault": [-1] * (top + 1)},
        "forms": dict(SETUP_FORMS, frmMain=dict(SETUP_FORMS["frmMain"]), frmProgressBar={}),
        "data": {"table": {"monsters": rows, "top": top}},
        "cases": cases,
    }
    with open(OUT, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "monsters", len(chosen), "melee pool", len(melee), "top", top, "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

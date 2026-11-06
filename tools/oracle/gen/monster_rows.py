#!/usr/bin/env python3
"""Writes fixtures/scenarios/monster_rows.json (FAM-21, monsters; card P3-27).

    python3 -I tools/oracle/gen/monster_rows.py [DB.sqlite]

modMain AddMonster2LV builds one Monsters-list row: the record's columns, the damage it deals the character (or a
party), and, in lair mode, exp/hr through CalcExpPerHour over the lair averages (GetLairAveragesFromLocs). Each case sets
a character up with the oracle's own RefreshAll (a case of the pipeline scenario), runs SetCurrentAttackTypeConfig,
LoadLairInfo and PopulateCharacterProfile as FilterMonsters does, and then builds the rows of many monsters into recording
ListViews with a `rows` step. tLastAvgLairInfo lives across the rows of one list, as it does while FilterMonsters walks the table.

Modes (the frmMain controls AddMonster2LV reads): solo (`optMonsterFilter(0)`: Exp/(Dmg+HP)), lair (`optMonsterFilter(1)`,
party of 1: exp/hr), party (`txtMonsterLairFilter(0)` above 1), the compare list (a ListView that is not lvMonsters), data
versions 1.82 and 1.7, and the optional arguments FilterMonsters passes when it already worked out exp/hr.
Characters: warrior, mage, mystic from the pipeline scenario, and "no character" (global filter off).

The monsters are stratified (no lairs, regen, rooms, lairs by count, magic level, dodge, anti-magic, exp 0, ExpMulti, undead, BS
defence, mixed accuracy, spell attacks, the two name-less records). The damage-vs-character, vs-party and default arrays are
invented but fixed values (-1 for a share of the monsters). nMonsterPossy and nMonsterSpawnChance are what CalculateAverageLairs
computes from the Lairs table. The "data" key holds the rows the Rust test builds its World from: the subject monsters, the lairs
in their Summoned By text and the monsters in those lairs, and the spells the subjects cast.
Deterministic: sha256-seeded choices only.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys
from decimal import ROUND_HALF_EVEN, Decimal

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "monster_rows.json")
PIPE = json.load(open(os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")))
PAT = re.compile(r"\[([\d\-]+)\]\[(\d+)\]Group\(lair\): (\d+)/(\d+)", re.A)

# sizes (the card keeps the fixture under 1.5 MB and generation under about 5 minutes)
N_SOLO = int(os.environ.get("MR_SOLO", 150))
N_LAIR = int(os.environ.get("MR_LAIR", 40))


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def rnd(x, n):
    """VB6 Round: half to even on the decimal value."""
    q = Decimal(1).scaleb(-n)
    return float(Decimal(repr(x)).quantize(q, rounding=ROUND_HALF_EVEN))


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    lairs = {r["GroupIndex"]: dict(r) for r in c.execute("select * from Lairs order by GroupIndex")}
    mons = {r["Number"]: dict(r) for r in c.execute("select * from Monsters order by Number")}
    spells = {r["Number"]: dict(r) for r in c.execute("select * from Spells order by Number")}
    top = max(mons)

    def abil(m):
        return {m["Abil-%d" % i]: m["AbilVal-%d" % i] for i in range(10) if m["Abil-%d" % i]}

    def sb(m):
        return m["Summoned By"] or ""

    def entries(m):
        return [(g, int(r)) for g, r, _, _ in PAT.findall(sb(m))]

    def mobs_of(g):
        L = lairs.get(g)
        return [int(x) for x in L["MobList"].split(",") if x.strip().isdigit() and int(x) in mons] if L else []

    named = [n for n, m in mons.items() if m["Name"] and not m["Name"].startswith("sdf")]
    skipped = [n for n, m in mons.items() if not m["Name"] or m["Name"].startswith("sdf")]

    # ---- strata
    def attack_spells(m):
        out = set()
        for i in range(5):
            if m["AttType-%d" % i] == 2 and m["AttAcc-%d" % i]:
                out.add(m["AttAcc-%d" % i])
            if m["AttHitSpell-%d" % i]:
                out.add(m["AttHitSpell-%d" % i])
            if m["MidSpell-%d" % i]:
                out.add(m["MidSpell-%d" % i])
        if m["DeathSpell"]:
            out.add(m["DeathSpell"])
        return out

    def mixed_acc(m):
        return len({m["AttAcc-%d" % i] for i in range(5) if 1 <= m["AttType-%d" % i] <= 3 and round(m["AttTrue%%-%d" % i]) > 0}) > 1

    S = {
        "plain": [n for n in named if not sb(mons[n]) and mons[n]["RegenTime"] == 0],
        "regen": [n for n in named if mons[n]["RegenTime"] > 0 and "(lair)" not in sb(mons[n]) and "Room" not in sb(mons[n])],
        "room": [n for n in named if "Room" in sb(mons[n]) and "(lair)" not in sb(mons[n]) and len(sb(mons[n])) < 1500],
        "room_regen": [n for n in named if mons[n]["RegenTime"] > 0 and "Room" in sb(mons[n]) and len(sb(mons[n])) < 1500],
        "lair1": [n for n in named if len(entries(mons[n])) == 1 and mons[n]["RegenTime"] == 0],
        "lair2": [n for n in named if len(entries(mons[n])) == 2 and mons[n]["RegenTime"] == 0],
        "lair3": [n for n in named if 3 <= len(entries(mons[n])) <= 5 and mons[n]["RegenTime"] == 0 and len(sb(mons[n])) < 800],
        "lair6": [n for n in named if 6 <= len(entries(mons[n])) <= 9 and mons[n]["RegenTime"] == 0 and len(sb(mons[n])) < 1200],
        "lairbig": [n for n in named if len(entries(mons[n])) >= 10 and mons[n]["RegenTime"] == 0 and len(sb(mons[n])) < 2500],
        "lair_regen": [n for n in named if len(entries(mons[n])) >= 1 and mons[n]["RegenTime"] > 0 and len(sb(mons[n])) < 1200],
        "magic": [n for n in named if abil(mons[n]).get(28, 0) > 0],
        "dodge": [n for n in named if abil(mons[n]).get(34, 0) > 0],
        "antimagic": [n for n in named if 51 in abil(mons[n])],
        "exp0": [n for n in named if mons[n]["EXP"] == 0],
        "expmulti": [n for n in named if mons[n]["ExpMulti"] not in (1.0,) and mons[n]["EXP"] > 0],
        "undead": [n for n in named if mons[n]["Undead"] == 1],
        "bsdef": [n for n in named if mons[n]["BSDefense"] > 0],
        "mixedacc": [n for n in named if mixed_acc(mons[n])],
        "spellatk": [n for n in named if attack_spells(mons[n])],
        "hp0": [n for n in named if mons[n]["HP"] == 0],
    }
    # strata members are only usable when their Summoned By text is short enough to ship in the fixture
    short = {n for n in named if len(sb(mons[n])) < 2500}
    for k in S:
        S[k] = [n for n in S[k] if n in short]

    def take(stratum, n, salt=""):
        return sorted(S[stratum], key=lambda x: H(stratum, salt, x))[:n]

    lairmode = set()
    for k, n in (("lair1", 6), ("lair2", 5), ("lair3", 6), ("lair6", 4), ("lairbig", 2), ("lair_regen", 5), ("room_regen", 4),
                 ("room", 3), ("plain", 2), ("regen", 3)):
        lairmode.update(take(k, n, "lair"))
    lairmode = sorted(lairmode)[:N_LAIR]
    solo = set(skipped) | set(lairmode)
    for k, n in (("plain", 8), ("regen", 12), ("room", 10), ("room_regen", 8), ("lair1", 8), ("lair2", 6), ("lair3", 8),
                 ("lair6", 6), ("lairbig", 4), ("lair_regen", 8), ("magic", 10), ("dodge", 8), ("antimagic", 6), ("exp0", 5),
                 ("expmulti", 10), ("undead", 5), ("bsdef", 5), ("mixedacc", 8), ("spellatk", 10), ("hp0", 3)):
        solo.update(take(k, n))
    if len(solo) > N_SOLO:
        keep = set(skipped) | set(lairmode)
        rest = sorted((n for n in solo if n not in keep), key=lambda x: H("solo", x))[: max(0, N_SOLO - len(keep))]
        solo = keep | set(rest)
    solo = sorted(solo)
    LAIR_SMALL = lairmode[: max(8, len(lairmode) // 3)]

    # ---- data the Rust test rebuilds the world from
    groups = sorted({g for n in lairmode for g, _ in entries(mons[n])})
    mob_nums = sorted({m for g in groups for m in mobs_of(g)})
    sub_nums = sorted(set(solo) | set(mob_nums))
    sp_nums = sorted({s for n in solo for s in attack_spells(mons[n]) if s in spells})
    # nested lookups of a spell: its EndCast spell (ability 151) matters for "does damage"
    more = True
    while more:
        more = False
        for s in list(sp_nums):
            a = {spells[s]["Abil-%d" % i]: spells[s]["AbilVal-%d" % i] for i in range(10) if spells[s]["Abil-%d" % i]}
            e = a.get(151, 0)
            if e and e in spells and e not in sp_nums:
                sp_nums.append(e)
                more = True
    sp_nums = sorted(sp_nums)

    SUBJ_COLS = ["Number", "Name", "RegenTime", "EXP", "ExpMulti", "HP", "ArmourClass", "DamageResist", "MagicRes", "BSDefense",
                 "Undead", "AvgLairExp", "HPRegen", "AvgDmg", "DeathSpell", "Summoned By"]

    def mon_row(n):
        m = mons[n]
        sub = n in solo
        row = [m[k] if sub or k in ("Number", "Undead", "BSDefense", "AvgDmg") else (0 if k != "Name" and k != "Summoned By" else "")
               for k in SUBJ_COLS]
        row[1] = (m["Name"] or "") if sub else ""
        row[15] = sb(m) if sub else ""
        row.append([[m["Abil-%d" % i], m["AbilVal-%d" % i]] for i in range(10)])
        row.append([[m["AttType-%d" % i], m["AttAcc-%d" % i], m["Att%%-%d" % i], m["AttTrue%%-%d" % i], m["AttHitSpell-%d" % i]]
                    for i in range(5)])
        row.append([[m["MidSpell-%d" % i], m["MidSpell%%-%d" % i]] for i in range(5)])
        return row

    LAIR_FIELDS = ["GroupIndex", "MobList", "Mobs", "TotalLairs", "AvgDelay", "AvgWalk", "AvgExp", "AvgDmg", "AvgHP",
                   "AvgAC", "AvgDR", "AvgMR", "AvgDodge"]
    SPELL_FIELDS = ["Number", "AttType", "Dur", "DurIncLVLs", "DurInc"]
    table = {
        "monster_fields": SUBJ_COLS + ["Abilities(ability,value)[10]", "Attacks(type,acc,pct,truepct,hitspell)[5]",
                                       "MidSpells(spell,pct)[5]"],
        "monsters": [mon_row(n) for n in sub_nums],
        "lair_fields": LAIR_FIELDS,
        "lairs": [[lairs[g][k] for k in LAIR_FIELDS] for g in groups if g in lairs],
        "spell_fields": SPELL_FIELDS + ["Abilities(ability,value)[10]"],
        "spells": [[spells[s][k] for k in SPELL_FIELDS] + [[[spells[s]["Abil-%d" % i], spells[s]["AbilVal-%d" % i]] for i in range(10)]]
                   for s in sp_nums],
    }

    # ---- damage caches (invented, fixed): -1 for a share of the monsters
    def damage_array(salt, lo, hi, miss):
        a = [-1] * (top + 1)
        for n in sub_nums:
            if H(salt, "miss", n) % miss:
                base = mons[n]["AvgDmg"]
                a[n] = rnd(base * (lo + (H(salt, "f", n) % int((hi - lo) * 10 + 1)) / 10.0) + (H(salt, "a", n) % 3) * 0.5, 1)
        return a

    vs_char = damage_array("vc", 0.3, 1.6, 5)
    vs_party = damage_array("vp", 0.2, 2.0, 4)
    vs_default = damage_array("vd", 0.5, 1.2, 6)

    # ---- nMonsterPossy / nMonsterSpawnChance as CalculateAverageLairs fills them (data 1.83)
    possy = [0.0] * (top + 1)
    chance = [0.0] * (top + 1)
    for n, m in mons.items():
        es = entries(m)
        if not es or "(lair)" not in sb(m):
            continue
        total, ch = 0.0, 0.0
        for g, r in es:
            total += r
            nm = len(mobs_of(g))
            if r > 0 and nm > 0:
                ch += rnd(1 - (1 - 1.0 / nm) ** r, 2)
        if total > 0:
            possy[n] = rnd(total / len(es), 1)
        chance[n] = rnd(ch / len(es), 2)

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

    def lair_forms(size, anti=0, hp="60", regen="20"):
        lt = {0: str(size), 1: hp, 2: regen, 3: "50", 4: "50", 5: "400", 6: str(anti), 7: "12", 8: "250", 9: "1"}
        return {"optMonsterFilter": arr({1: {"Value": True}}),
                "txtMonsterLairFilter": arr({k: {"Text": v} for k, v in lt.items()}),
                "txtMonsterDamage": {"Text": "55"},
                "txtMonsterDamageOUT": arr({0: {"Text": "140"}, 1: {"Text": "60"}})}

    def prof():
        return {"udt": "tCharacterProfile", "fields": {}, "out": True}

    cases = []

    def case(cid, who, steps, forms=None, glob_=None):
        """who: a CH key, or None for "no character" (global filter off, the default defender)."""
        base, gl = CH[who or "warrior"]
        g = char_globals(base, **gl)
        g.update(glob_ or {})
        f = char_forms(base, **(forms or {}))
        if who is None:
            f["chkGlobalFilter"] = {"Value": 0}
            f.update({k: v for k, v in (forms or {}).items()})
        pre = [{"call": "RefreshAll"}, {"call": "SetCurrentAttackTypeConfig"}, {"call": "LoadLairInfo"},
               {"call": "PopulateCharacterProfile", "args": [prof()], "as": "tchar"}]
        cases.append({"id": cid, "forms": {"frmMain": f}, "globals": g, "steps": pre + steps})

    def rows(as_, keys, extra_args=(), lv="each", hwnd=None):
        spec = {"table": "tabMonsters", "index": "pkMonsters", "keys": list(keys), "lv": lv}
        if hwnd is not None:
            spec["hwnd"] = hwnd
        return {"rows": spec, "call": "AddMonster2LV", "args": [{"lv": True}, {"saved": "tchar"}] + list(extra_args), "as": as_}

    # solo mode ("all" monsters): no lair calls at all
    case("nochar-solo", None, [rows("rows", solo)])
    case("nochar-solo-expmulti", None, [rows("rows", solo[::2])], glob_={"UseExpMulti": True})
    for who in ("warrior", "mage", "mystic"):
        case(who + "-solo", who, [rows("rows", solo)])
    case("warrior-solo-expmulti", "warrior", [rows("rows", solo[1::2])], glob_={"UseExpMulti": True})

    # lair mode, party of one (exp/hr per monster)
    case("nochar-lair", None, [rows("rows", lairmode)], forms=lair_forms(1))
    for who in ("warrior", "mage", "mystic"):
        case(who + "-lair", who, [rows("rows", lairmode)], forms=lair_forms(1))
    case("rogue-lair", "rogue", [rows("rows", LAIR_SMALL)], forms=lair_forms(1))
    case("warrior-lair-blank-party", "warrior", [rows("rows", LAIR_SMALL)], forms=dict(lair_forms(1), txtMonsterLairFilter=arr(
        {k: {"Text": ""} for k in range(10)})))

    # party mode
    for who in ("warrior", "mage"):
        for size, anti in ((2, 0), (4, 2), (6, 6)):
            case("%s-party%d" % (who, size), who, [rows("rows", lairmode if size == 4 else LAIR_SMALL)], forms=lair_forms(size, anti))
    case("nochar-party3", None, [rows("rows", LAIR_SMALL)], forms=lair_forms(3, 1))
    case("warrior-party-over", "warrior", [rows("rows", LAIR_SMALL[:6])], forms=lair_forms(9, 0))

    # the compare list is not lvMonsters: no lair columns
    case("warrior-compare", "warrior", [rows("rows", LAIR_SMALL, hwnd=77)], forms=lair_forms(1))
    case("mage-compare-party", "mage", [rows("rows", LAIR_SMALL, hwnd=77)], forms=lair_forms(3, 1))

    scn = {
        "family": "monster_rows",
        "description": "Monsters list row (FAM-21, stock): AddMonster2LV in solo, lair and party modes, with the lair averages, "
                       "the damage columns and exp/hr through CalcExpPerHour.",
        "notes": [],
        "database": True,
        "frm": PIPE["frm"],
        "tolerance": {"default": 0},
        "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "UseExpMulti": False,
                    "nMonsterDamageVsChar": vs_char, "nMonsterDamageVsParty": vs_party, "nMonsterDamageVsDefault": vs_default,
                    "nMonsterPossy": possy, "nMonsterSpawnChance": chance,
                    "bGlobal_cephModelA": True, "bGlobal_cephModelB": True, "bGlobal_cephModelC": True, "bGlobal_cephModelD": True,
                    "bGlobal_cephRecoveryOnly": False, "bGlobal_cephShowAll": False,
                    "nGlobal_cephXP_Knob": 1.0, "nGlobal_cephDMG_Knob": 1.0, "nGlobal_cephMana_Knob": 1.0,
                    "nGlobal_cephMove_Knob": 1.0,
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
    print("cases", len(cases), "solo", len(solo), "lair", len(lairmode), "groups", len(groups), "mobs", len(mob_nums),
          "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

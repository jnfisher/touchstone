#!/usr/bin/env python3
"""Writes fixtures/scenarios/monster_detail.json (FAM-20, monsters; card P3-32).

    python3 -I tools/oracle/gen/monster_detail.py [DB.sqlite]

modMain PullMonsterDetail fills the Monsters detail pane: the record's attributes, abilities, drops, the mob's attacks,
the damage the mob does, the damage the character does it (a weapon, a spell, martial arts, a backstab opener, a
party or a manual figure), the lair averages, the scripting estimate (exp/hr) and the "Spawns via" locations. Each
case sets a character up with the oracle's own RefreshAll (a case of the pipeline scenario), runs
SetCurrentAttackTypeConfig and LoadLairInfo as the explorer does, and then builds the panes of many monsters into
recording ListViews with a `rows` step (`"lv": "list"`: every row, with its Bold flag; `{"key": true}`: the
record number). AddMonsterReachRows is stubbed (the reachability module is optional); LookUpMonsterRegen is cut from
frmMain with the pipeline's other procedures.

Modes: no character (global filter off: the generic defender), a weapon user, a backstabber, a caster, a martial
artist and the other attack choices, solo and party. The monsters are stratified (spells, drops, lairs, regen, undead,
non-hostile, summons, guards, greet text, exp 0 ...). The damage-versus-character arrays are invented but fixed values.
The "data" key holds the rows the Rust test builds its World from: the subject monsters in full, the lair mobs and the
other monsters they name in part, the spells, items, rooms, shops and text blocks the panes reach.
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
OUT = os.path.join(ROOT, "fixtures", "scenarios", "monster_detail.json")
PIPE = json.load(open(os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")))
PAT = re.compile(r"\[([\d\-]+)\]\[(\d+)\]Group\(lair\): (\d+)/(\d+)", re.A)
KEY = re.compile(r"(?i)(room |monster #|textblock #|textblock\(rndm\) #|item #|spell #|shop #|shop\(sell\) #|shop\(nogen\) #|"
                 r"group\(lair\): |group: |npc #)(\d+(?:/\d+)?)")
PAIR = re.compile(r"(\d+)/(\d+)")

N_NOCHAR = int(os.environ.get("MD_NOCHAR", 46))
N_WARRIOR = int(os.environ.get("MD_WARRIOR", 12))
N_MAGE = int(os.environ.get("MD_MAGE", 10))
SUMMON_MAX = 900     # longest Summoned By text a subject may have (the "Spawns via" rows and the lair table grow with it)


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def rnd(x, n):
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
    items = {r["Number"]: dict(r) for r in c.execute("select * from Items order by Number")}
    shops = {r["Number"]: dict(r) for r in c.execute("select * from Shops order by Number")}
    rooms = {(r["Map Number"], r["Room Number"]): dict(r) for r in c.execute("select * from Rooms")}
    tbinfo = {r["Number"]: dict(r) for r in c.execute("select * from TBInfo order by Number")}
    classes = {r["Number"]: r["Name"] for r in c.execute("select Number, Name from Classes")}
    top = max(mons)
    mon_cols = [r[1] for r in c.execute("pragma table_info(Monsters)")]
    spell_cols = [r[1] for r in c.execute("pragma table_info(Spells)")]
    item_cols = [r[1] for r in c.execute("pragma table_info(Items)")]
    shop_cols = [r[1] for r in c.execute("pragma table_info(Shops)")]

    def abil(m):
        return {m["Abil-%d" % i]: m["AbilVal-%d" % i] for i in range(10) if m["Abil-%d" % i]}

    def sb(m):
        return m["Summoned By"] or ""

    def entries(m):
        return [(g, int(r)) for g, r, _, _ in PAT.findall(sb(m))]

    def mobs_of(g):
        L = lairs.get(g)
        return [int(x) for x in L["MobList"].split(",") if x.strip().isdigit() and int(x) in mons] if L else []

    def spells_of(m):
        out = set()
        for i in range(5):
            if m["AttType-%d" % i] == 2 and m["AttAcc-%d" % i]:
                out.add(m["AttAcc-%d" % i])
            if m["AttHitSpell-%d" % i]:
                out.add(m["AttHitSpell-%d" % i])
            if m["MidSpell-%d" % i]:
                out.add(m["MidSpell-%d" % i])
        for k in ("DeathSpell", "CreateSpell"):
            if m[k]:
                out.add(m[k])
        return out

    named = [n for n, m in mons.items() if m["Name"] and not m["Name"].startswith("sdf")]
    skipped = [n for n, m in mons.items() if not m["Name"] or m["Name"].startswith("sdf")]
    short = {n for n in named if len(sb(mons[n])) <= SUMMON_MAX}

    def area_spell(m):
        for i in range(5):
            if m["AttType-%d" % i] == 2:
                s = spells.get(m["AttAcc-%d" % i])
                if s and s["Targets"] == 12:
                    return True
        return False

    S = {
        "plain": [n for n in named if not sb(mons[n]) and mons[n]["RegenTime"] == 0],
        "regen": [n for n in named if mons[n]["RegenTime"] > 0 and "(lair)" not in sb(mons[n]) and "Room" not in sb(mons[n])],
        "room": [n for n in named if "Room" in sb(mons[n]) and "(lair)" not in sb(mons[n])],
        "room_regen": [n for n in named if mons[n]["RegenTime"] > 0 and "Room" in sb(mons[n])],
        "spellsummon": [n for n in named if "Spell #" in sb(mons[n])],
        "lair1": [n for n in named if len(entries(mons[n])) == 1 and mons[n]["RegenTime"] == 0],
        "lair2": [n for n in named if len(entries(mons[n])) == 2 and mons[n]["RegenTime"] == 0],
        "lair3": [n for n in named if 3 <= len(entries(mons[n])) <= 5 and mons[n]["RegenTime"] == 0],
        "lair6": [n for n in named if len(entries(mons[n])) >= 6 and mons[n]["RegenTime"] == 0],
        "lair_regen": [n for n in named if len(entries(mons[n])) >= 1 and mons[n]["RegenTime"] > 0],
        "magic": [n for n in named if abil(mons[n]).get(28, 0) > 0],
        "dodge": [n for n in named if abil(mons[n]).get(34, 0) > 0],
        "antimagic": [n for n in named if 51 in abil(mons[n])],
        "nonliving": [n for n in named if 109 in abil(mons[n])],
        "animal": [n for n in named if 78 in abil(mons[n])],
        "spellimmu": [n for n in named if abil(mons[n]).get(139, 0) > 0],
        "resist": [n for n in named if any(a in abil(mons[n]) for a in (3, 5, 65, 66, 147))],
        "guard": [n for n in named if 146 in abil(mons[n])],
        "exp0": [n for n in named if mons[n]["EXP"] <= 1],
        "expmulti": [n for n in named if mons[n]["ExpMulti"] not in (1.0,) and mons[n]["EXP"] > 0],
        "undead": [n for n in named if mons[n]["Undead"] == 1],
        "bsdef": [n for n in named if mons[n]["BSDefense"] > 0],
        "spellatk": [n for n in named if any(mons[n]["AttType-%d" % i] == 2 for i in range(5))],
        "areaspell": [n for n in named if area_spell(mons[n])],
        "hitspell": [n for n in named if any(mons[n]["AttHitSpell-%d" % i] for i in range(5))],
        "midspell": [n for n in named if any(mons[n]["MidSpell-%d" % i] for i in range(5))],
        "deathspell": [n for n in named if mons[n]["DeathSpell"]],
        "createspell": [n for n in named if mons[n]["CreateSpell"]],
        "greet": [n for n in named if mons[n]["GreetTXT"] > 0],
        "drops": [n for n in named if any(mons[n]["DropItem-%d" % i] for i in range(10))],
        "weapon": [n for n in named if mons[n]["Weapon"]],
        "coin": [n for n in named if mons[n]["R"] or mons[n]["P"] or mons[n]["G"]],
        "limit": [n for n in named if mons[n]["GameLimit"] > 0],
        "hp0": [n for n in named if mons[n]["HP"] < 1],
        "nothostile": [n for n in named if mons[n]["Align"] in (0, 3, 4)],
        "lawfulevil": [n for n in named if mons[n]["Align"] == 6],
        "noenergy": [n for n in named if mons[n]["Energy"] == 0],
        "attnamed": [n for n in named if any(mons[n]["AttName-%d" % i].strip() for i in range(5))],
        "dbl_attacks": [n for n in named if sum(1 for i in range(5) if 1 <= mons[n]["AttType-%d" % i] <= 3) >= 4],
    }
    for k in S:
        S[k] = [n for n in S[k] if n in short]

    def take(stratum, n, salt=""):
        return sorted(S[stratum], key=lambda x: H(stratum, salt, x))[:n]

    # the monsters every mode shows: the strata first (two of each, three of the big ones), then a hash-ordered fill
    subj = set(skipped[:2])
    quota = {"lair1": 6, "lair2": 5, "lair3": 5, "lair6": 3, "lair_regen": 4, "plain": 4, "regen": 4, "room": 4, "room_regen": 4,
             "exp0": 3, "undead": 3, "guard": 3, "spellatk": 5, "areaspell": 3, "hitspell": 3, "midspell": 4, "drops": 3, "weapon": 3,
             "greet": 2, "bsdef": 3, "dodge": 3, "antimagic": 3, "magic": 4}
    for k, n in quota.items():
        subj.update(take(k, n))
    for k in S:
        if k not in quota:
            subj.update(take(k, 2))
    rest = sorted((n for n in named if n in short and n not in subj), key=lambda x: H("fill", x))
    nochar = sorted(subj)
    for n in rest:
        if len(nochar) >= N_NOCHAR:
            break
        nochar.append(n)
    nochar = sorted(nochar)[:N_NOCHAR + 2]

    def pick(seq, n, salt):
        return sorted(sorted(seq, key=lambda x: H(salt, x))[:n])

    lairy = [n for n in nochar if "(lair)" in sb(mons[n])]
    # per-mode subsets (every one is a subset of nochar so the data closure is one set)
    warrior_set = sorted(set(pick([n for n in nochar if n in set(lairy)], 12, "w1")) | set(pick(nochar, N_WARRIOR, "w2")))[:N_WARRIOR]
    mage_set = sorted(set(pick(lairy, 8, "m1")) | set(pick(nochar, N_MAGE, "m2")))[:N_MAGE]
    spellies = [n for n in nochar if n in set(S["spellatk"])]
    small = pick(lairy, 2, "small") + pick([n for n in nochar if n not in set(lairy)], 2, "small2")
    small = sorted(set(small))
    party_set = sorted(set(pick(lairy, 8, "p1")) | set(pick(nochar, 12, "p2")))[:8]

    evil_set = pick(S["lawfulevil"] + S["nothostile"][:3], 6, "ev") or small[:2]

    # ---- data the Rust test rebuilds the world from
    groups = sorted({g for n in nochar + evil_set for g, _ in entries(mons[n])})
    mob_nums = sorted({m for g in groups for m in mobs_of(g)})
    guard_nums = sorted({mons[n]["AbilVal-%d" % i] for n in nochar + evil_set for i in range(10)
                         if mons[n]["Abil-%d" % i] == 146 and mons[n]["AbilVal-%d" % i] in mons})
    # records named by the Summoned By texts ("Spawns via ...")
    need = {"room": set(), "mon": set(), "item": set(), "spell": set(), "shop": set()}
    seen = set()
    work = [sb(mons[n]) for n in nochar + evil_set]
    while work:
        t = work.pop().lower()
        if t in seen:
            continue
        seen.add(t)
        for p in PAIR.finditer(t):
            need["room"].add((int(p.group(1)), int(p.group(2))))
        for m in KEY.finditer(t):
            k, v = m.group(1).lower(), m.group(2)
            if "/" in v:
                continue
            v = int(v)
            if k.startswith("monster") or k.startswith("npc"):
                need["mon"].add(v)
            elif k.startswith("item"):
                if v not in need["item"]:
                    need["item"].add(v)
                    if v in items and items[v]["ItemType"] == 8:
                        work.append(items[v]["Obtained From"] or "")
            elif k.startswith("spell"):
                need["spell"].add(v)
            elif k.startswith("shop"):
                if v not in need["shop"]:
                    need["shop"].add(v)
                    if v in shops:
                        work.append(shops[v]["Assigned To"] or "")
    sp_nums = set()
    it_nums = set(need["item"])
    mon_names = set(need["mon"])
    tb_nums = set()
    for n in nochar + evil_set:
        m = mons[n]
        sp_nums |= spells_of(m)
        if m["Weapon"]:
            it_nums.add(m["Weapon"])
        it_nums |= {m["DropItem-%d" % i] for i in range(10) if m["DropItem-%d" % i]}
        if m["GreetTXT"] > 0:
            tb_nums.add(m["GreetTXT"])
    sp_nums |= need["spell"]
    for pc in PIPE["cases"]:           # the characters' attack and heal spells (their names are in the panes)
        g = pc.get("globals", {})
        for k in ("nGlobalAttackSpellNum", "nGlobalAttackHealSpellNum"):
            if g.get(k) in spells:
                sp_nums.add(g[k])
    more = True
    while more:                 # the spells, items, monsters and text blocks that spell text names
        more = False
        for s in list(sp_nums):
            sp = spells.get(s)
            if not sp:
                continue
            for i in range(10):
                a, v = sp["Abil-%d" % i], sp["AbilVal-%d" % i]
                lo, hi = sp["MinBase"], sp["MaxBase"]
                span = range(lo, hi + 1) if 0 < lo < hi and hi - lo <= 10 else [lo]
                if a in (43, 153, 151, 42, 122, 160):
                    for v2 in [v] + (list(span) if a in (43, 153, 151) else []):
                        if v2 in spells and v2 not in sp_nums:
                            sp_nums.add(v2)
                            more = True
                elif a == 12:
                    for v2 in [v] + list(span):
                        if v2 in mons and v2 not in mon_names:
                            mon_names.add(v2)
                    continue
                elif a in (12, 146) and v in mons and v not in mon_names:
                    mon_names.add(v)
                elif a in (185, 1115) and v in items:
                    it_nums.add(v)
                elif a == 148 and v in tbinfo:
                    tb_nums.add(v)
        for t in list(tb_nums):
            act = (tbinfo[t]["Action"] or "") if t in tbinfo else ""
            for part in act.split(":"):
                if part.lower().startswith("cast "):
                    mnum = re.match(r"\s*(\d+)", part[5:])
                    if mnum and int(mnum.group(1)) in spells and int(mnum.group(1)) not in sp_nums:
                        sp_nums.add(int(mnum.group(1)))
                        more = True
    shop_nums = set(need["shop"])
    for s in list(shop_nums):
        if s in shops:
            it_nums |= {shops[s]["Item-%d" % x] for x in range(20) if shops[s]["Item-%d" % x]}

    def mon_row(n, full):
        m = mons[n]
        keep = {"Number", "Name", "RegenTime", "EXP", "ExpMulti", "HP", "ArmourClass", "DamageResist", "MagicRes", "BSDefense",
                "Undead", "AvgLairExp", "HPRegen", "AvgDmg", "DeathSpell"}
        out = []
        for k in mon_cols:
            v = m[k]
            if not full and not (k in keep or k.startswith(("Abil-", "AbilVal-", "AttType-", "AttAcc-", "Att%-", "AttTrue%-",
                                                            "AttHitSpell-", "MidSpell-", "MidSpell%-"))):
                v = "" if isinstance(m[k], str) or m[k] is None and k in ("Name", "Summoned By") else 0
            out.append(v if v is not None else "")
        if not full:
            out[mon_cols.index("Name")] = m["Name"] or ""
        return out

    mon_set = set(nochar) | set(evil_set)
    other_mons = sorted((set(mob_nums) | set(guard_nums) | mon_names | {v for g in groups for v in mobs_of(g)}) - mon_set)
    table = {
        "monster_cols": mon_cols,
        "monsters": [mon_row(n, True) for n in sorted(mon_set)] + [mon_row(n, False) for n in other_mons],
        "spell_cols": spell_cols,
        "spells": [[("" if spells[s][k] is None else spells[s][k]) for k in spell_cols] for s in sorted(sp_nums) if s in spells],
        "item_cols": item_cols,
        "items": [[("" if items[i][k] is None else items[i][k]) for k in item_cols] for i in sorted(it_nums) if i in items],
        "shop_cols": shop_cols,
        "shops": [[("" if shops[s][k] is None else shops[s][k]) for k in shop_cols] for s in sorted(shop_nums) if s in shops],
        "rooms": [[mk[0], mk[1], rooms[mk]["Name"] or "", rooms[mk]["Delay"]] for mk in sorted(need["room"]) if mk in rooms],
        "tbinfo": [[t, tbinfo[t]["LinkTo"], tbinfo[t]["Action"] or "", tbinfo[t]["Called From"] or ""] for t in sorted(tb_nums)
                   if t in tbinfo],
        "classes": [[k, v] for k, v in sorted(classes.items())],
        "lair_cols": ["GroupIndex", "MobList", "Mobs", "TotalLairs", "AvgDelay", "AvgWalk", "AvgExp", "AvgDmg", "AvgHP", "AvgAC",
                      "AvgDR", "AvgMR", "AvgDodge"],
        "lairs": [[lairs[g][k] for k in ("GroupIndex", "MobList", "Mobs", "TotalLairs", "AvgDelay", "AvgWalk", "AvgExp", "AvgDmg",
                                         "AvgHP", "AvgAC", "AvgDR", "AvgMR", "AvgDodge")] for g in groups if g in lairs],
    }

    # ---- damage caches (invented, fixed): -1 for a share of the monsters
    sub_nums = sorted(mon_set | set(other_mons))

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

    def char_forms(base, **more_):
        f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
        f.update(more_)
        return f

    def char_globals(base, **more_):
        g = json.loads(json.dumps(PC[base]["globals"]))
        g.update(more_)
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

    cases = []

    def case(cid, who, steps, forms=None, glob_=None, frm_vars=None):
        """who: a CH key, or None for "no character" (global filter off, the default defender)."""
        base, gl = CH[who or "warrior"]
        g = char_globals(base, **gl)
        g.update(glob_ or {})
        f = char_forms(base, **(forms or {}))
        if who is None:
            f["chkGlobalFilter"] = {"Value": 0}
            f.update({k: v for k, v in (forms or {}).items()})
        f.update(frm_vars or {})
        pre = [{"call": "RefreshAll"}, {"call": "SetCurrentAttackTypeConfig"}, {"call": "LoadLairInfo"}]
        cases.append({"id": cid, "forms": {"frmMain": f}, "globals": g, "objects": SIM, "steps": pre + steps})

    SIM = {"sim": {"class": "clsMonsterAttackSim", "global": "clsMonAtkSim"}}

    def panes(as_, keys, limit=8):
        spec = {"table": "tabMonsters", "index": "pkMonsters", "keys": list(keys), "lv": "list"}
        args = [{"key": True}, {"lv": True}] + ([limit] if limit is not None else [])
        return {"rows": spec, "call": "PullMonsterDetail", "args": args, "as": as_}

    # no character: the generic defender, the global filter off
    case("nochar", None, [panes("rows", nochar)])
    case("nochar-fulllocations", None, [panes("rows", pick(lairy, 3, "full"), 100)])
    case("nochar-expmulti", None, [panes("rows", pick(nochar, 6, "em"))], glob_={"UseExpMulti": True})
    case("nochar-hide-numbers", None, [panes("rows", small)], glob_={"bHideRecordNumbers": True})
    case("nochar-nolookup", None, [panes("rows", small)], frm_vars={"bDontLookupMonRegen": True})
    case("nochar-limit", None, [panes("rows", pick(lairy, 4, "lim"), 3)])
    case("nochar-show-all", None, [panes("rows", small[:6])], glob_={"bGlobal_cephShowAll": True})

    # a weapon user, a backstabber, a caster and a martial artist
    case("warrior", "warrior", [panes("rows", warrior_set)])
    case("rogue", "rogue", [panes("rows", pick(warrior_set, 10, "rg"))])
    case("mage", "mage", [panes("rows", mage_set)])
    case("mystic", "mystic", [panes("rows", pick(mage_set, 10, "my"))])
    case("warrior-evil", "warrior", [panes("rows", evil_set)],
         forms={"cmbGlobalAlignment": {"ListIndex": 3}})
    case("warrior-expmulti", "warrior", [panes("rows", pick(warrior_set, 4, "wem"))], glob_={"UseExpMulti": True})
    # the other attack choices: one-shot (0), spell any level (3), manual (5), bash (6), smash (7)
    case("oneshot", "warrior", [panes("rows", small)], glob_={"nGlobalAttackTypeMME": 0})
    case("spellany", "mage", [panes("rows", small)], glob_={"nGlobalAttackTypeMME": 3, "nGlobalAttackSpellLVL": 20})
    case("manual", "warrior", [panes("rows", small)], glob_={"nGlobalAttackTypeMME": 5, "nGlobalAttackManualP": 90,
                                                              "nGlobalAttackManualM": 40})
    case("bash", "warrior", [panes("rows", small)], glob_={"nGlobalAttackTypeMME": 6})
    case("smash", "warrior", [panes("rows", small[:2])], glob_={"nGlobalAttackTypeMME": 7})
    case("rogue-ma", "rogue", [panes("rows", small)], glob_={"nGlobalAttackTypeMME": 4, "nGlobalAttackMA": 2})
    case("rogue-ma-punch", "rogue", [panes("rows", small[:5])], glob_={"nGlobalAttackTypeMME": 4, "nGlobalAttackMA": 1})
    case("mage-nospell", "mage", [panes("rows", small[:5])], glob_={"nGlobalAttackSpellNum": 0})
    case("rogue-weapon-zero", "rogue", [panes("rows", small[:5])], glob_={"nGlobalAttackBackstabWeapon": 0})

    # the exp/hr text with the different settings
    case("warrior-show-all", "warrior", [panes("rows", small)], glob_={"bGlobal_cephShowAll": True})

    # lair-mode controls: print the character's damage first, and the party
    case("warrior-lair-first", "warrior", [panes("rows", small)], forms=lair_forms(1), glob_={"bMobPrintCharDamageOutFirst": True})
    case("mage-lair", "mage", [panes("rows", small)], forms=lair_forms(1))
    case("nochar-lair", None, [panes("rows", small)], forms=lair_forms(1))
    for size, anti in ((2, 0), (4, 2), (6, 6)):
        case("warrior-party%d" % size, "warrior", [panes("rows", party_set if size == 4 else small)], forms=lair_forms(size, anti))
    case("mage-party3", "mage", [panes("rows", small)], forms=lair_forms(3, 1))
    case("nochar-party3", None, [panes("rows", small[:5])], forms=lair_forms(3, 1))
    case("warrior-party-first", "warrior", [panes("rows", small[:5])], forms=lair_forms(2, 1),
         glob_={"bMobPrintCharDamageOutFirst": True})

    # the "stale" notes under the lair stats: the damage configuration texts and the party flags
    stale = {"sMonsterDamageVsCharDefenseConfig": "old", "sGlobalCharDefenseDescription": "new"}
    case("warrior-stale", "warrior", [panes("rows", small)], glob_=stale)
    case("warrior-notcalc", "warrior", [panes("rows", small)], glob_={"sMonsterDamageVsCharDefenseConfig": "",
                                                                       "sGlobalCharDefenseDescription": "something"})
    case("warrior-party-incomplete", "warrior", [panes("rows", small)], forms=lair_forms(3, 0),
         glob_={"bMonsterDamageVsPartyCalculated": False, "bDontPromptCalcPartyMonsterDamage": False})
    case("warrior-party-complete", "warrior", [panes("rows", small[:4])], forms=lair_forms(3, 0),
         glob_={"bMonsterDamageVsPartyCalculated": True, "bDontPromptCalcPartyMonsterDamage": True})

    # the damage the mob does the character, simulated on the spot instead of read from the cache
    case("warrior-autocalc", "warrior", [panes("rows", small[:6])], frm_vars={"bAutoCalcMonDamage": True})
    case("warrior-autocalc-party", "warrior", [panes("rows", small[:4])], forms=lair_forms(3, 1), frm_vars={"bAutoCalcMonDamage": True})

    # older data versions
    def possy_for(ver):
        out = [0.0] * (top + 1)
        for n, m in mons.items():
            es = entries(m)
            if es and "(lair)" in sb(m):
                tot = sum(r for _, r in es) if ver >= 1.82 else len(es)
                out[n] = rnd(tot / len(es), 1)
        return out

    zeros = [0.0] * (top + 1)
    for ver, tag in ((1.82, "v182"), (1.7, "v170")):
        g = {"nNMRVer": ver, "nMonsterPossy": possy_for(ver), "nMonsterSpawnChance": zeros}
        if ver < 1.8:
            g["nMonsterDamageVsDefault"] = [(round(mons[n]["AvgDmg"] * 0.9, 1) if n in mons else -1) for n in range(top + 1)]
        pick_n = small + pick(nochar, 3, "old" + tag)
        case("warrior-%s" % tag, "warrior", [panes("rows", sorted(set(pick_n)))], glob_=g)
        if ver < 1.8:
            case("nochar-%s" % tag, None, [panes("rows", small[:3])], glob_=g)

    frm = json.loads(json.dumps(PIPE["frm"]))
    frm["procs"] = frm["procs"] + ["LookUpMonsterRegen"]
    frm["stub_replace"] = ["AddMonsterReachRows"]

    scn = {
        "family": "monster_detail",
        "description": "Monsters detail pane (FAM-20, stock): PullMonsterDetail and AddMonsterDamageOutText into a recording "
                       "ListView, for no character, a weapon user, a caster and the other attack choices, solo and party.",
        "notes": ["AddMonsterReachRows is stubbed (the optional reachability module); the reach rows are not part of the fixture."],
        "database": True,
        "frm": frm,
        "tolerance": {"default": 0},
        "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "UseExpMulti": False, "nGlobalMonsterSimRounds": 500,
                    "bHideRecordNumbers": False, "bMobPrintCharDamageOutFirst": False,
                    "sMonsterDamageVsCharDefenseConfig": "same", "sGlobalCharDefenseDescription": "same",
                    "bMonsterDamageVsPartyCalculated": True, "bDontPromptCalcPartyMonsterDamage": True,
                    "nMonsterDamageVsChar": vs_char, "nMonsterDamageVsParty": vs_party, "nMonsterDamageVsDefault": vs_default,
                    "nMonsterPossy": possy, "nMonsterSpawnChance": chance,
                    "bGlobal_cephModelA": True, "bGlobal_cephModelB": True, "bGlobal_cephModelC": True, "bGlobal_cephModelD": True,
                    "bGlobal_cephRecoveryOnly": False, "bGlobal_cephShowAll": False,
                    "nGlobal_cephXP_Knob": 1.0, "nGlobal_cephDMG_Knob": 1.0, "nGlobal_cephMana_Knob": 1.0,
                    "nGlobal_cephMove_Knob": 1.0,
                    **{n: [-1] * (top + 1) for n in (
                        "nCharDamageVsMonster", "nCharFirstRoundDamageVsMonster", "nCharMinRoundDamageVsMonster",
                        "nCharSurpriseDamageVsMonster", "nCharSurpriseMinDamageVsMonster", "nCharSurpriseChanceVsMonster")}},
        "forms": dict(SETUP_FORMS, frmMain=dict(SETUP_FORMS["frmMain"], bAutoCalcMonDamage=False, bDontLookupMonRegen=False)),
        "data": {"table": table},
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "nochar", len(nochar), "warrior", len(warrior_set), "mage", len(mage_set), "groups", len(groups),
          "mobs", len(mob_nums), "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

"""Writes fixtures/scenarios/combat.json (FAM-08, damage vs monster and combat rounds, stock).

    python3 -I tools/oracle/gen/combat.py [DB.sqlite]

Procedures: GetDamageOutput, CalcCombatRounds (and its ccr_* helpers), IsMobKillable, CalcAverageNonZero.

Deterministic (splitmix-style counter, no clock). The case "table" carries the Monsters, Spells, Items,
Classes and Races rows the other cases touch (only the fields the Rust port reads), so the Rust test
needs no database. GetDamageOutput cases are grids of one character profile over a monster list
(profiles: melee, two-hander, maxed, bash, smash, martial arts, backstab, manual, one-shot, lair party,
forced character, old data version) plus seeded samples for spells and for explicit AC/DR/MR arguments.
Every case sets the whole of frmMain it needs (a control left out reads as the VB default).
"""
import json
import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DEFAULT_DB = os.path.expanduser(
    "~/.cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite")
M64 = 2**64 - 1
MISSING = {"t": "missing"}

SPELL_FIELDS = ["Number", "Name", "Short", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff",
                "TypeOfResists", "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc",
                "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc", "Learnable", "LearnedFromLen", "CastedByLen",
                "Classes"]
ITEM_FIELDS = ["Number", "Name", "Encum", "Accy", "StrReq", "Speed", "Min", "Max", "WeaponType", "AbilitiesFlat",
               "ItemType"]
MON_FIELDS = ["Number", "ArmourClass", "DamageResist", "MagicRes", "BSDefense", "Undead", "AbilitiesFlat"]
G_NAMES = ["Number", "Encum", "Accy", "Crit", "STR", "AGI", "MaxDmg", "BSaccy", "BSmindmg", "BSmaxdmg", "PunchSkill",
           "PunchAccy", "PunchDmg", "KickSkill", "KickAccy", "KickDmg", "JkSkill", "JkAccy", "JkDmg", "Stealth"]
STAT_ABILS = {7, 11, 13, 14, 15, 19, 34, 35, 36, 37, 38, 39, 40, 41, 42, 101, 102}
CLASSES = [1, 4, 6, 7, 8, 12, 14, 15]
RACES = [1, 2, 3, 5, 9, 13]


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def pick(self, xs):
        return xs[self.next() % len(xs)]

    def int(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)


def flat(ab):
    f = [x for pair in ab for x in pair]
    while f and f[-1] == 0:
        f.pop()
    return f


def load(db):
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    mons, spells, items, classes, races = {}, {}, {}, [], []
    for r in c.execute("select * from Monsters order by Number"):
        ab = [(r["Abil-%d" % i], r["AbilVal-%d" % i]) for i in range(10)]
        mons[r["Number"]] = {"row": [r["Number"], r["ArmourClass"], r["DamageResist"], r["MagicRes"], r["BSDefense"],
                                     r["Undead"], flat(ab)], "ab": ab, "ac": r["ArmourClass"], "dr": r["DamageResist"],
                             "mr": r["MagicRes"], "bs": r["BSDefense"], "undead": r["Undead"]}
    for r in c.execute("select * from Spells order by Number"):
        ab = []
        for i in range(10):
            ab += [r["Abil-%d" % i], r["AbilVal-%d" % i]]
        spells[r["Number"]] = {"row": [r["Number"], r["Name"] or "", r["Short"] or "", r["ReqLevel"], r["EnergyCost"],
                                       r["ManaCost"], r["MinBase"], r["MaxBase"], r["Diff"], r["TypeOfResists"],
                                       r["Targets"], r["Dur"], r["AttType"], r["Magery"], r["MageryLVL"], r["Cap"],
                                       r["MaxIncLVLs"], r["MaxInc"], r["MinIncLVLs"], r["MinInc"], r["DurIncLVLs"],
                                       r["DurInc"], r["Learnable"], len(r["Learned From"] or ""),
                                       len(r["Casted By"] or ""), r["Classes"] or "", ab],
                               "att": r["AttType"], "res": r["TypeOfResists"], "req": r["ReqLevel"],
                               "ab": [r["Abil-%d" % i] for i in range(10)]}
    for r in c.execute("select * from Items where ItemType = 1 order by Number"):
        ab = [(r["Abil-%d" % i], r["AbilVal-%d" % i]) for i in range(20)]
        items[r["Number"]] = {"row": [r["Number"], r["Name"] or "", r["Encum"], r["Accy"], r["StrReq"], r["Speed"],
                                      r["Min"], r["Max"], r["WeaponType"], flat(ab), r["ItemType"]],
                              "ab": ab, "wt": r["WeaponType"]}
    for r in c.execute("select * from Classes order by Number"):
        ab = []
        for i in range(10):
            ab += [r["Abil-%d" % i], r["AbilVal-%d" % i]]
        classes.append([r["Number"], r["CombatLVL"], ab])
    for r in c.execute("select * from Races order by Number"):
        races.append([r["Number"], [r["Abil-%d" % i] for i in range(10)]])
    return mons, spells, items, classes, races


def pick_monsters(mons, count):
    """A spread over AC, DR, MR, BSDefense, undead and every ability the function reads."""
    nums = sorted(mons)
    chosen = []

    def add(n):
        if n not in chosen:
            chosen.append(n)

    for ab in (28, 34, 51, 78, 109, 139, 3, 5, 65, 66, 147):
        have = [n for n in nums if any(a == ab for a, _ in mons[n]["ab"])]
        if not have:
            continue
        by_val = sorted(have, key=lambda n: max(v for a, v in mons[n]["ab"] if a == ab))
        add(by_val[0])
        add(by_val[-1])
        add(by_val[len(by_val) // 2])
    add([n for n in nums if mons[n]["undead"] == 1][0])
    add([n for n in nums if mons[n]["undead"] == 1][-1])
    for key in ("ac", "dr", "mr", "bs"):
        by = sorted(nums, key=lambda n: (mons[n][key], n))
        add(by[0])
        add(by[-1])
        add(by[len(by) // 2])
        add(by[len(by) * 3 // 4])
    # the unexplored middle: a stride over the rest
    k = 0
    stride = max(1, len(nums) // (count + 7))
    while len(chosen) < count and k < len(nums):
        add(nums[k])
        k += stride
    return sorted(chosen[:count])


def pick_spells(spells):
    """One or two spells for each combination of attack type, resist type, restriction ability and level."""
    seen, out = {}, []
    for n in sorted(spells):
        s = spells[n]
        restr = tuple(sorted({a for a in s["ab"] if a in (23, 80, 108)}))
        dmg = tuple(sorted({a for a in s["ab"] if a in (1, 8, 17, 18)}))
        key = (s["att"], s["res"], restr, dmg, s["req"] > 0)
        if seen.get(key, 0) < (2 if restr else 1):
            seen[key] = seen.get(key, 0) + 1
            out.append(n)
    return out


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def frm(p):
    f = {}
    f["chkGlobalFilter"] = {"Value": p.get("filter", 1)}
    f["lblDatVer"] = {"Caption": "v1.11p"}
    if p.get("lair"):
        f["optMonsterFilter"] = arr({1: {"Value": True}})
    if p.get("lairtxt"):
        f["txtMonsterLairFilter"] = arr({k: {"Text": v} for k, v in p["lairtxt"].items()})
    if p.get("mdmg") is not None:
        f["txtMonsterDamage"] = {"Text": p["mdmg"]}
    if p.get("dout"):
        f["txtMonsterDamageOUT"] = arr({0: {"Text": p["dout"][0]}, 1: {"Text": p["dout"][1]}})
    f["txtGlobalLevel"] = arr({0: {"Text": str(p["level"])}})
    f["cmbGlobalClass"] = arr({0: {"ItemData": [p["cls"]], "ListIndex": 0}})
    f["cmbGlobalRace"] = arr({0: {"ItemData": [p["race"]], "ListIndex": 0}})
    f["cmbGlobalAlignment"] = {"ListIndex": p.get("align", 2)}
    stat = {0: {"Caption": str(p["enc"][0])}, 1: {"Caption": str(p["enc"][1])}}
    for k, v in p["tag"].items():
        stat.setdefault(k, {})["Tag"] = str(v)
    f["lblInvenCharStat"] = arr(stat)
    f["lblInvenStats"] = arr({19: {"Tag": str(p.get("cs19", 2))}})
    f["txtCharStats"] = arr({k: {"Tag": str(v)} for k, v in p["cstat"].items()})
    for key, ctl, prop in (("hp", "lblCharMaxHP", "Tag"), ("rest", "lblCharRestRate", "Tag"),
                           ("med", "txtCharManaRegen", "Tag"), ("mana", "lblCharMaxMana", "Tag"),
                           ("mrate", "lblCharManaRate", "Tag"), ("sc", "lblCharSC", "Tag"),
                           ("bless", "lblCharBless", "Caption")):
        if key in p:
            f[ctl] = {prop: str(p[key])}
    return {"frmMain": f}


def main():
    db = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB
    mons, spells, items, classes, races = load(db)
    rng = Rng(1212)
    cases, grids, samples = [], [], []

    MON = pick_monsters(mons, 60)

    def sub(n, off=0):
        """n monsters spread over the list from offset off (so the profiles together cover all 60)."""
        step = max(1, len(MON) // n)
        return [MON[(off + k * step) % len(MON)] for k in range(n)]

    SPELLS = pick_spells(spells)

    # ---- weapon pool: no spell-casting weapons (their text is the attack family's)
    def has(n, a):
        return any(x == a for x, _ in items[n]["ab"])

    plain = [n for n in items if not has(n, 43) and items[n]["row"][8] in (0, 1, 2, 3)]
    pos116 = [n for n in plain if any(a == 116 and v > 0 for a, v in items[n]["ab"])]
    neg116 = [n for n in plain if any(a == 116 and v < 0 for a, v in items[n]["ab"])]
    stat_w = [n for n in plain if any(a in STAT_ABILS for a, _ in items[n]["ab"])]
    hit_magic_w = [n for n in plain if has(n, 142)]
    magic_w = [n for n in plain if has(n, 28)]
    twohand = [n for n in plain if items[n]["wt"] in (1, 3)]
    onehand = [n for n in plain if items[n]["wt"] in (0, 2)]
    W = {
        "pos": pos116[0], "pos2": pos116[len(pos116) // 2], "neg": neg116[0], "two": twohand[0], "two2": twohand[-1],
        "one": onehand[0], "one2": onehand[len(onehand) // 2], "stat": stat_w[0], "stat2": stat_w[-1],
        "hm": hit_magic_w[0] if hit_magic_w else onehand[1], "mag": magic_w[0] if magic_w else onehand[2],
    }
    used_items = set(W.values())

    def weapon_globals(w0, w1):
        g = {}
        for nm in G_NAMES:
            if nm == "Number":
                g["nGlobalCharWeaponNumber"] = [w0, w1]
                continue
            hi = {"Encum": 40, "Accy": 15, "Crit": 10, "STR": 8, "AGI": 8, "MaxDmg": 12, "BSaccy": 20,
                  "BSmindmg": 10, "BSmaxdmg": 15, "Stealth": 10}.get(nm, 10)
            g["nGlobalCharWeapon" + nm] = [rng.int(0, hi) if w0 else 0, rng.int(0, hi) if w1 else 0]
        return g

    def profile(level, cls, race, **kw):
        p = {
            "filter": 1, "level": level, "cls": cls, "race": race, "align": kw.pop("align", 2),
            "enc": kw.pop("enc", [rng.int(10, 120), 300]),
            "tag": {8: rng.int(10, 120), 7: rng.int(0, 25), 11: rng.int(0, 40), 30: rng.int(0, 25),
                    13: rng.int(0, 40), 14: rng.int(0, 20), 15: rng.int(0, 30), 19: rng.int(0, 80),
                    10: rng.int(60, 320), 12: kw.pop("hitmagic", 0), 31: rng.pick([0, 5, 12]),
                    33: rng.pick([0, 10, 25]), 34: rng.int(0, 20), 35: rng.int(0, 20), 36: rng.int(0, 20),
                    37: rng.int(0, 10), 38: rng.int(0, 10), 39: rng.int(0, 10), 40: rng.int(0, 15),
                    41: rng.int(0, 15), 42: rng.int(0, 15)},
            "cstat": {0: rng.int(60, 120), 1: rng.int(40, 120), 3: rng.int(50, 120), 5: rng.int(40, 100)},
            "hp": rng.int(80, 700), "rest": rng.int(5, 60), "med": rng.int(0, 20), "mana": rng.int(0, 400),
            "mrate": rng.int(0, 20), "sc": rng.int(0, 200), "bless": rng.pick([0, 6, 12]),
        }
        p.update(kw)
        return p

    def gl(w0, w1=0, **kw):
        g = {"nNMRVer": 1.84, "gAvgLevelMaxAllStats": 120.5, "nGlobalCharHitMagicNonWeapon": 0,
             "nGlobalCharQnDbonus": 0, "nGlobalCharAccyAbils": rng.int(0, 20), "nGlobalCharAccyOther": rng.int(0, 20),
             "nGlobalCharAccyItems": rng.int(0, 30), "nGlobalAttackHealValue": 0, "nGlobalAttackHealCost": 0.0,
             "nGlobalAttackTypeMME": 1, "bGlobalAttackBackstab": False}
        g.update(weapon_globals(w0, w1))
        g.update(kw)
        return g

    def args(mon=0, **kw):
        order = ["ac", "dr", "mr", "dodge", "flags", "speed", "immu", "mlvl", "aflags", "bsdef", "rcol", "rfir",
                 "rsto", "rlit", "rwat", "force"]
        out = [mon] + [kw.get(k, MISSING) for k in order]
        while out and out[-1] == MISSING:
            out.pop()
        return out

    def grid(name, p, g, mons_list, **argkw):
        grids.append({"name": name, "axes": {"mon": mons_list},
                      "case": {"forms": frm(p), "globals": g, "proc": "GetDamageOutput", "args": args("$mon", **argkw)}})

    # ======================================================================= melee
    grid("melee", profile(40, 1, 3, hitmagic=0), gl(W["pos"], 0), MON)
    grid("two", profile(100, 7, 5, hitmagic=3), gl(W["two"], 0, nGlobalCharAccyAbils=10), sub(8, 1))
    grid("maxed", profile(255, 15, 2, hitmagic=250, enc=[250, 300]), gl(W["stat"], W["one2"]), sub(8, 2))
    grid("stat2", profile(70, 6, 9, hitmagic=2), gl(W["stat2"], 0, nGlobalCharQnDbonus=8), sub(5, 3))
    grid("bash", profile(55, 12, 1, hitmagic=1), gl(W["two2"], 0, nGlobalAttackTypeMME=6), sub(5, 4))
    grid("smash", profile(80, 7, 13, hitmagic=250), gl(W["two"], 0, nGlobalAttackTypeMME=7), sub(5, 5))
    grid("noweapon", profile(30, 4, 3), gl(0, 0), sub(3, 6))
    # ======================================================================= martial arts
    for ma, nm in ((1, "punch"), (2, "kick"), (3, "jk"), (0, "ma0")):
        grid(nm, profile(60, 8, 3, hitmagic=0), gl(0, 0, nGlobalAttackTypeMME=4, nGlobalAttackMA=ma,
                                                  nGlobalCharHitMagicNonWeapon=rng.pick([0, 2, 180])), sub(5, 7 + ma))
    grid("kick-hm", profile(60, 14, 3), gl(0, 0, nGlobalAttackTypeMME=4, nGlobalAttackMA=2,
                                           nGlobalCharHitMagicNonWeapon=250), sub(5, 11))
    # ======================================================================= backstab
    grid("bs-cur", profile(60, 7, 3, hitmagic=0), gl(W["pos"], 0, bGlobalAttackBackstab=True,
                                                    nGlobalAttackBackstabWeapon=0), sub(12, 0))
    grid("bs-wpn", profile(60, 7, 3, hitmagic=0), gl(W["one"], 0, bGlobalAttackBackstab=True,
                                                    nGlobalAttackBackstabWeapon=W["pos2"],
                                                    nGlobalCharHitMagicNonWeapon=3), sub(12, 1))
    grid("bs-hm", profile(90, 7, 5), gl(W["hm"], W["one2"], bGlobalAttackBackstab=True,
                                         nGlobalAttackBackstabWeapon=W["mag"],
                                         nGlobalCharHitMagicNonWeapon=40), sub(6, 2))
    grid("bs-fists", profile(40, 14, 3), gl(0, 0, bGlobalAttackBackstab=True, nGlobalAttackBackstabWeapon=0,
                                            nGlobalCharHitMagicNonWeapon=5), sub(6, 3))
    grid("bs-two", profile(75, 7, 9), gl(W["two"], W["one"], bGlobalAttackBackstab=True,
                                          nGlobalAttackBackstabWeapon=W["two2"],
                                          nGlobalCharHitMagicNonWeapon=250), sub(6, 4))
    grid("bs-kick", profile(50, 7, 3), gl(0, 0, nGlobalAttackTypeMME=4, nGlobalAttackMA=2, bGlobalAttackBackstab=True,
                                          nGlobalAttackBackstabWeapon=W["neg"],
                                          nGlobalCharHitMagicNonWeapon=250), sub(6, 5))
    # ======================================================================= manual, one-shot
    grid("manual", profile(40, 1, 3), gl(W["pos"], 0, nGlobalAttackTypeMME=5, nGlobalAttackManualP=45,
                                         nGlobalAttackManualM=30), sub(4, 6))
    grid("manual-nofilter", profile(40, 1, 3, filter=0), gl(W["pos"], 0, nGlobalAttackTypeMME=5,
                                                           nGlobalAttackManualP=120, nGlobalAttackManualM=0),
         sub(4, 7))
    grid("manual-spell", profile(40, 1, 3), gl(0, 0, nGlobalAttackTypeMME=5, nGlobalAttackManualP=0,
                                               nGlobalAttackManualM=200, bGlobalAttackBackstab=True), sub(4, 8))
    grid("oneshot", profile(40, 1, 3), gl(W["pos"], 0, nGlobalAttackTypeMME=0, bGlobalAttackBackstab=True), sub(2, 9))
    # ======================================================================= lair party
    for size, nm, mode in ((2, "party2", 1), (4, "party4", 5), (6, "party6", 2), (9, "party9", 1), (3, "partymanual", 5)):
        lt = {0: str(size), 1: "60", 2: "20", 3: "50", 4: "50", 5: "400", 6: "1", 7: "12", 8: str(rng.int(120, 300)), 9: rng.pick(["0", "1", "2.5", "4", "9"])}
        grid(nm, profile(50, 1, 3, lair=True, lairtxt=lt, mdmg="55", dout=[str(rng.int(30, 200)), rng.pick(["0", "40", "150"])]),
             gl(W["pos"], 0, nGlobalAttackTypeMME=mode, nGlobalAttackManualP=77, nGlobalAttackManualM=15), sub(4, 10 + size),
             flags=rng.pick([0, 8]))
    # txtMonsterLairFilter(9) is the party's swings; it is outside PartyFilter's nine fields
    # ======================================================================= forced character
    lt = {0: "4", 1: "60", 2: "20", 3: "50", 4: "50", 5: "400", 6: "1", 7: "12", 8: "250"}
    grid("force", profile(50, 1, 3, lair=True, lairtxt=lt, mdmg="55", dout=["90", "40"]),
         gl(W["pos"], 0, nGlobalAttackTypeMME=1, bGlobalAttackBackstab=True), sub(4, 12), force=True)
    # ======================================================================= old data version
    grid("oldver", profile(40, 1, 3, hitmagic=0), gl(W["pos"], 0, nNMRVer=1.7, bGlobalAttackBackstab=True), sub(5, 13))

    # ======================================================================= spells
    def spell_profile(level, cls, **kw):
        return profile(level, cls, 3, mana=rng.int(100, 900), sc=rng.int(20, 250), **kw)

    for i, (lvl, cls) in enumerate([(40, 4), (100, 12), (20, 6)]):
        p = spell_profile(lvl, cls)
        p["tag"][33] = [0, 25, 60][i]
        samples.append({
            "name": "spell%d" % i, "seed": 4100 + i, "count": 10,
            "draw": {"mon": {"choice": MON}, "sp": {"choice": SPELLS}, "mode": {"choice": [2, 3]},
                     "lv": {"choice": [1, 10, 33, 80, 200, 0, -5]},
                     "af": {"choice": [0, 0, 0, 0, 1, 2, 4, 8, 6]}, "df": {"choice": [0, 0, 1, 2, 4, 8]}},
            "case": {"forms": frm(p), "globals": gl(0, 0, nGlobalAttackTypeMME="$mode", nGlobalAttackSpellNum="$sp",
                                                    nGlobalAttackSpellLVL="$lv"),
                     "proc": "GetDamageOutput", "args": args("$mon", flags="$df", aflags="$af")}})
    # spell with backstab and a bad spell number
    samples.append({
        "name": "spellbs", "seed": 4200, "count": 6,
        "draw": {"mon": {"choice": MON}, "sp": {"choice": [SPELLS[0], SPELLS[5], 99999, 0]}},
        "case": {"forms": frm(spell_profile(60, 4)), "globals": gl(W["pos"], 0, nGlobalAttackTypeMME=2,
                                                                   nGlobalAttackSpellNum="$sp",
                                                                   bGlobalAttackBackstab=True),
                 "proc": "GetDamageOutput", "args": args("$mon")}})

    # ======================================================================= explicit arguments (no monster row)
    for i, (mode, w) in enumerate([(1, W["pos"]), (4, 0), (2, 0), (5, 0)]):
        pp = spell_profile(50, 4) if mode == 2 else profile(50, 7, 3, hitmagic=rng.pick([0, 5, 250]))
        samples.append({
            "name": "explicit%d" % i, "seed": 4300 + i, "count": 5,
            "draw": {"ac": {"choice": [0, 10, 60, 250, 1500, 9999]}, "dr": {"choice": [0, 3, 40, 300, 9999]},
                     "mr": {"choice": [0, 20, 60, 100, 5000]}, "dodge": {"choice": [-1, -7, 0, 30, 200]},
                     "ml": {"choice": [0, 1, 5, 20]}, "bsd": {"choice": [0, 0, 100, 250]},
                     "imm": {"choice": [0, 0, 5, 40]}, "rc": {"choice": [0, 30, -20, 100]},
                     "df": {"choice": [0, 8, 1, 4]}},
            "case": {"forms": frm(pp), "globals": gl(w, 0, nGlobalAttackTypeMME=mode, nGlobalAttackMA=2,
                                                     nGlobalAttackSpellNum=SPELLS[3], nGlobalAttackManualP=60,
                                                     nGlobalAttackManualM=60, bGlobalAttackBackstab=(i == 0)),
                     "proc": "GetDamageOutput",
                     "args": args(0, ac="$ac", dr="$dr", mr="$mr", dodge="$dodge", flags="$df", immu="$imm",
                                  mlvl="$ml", bsdef="$bsd", rcol="$rc", rfir="$rc", rsto="$rc", rlit="$rc",
                                  rwat="$rc")}})
    # a monster number with no row, and a missing-record backstab
    cases.append({"id": "nomonster", "forms": frm(profile(40, 1, 3)), "globals": gl(W["pos"], 0),
                  "proc": "GetDamageOutput", "args": args(99999)})
    cases.append({"id": "nomonster-bs", "forms": frm(profile(40, 1, 3)),
                  "globals": gl(W["pos"], 0, bGlobalAttackBackstab=True), "proc": "GetDamageOutput",
                  "args": args(-3, ac=40, dr=5)})

    # ======================================================================= ccr_* helpers
    for x in (-5.0, -0.0, 0.0, 0.25, 0.5, 0.999999, 1.0, 1.5, 100.0):
        cases.append({"id": "sat-%s" % x, "proc": "ccr_Saturate", "args": [{"t": "f64", "v": x}]})
    for e0, e1, x in [(0.0, 0.6, -1.0), (0.0, 0.6, 0.0), (0.0, 0.6, 0.15), (0.0, 0.6, 0.3), (0.0, 0.6, 0.45),
                      (0.0, 0.6, 0.6), (0.0, 0.6, 2.0), (3.0, 8.0, 1.0), (3.0, 8.0, 3.0), (3.0, 8.0, 4.5),
                      (3.0, 8.0, 5.5), (3.0, 8.0, 7.9), (3.0, 8.0, 8.0), (3.0, 8.0, 12.0), (5.0, 5.0, 4.0),
                      (5.0, 5.0, 5.0), (5.0, 5.0, 6.0), (8.0, 3.0, 5.0), (0.0, 1.0, 0.5)]:
        cases.append({"id": "ss-%s-%s-%s" % (e0, e1, x), "proc": "ccr_SmoothStep",
                      "args": [{"t": "f64", "v": e0}, {"t": "f64", "v": e1}, {"t": "f64", "v": x}]})
    for a, b, t in [(0.0, 10.0, 0.0), (1.0, 0.5, 0.5), (2.0, 8.0, 1.0), (-4.0, 4.0, 0.25), (1.0, 0.7071, 0.75)]:
        cases.append({"id": "lerp-%s-%s-%s" % (a, b, t), "proc": "ccr_Lerp",
                      "args": [{"t": "f64", "v": a}, {"t": "f64", "v": b}, {"t": "f64", "v": t}]})
    for n, d, df in [(6.0, 3.0, 9.0), (6.0, 0.0, 9.0), (6.0, 0.0, MISSING), (-7.0, 2.0, MISSING), (0.0, 5.0, 1.0)]:
        a = [{"t": "f64", "v": n}, {"t": "f64", "v": d}]
        if df != MISSING:
            a.append({"t": "f64", "v": df})
        cases.append({"id": "sd-%s-%s-%s" % (n, d, "m" if df == MISSING else df), "proc": "ccr_SafeDiv", "args": a})
    for a, b in [(1.0, 2.0), (2.0, 1.0), (3.0, 3.0), (-1.0, -2.0)]:
        for pr in ("ccr_Min", "ccr_Max"):
            cases.append({"id": "%s-%s-%s" % (pr, a, b), "proc": pr, "args": [{"t": "f64", "v": a}, {"t": "f64", "v": b}]})

    # ======================================================================= CalcCombatRounds
    def ccr_args(dmg=-9999, hp=0, mdmg=-1, chp=0, regen=0, nm=1.0, ovr=0.0, sur=-9999.0, first=-9999):
        d = lambda v: {"t": "f64", "v": float(v)}
        return [dmg, hp, mdmg, chp, regen, d(nm), d(ovr), d(sur), first]

    def ccr_case(cid, **kw):
        cases.append({"id": cid, "proc": "CalcCombatRounds", "args": ccr_args(**kw)})

    ccr_case("ccr-defaults")
    # single-monster grid: damage x HP x regen x mob damage
    for dmg in (-9999, 0, 1, 7, 33, 120, 5000):
        for hp in (0, 1, 2, 30, 250, 3000):
            for regen in (0, 12):
                ccr_case("ccr-g-%d-%d-%d" % (dmg, hp, regen), dmg=dmg, hp=hp, regen=regen, mdmg=40, chp=300)
    # RTK against the regeneration tick (rounds near 16.2 = 0.9 x 18) and the 200-round cutoff
    for hp in (160, 161, 162, 163, 300, 330, 331, 400, 650, 1000, 1990, 2000, 2100, 4000):
        for regen in (0, 25, 400):
            ccr_case("ccr-r-%d-%d" % (hp, regen), dmg=10, hp=hp, regen=regen, mdmg=12, chp=150)
    # pack sizes, override RTK, first-round damage
    for nm in (0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0):
        for ovr in (0.0, 0.5, 2.0, 10.0):
            ccr_case("ccr-p-%s-%s" % (nm, ovr), dmg=40, hp=240, nm=nm, ovr=ovr, mdmg=30, chp=400, regen=20)
    for first in (-9999, -1, 0, 10, 39, 40, 99):
        for hp in (40, 60, 70, 100, 140):
            ccr_case("ccr-f-%d-%d" % (first, hp), dmg=40, hp=hp, first=first, mdmg=0, chp=0)
    # surprise opener: every edge of the regen and pack smoothsteps
    sur = []
    for nm in (1.0, 2.0, 3.0, 4.0, 5.5, 8.0, 10.0):
        for regen in (0, 10, 30, 80, 200):
            for s in (0.0, 15.0, 200.0, 4000.0):
                sur.append((nm, regen, s))
    for nm, regen, s in sur:
        ccr_case("ccr-s-%s-%d-%d" % (nm, regen, int(s)), dmg=25, hp=int(300 * nm), nm=nm, regen=regen, sur=s,
                 mdmg=20, chp=200)
    for ovr in (1.0, 3.0, 8.0):
        ccr_case("ccr-so-%s" % ovr, dmg=25, hp=600, nm=2.0, ovr=ovr, sur=100.0, mdmg=20, chp=200)
    ccr_case("ccr-s-nohp", dmg=25, hp=1, sur=100.0)
    ccr_case("ccr-s-nodmg", dmg=0, hp=500, sur=100.0, ovr=3.0)
    ccr_case("ccr-s-first", dmg=40, hp=70, sur=60.0, first=20)
    # the rounds-to-die and success branches
    for chp in (0, 1, 50, 5000, 99999):
        for mdmg in (-1, 0, 1, 7, 100, 4000):
            ccr_case("ccr-d-%d-%d" % (chp, mdmg), dmg=30, hp=300, mdmg=mdmg, chp=chp)
    # Long overflow of the regenerated HP, and a huge fight
    ccr_case("ccr-ovf", dmg=1, hp=2147483000, regen=2147483000, mdmg=1, chp=10)
    ccr_case("ccr-huge", dmg=3, hp=900000, regen=5, mdmg=2, chp=900)
    ccr_case("ccr-pack-round", dmg=7, hp=1000, nm=3.0, mdmg=9, chp=100)
    ccr_case("ccr-pack-half", dmg=7, hp=1001, nm=2.0, mdmg=9, chp=100)

    # ======================================================================= IsMobKillable
    def k(cid, cdmg, chp, mdmg, mhp, cregen=0, mregen=0):
        cases.append({"id": cid, "proc": "IsMobKillable",
                      "args": [{"t": "f64", "v": float(cdmg)}, chp, {"t": "f64", "v": float(mdmg)}, mhp, cregen, mregen]})

    for cdmg in (-5, 0, 0.5, 8, 40, 400):
        for mhp in (-10, 0, 1, 50, 900, 20000):
            k("k-%s-%d" % (cdmg, mhp), cdmg, 300, 30, mhp)
    for mhp in (200, 1000, 5000, 12000, 30000):
        for mregen in (0, 6, 40, 500, 3000):
            for cdmg in (15, 60):
                k("kr-%d-%d-%d" % (mhp, mregen, cdmg), cdmg, 400, 25, mhp, 0, mregen)
    for chp in (1, 100, 5000):
        for mdmg in (-4, 0, 0.5, 20, 400):
            for cregen in (0, 18, 120, 32000):
                k("kd-%d-%s-%d" % (chp, mdmg, cregen), 30, chp, mdmg, 500, cregen, 0)
    k("k-ovf-regen", 10, 100, 5, 2000000000, 0, 2000000000)
    k("k-ovf-char", 10, 2000000000, 0.5, 100, 32000, 0)
    k("k-regen-short", 100, 50, 10, 120, 0, 90)
    k("k-regen-tie", 12.8, 100, 1, 100, 0, 40)

    # ======================================================================= CalcAverageNonZero
    for i, vals in enumerate([[0.0], [5.0], [0.0, 0.0, 0.0], [1.0, 0.0, 3.0, 0.0, 8.0], [-4.0, 4.0, 10.0],
                              [2.5, 0.1, 0.0, 7.75], [1e15, 1.0, 2.0], [0.0, -0.5]]):
        cases.append({"id": "avg-%d" % i, "proc": "CalcAverageNonZero",
                      "args": [{"ref": "array:double", "values": vals}]})

    # ======================================================================= table
    need_spells = set(SPELLS) | {SPELLS[3]}
    table = {
        "id": "table",
        "spell_fields": SPELL_FIELDS + ["AbilitiesFlat"],
        "item_fields": ITEM_FIELDS,
        "class_fields": ["Number", "CombatLVL", "AbilitiesFlat"],
        "monster_fields": MON_FIELDS,
        "spells": [spells[n]["row"] for n in sorted(need_spells)],
        "items": [items[n]["row"] for n in sorted(used_items)],
        "classes": classes,
        "races": races,
        "monsters": [mons[n]["row"] for n in sorted(set(MON))],
    }
    cases.insert(0, table)

    # arrays the cache code indexes by monster number: MME allocates them for the highest number; -1 is "empty"
    top = max(mons)
    scn = {
        "family": "combat",
        "database": True,
        "description": "Damage vs monster and combat rounds (FAM-08, stock): GetDamageOutput, CalcCombatRounds with its ccr_* helpers, IsMobKillable and CalcAverageNonZero.",
        "notes": [
            "Generated by tools/oracle/gen/combat.py. The case 'table' carries the Monsters, Spells, Items, Classes and Races rows the other cases touch (fields named in the case); the Rust test needs no database.",
            "frmMain is replaced whole by each case's 'forms'; a control not listed reads as the VB default. Control .Tag/.Caption values are what CalcCharacterStats and the Char-tab refreshers leave there; the Rust test reads them with Val().",
            "The per-monster damage cache arrays are allocated to the highest monster number and emptied (-1), so GetDamageOutput always computes; the Rust port has no cache.",
            "Weapons that cast spells (ability 43) are not used: their cast text needs the spell text tables and belongs to the attack family.",
            "GetDamageOutput's result Type is read back as outputs['result'] (lower-cased member names); an error inside it (handler resumes at 'out') leaves its start value, -9999 for average, first-round and surprise damage.",
            "ccr_* are Private in MME; the harness calls them by name.",
        ],
        "tolerance": {"default": 0},
        "globals": {n: [-1] * (top + 1) for n in (
            "nCharDamageVsMonster", "nCharFirstRoundDamageVsMonster", "nCharMinRoundDamageVsMonster",
            "nCharSurpriseDamageVsMonster", "nCharSurpriseMinDamageVsMonster", "nCharSurpriseChanceVsMonster")},
        "cases": cases,
        "grids": grids,
        "samples": samples,
    }
    out = os.path.join(ROOT, "fixtures", "scenarios", "combat.json")
    with open(out, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("wrote %s: %d explicit cases, %d grids, %d samples, %d bytes" % (
        out, len(cases), len(grids), len(samples), os.path.getsize(out)))


if __name__ == "__main__":
    main()

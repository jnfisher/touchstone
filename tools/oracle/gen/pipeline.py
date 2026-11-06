"""Writes fixtures/scenarios/pipeline.json: python3 -I tools/oracle/gen/pipeline.py fixtures/scenarios/pipeline.json
Character pipeline (card P3-44): frmMain RefreshAll (SetupClass, RefreshCPs, CalcCharacterStats with its RefreshMagic /
RefreshMainStatBonuses / RefreshDodge / encumbrance-level tail, RefreshHitPoints, RefreshMagicRes, RefreshPicklocks,
RefreshCombatHealingValues) run whole, then PopulateCharacterProfile twice (a plain profile and a backstab profile).
Characters are built from stock rows only. Deterministic (splitmix64). The rows the cases use are written once into the
scenario's "data" (as field lists, to keep the file small) so the Rust test can rebuild the world."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

MASK = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & MASK

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def below(self, n):
        return self.next() % n

    def between(self, lo, hi):
        return lo + self.below(hi - lo + 1)

    def choice(self, seq):
        return seq[self.below(len(seq))]

    def chance(self, pct):
        return self.below(100) < pct


rng = Rng(0x44C0DE)

# ---------------------------------------------------------------------------------- stock rows
items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}


def ab(row, n):
    return [[row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n) if row["Abil-%d" % i] != 0]


def abil_of(row, n):
    return {a: v for a, v in ab(row, n)}


STOCK_CLASSES = sorted(classes)
STOCK_RACES = sorted(races)
LEVELS = [1, 15, 50, 255]

# ---------------------------------------------------------------------------------- pools
SLOT_WORN = {0: [2], 1: [15], 2: [8], 3: [7], 4: [11], 5: [6], 6: [14, 17], 7: [14, 17], 8: [3], 9: [4, 13], 10: [4, 13],
             11: [10], 12: [9], 13: [5], 14: [16], 15: [12], 17: [18], 18: [19], 19: [1]}
WEAPONS = [n for n, r in items.items() if r["ItemType"] == 1 and r["Encum"] < 900]


def usable(n, cls, level, align):
    """A compact copy of ItemIsUsableByChar's rules, to pick plausible gear (the oracle decides what the stats are)."""
    it = items[n]
    c = classes[cls]
    magical = False
    class_ok = False
    for a, v in ab(it, 20):
        if a == 135 and v > level:
            return False
        if a == 136 and v < level:
            return False
        if a == 59 and v > 0 and v == cls:
            class_ok = True
        if a == 28:
            magical = True
        if a in (97, 98, 112) and align and a != {1: 97, 2: 112, 3: 98}[align]:
            return False
        if a in (110, 111, 113) and align and a == {1: 110, 2: 113, 3: 111}[align]:
            return False
    if 51 in abil_of(c, 10) and magical:
        return False
    if not class_ok:
        rest = [it["ClassRest-%d" % k] for k in range(10) if it["ClassRest-%d" % k]]
        if rest and cls not in rest:
            return False
        if cls in rest:
            return True
    if class_ok:
        return True
    if it["ItemType"] == 0:
        if c["ArmourType"] < it["ArmourType"]:
            return False
        if it["Worn"] == 12 and c["WeaponType"] in (0, 2, 4, 9):
            return False
    elif it["ItemType"] == 1:
        wt = it["WeaponType"]
        k = c["WeaponType"]
        ok = {0: wt == 0, 1: wt == 1, 2: wt == 2, 3: wt == 3, 4: wt in (0, 2), 5: wt in (1, 3), 6: wt >= 2, 7: wt <= 1,
              9: n in (68, 100)}.get(k, True)
        if not ok:
            return False
    return True


_usable_cache = {}


def pools_for(cls, level, align):
    key = (cls, level, align)
    if key not in _usable_cache:
        by_slot = {}
        for s, worn in SLOT_WORN.items():
            by_slot[s] = [n for n, r in items.items() if r["Worn"] in worn and r["ItemType"] in (0, 2, 3, 10)
                          and r["Encum"] < 400 and usable(n, cls, level, align)]
        by_slot[16] = [n for n in WEAPONS if usable(n, cls, level, align)]
        _usable_cache[key] = {s: v for s, v in by_slot.items() if v}
    return _usable_cache[key]


def pick_gear(cls, level, align, fill=85):
    eq = [0] * 20
    for s, pool in pools_for(cls, level, align).items():
        if rng.chance(fill):
            eq[s] = rng.choice(pool)
    return eq


CARRY_POOL = ([n for n, r in items.items() if r["ItemType"] == 10 and any(a in (36, 34, 22, 96, 24, 25, 77, 13, 145, 4, 58, 69, 70, 88, 123, 66, 65, 5, 3)
                                                                       for a, _ in ab(r, 20))][:40]
              + [n for n, r in items.items() if r["ItemType"] == 0 and r["Worn"] == 0][:12])
STAT_SLOTS = {2, 10, 3, 4, 5, 7, 13, 14, 22, 24, 25, 27, 29, 30, 34, 35, 36, 37, 40, 44, 45, 46, 47, 48, 49, 58, 65, 66, 67, 69, 70,
              77, 88, 89, 90, 91, 92, 93, 94, 96, 116, 117, 118, 123, 142, 145, 147, 165, 9}
BLESS_KEY = [34, 52, 4, 1185, 384, 129, 14, 15, 23, 42, 7, 38, 127, 105, 141, 37, 144, 45, 97, 46, 96, 47, 98, 48, 99, 276, 1388,
             1389, 1055, 20, 21, 107, 108, 289, 64, 6, 9, 138, 269, 746, 1314, 869, 39, 302, 858, 879, 118, 307, 100, 26, 130,
             91, 134, 337, 586, 308, 312, 186, 792, 897, 5, 12]
BLESS_KEY = [n for n in BLESS_KEY if n in spells and spells[n]["Dur"] > 0]
HEAL_SPELLS = [13, 17, 27, 89, 121, 123, 138, 747, 831, 1039, 1059, 19, 313]
ATTACK_SPELLS = [1, 285, 301, 340, 706, 413, 426, 207, 17]

used_items, used_spells = set(), set()

# ---------------------------------------------------------------------------------- form builders


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def combo(n):
    return arr({0: {"ListIndex": 1, "ItemData": {"1": n}} if n else {"ListIndex": 0}})


def forms_of(c):
    """frmMain controls for a character dict."""
    f = {
        "cmbGlobalClass": combo(c["cls"]), "cmbGlobalRace": combo(c["race"]),
        "txtGlobalLevel": arr({0: {"Text": str(c["level"])}}),
        "txtCharStats": arr({i: {"Text": str(v)} for i, v in enumerate(c["stats"])}),
        "txtCharMaxStats": arr({i: {"Tag": str(v)} for i, v in enumerate(c["floors"])}),
    }
    eq = {str(i): {"ListIndex": 0, "ItemData": [n]} for i, n in enumerate(c["equipped"]) if n}
    for n in c["equipped"]:
        if n:
            used_items.add(n)
    if eq:
        f["cmbEquip"] = {"$array": eq}
    if c.get("carried"):
        rows = []
        for n, status in c["carried"]:
            rows.append({"Text": str(n), "Sub": ["", status]})
            used_items.add(n)
        f["lvItemManager"] = {"$listview": rows}
    if c.get("hide"):
        f["chkInvenHideCharStats"] = {"Value": 1}
    if c.get("add_weight") is not None:
        f["chkInvenAddWeight"] = {"Value": 1}
        f["txtInvenAddWeight"] = {"Text": str(c["add_weight"])}
    if c.get("align"):
        f["cmbGlobalAlignment"] = {"ListIndex": c["align"]}
    if c.get("minlvl") is not None:
        f["txtGlobalMinLVL"] = {"Text": str(c["minlvl"])}
    bl = {}
    for k, n in enumerate(c.get("bless") or []):
        if n:
            bl[str(k)] = {"ListIndex": 0, "ItemData": [n]}
            used_spells.add(n)
    if bl:
        f["cmbCharBless"] = {"$array": bl}
    if c.get("quests"):
        f["chkCharQuests"] = {"$array": {str(q): {"Value": 1} for q in c["quests"]}}
    if c.get("qopts"):
        f["cmbCharQuestOpts"] = {"$array": {str(k): {"ListIndex": v} for k, v in c["qopts"].items()}}
    f["chkGlobalFilter"] = {"Value": c.get("gfilter", 1)}
    return f


def globals_of(c):
    g = {"nEquippedItem": c["equipped"]}
    if c.get("adjust"):
        a = [0] * 47
        for k, v in c["adjust"].items():
            a[k] = v
        g["char_StatAdjustments"] = a
    a = c.get("attack") or {}
    for k, v in a.items():
        g[k] = v
    for k in ("nGlobalAttackSpellNum", "nGlobalAttackHealSpellNum", "nGlobalAttackBackstabWeapon"):
        if g.get(k):
            (used_spells if "Spell" in k else used_items).add(g[k])
    return g


def attack_cfg(atype=None, heal=None, backstab=False):
    atype = rng.between(0, 7) if atype is None else atype
    heal = rng.between(0, 4) if heal is None else heal
    a = {
        "nGlobalAttackTypeMME": atype,
        "nGlobalAttackSpellNum": rng.choice(ATTACK_SPELLS),
        "nGlobalAttackSpellLVL": rng.choice([0, 5, 30]),
        "bGlobalAttackUseMeditate": rng.chance(40),
        "nGlobalAttackManualP": rng.choice([0, 20, 150]),
        "nGlobalAttackManualM": rng.choice([0, 15, 300]),
        "nGlobalAttackMA": rng.between(1, 3),
        "bGlobalAttackBackstab": backstab,
        "nGlobalAttackBackstabWeapon": rng.choice(WEAPONS) if backstab else 0,
        "nGlobalAttackHealType": heal,
        "nGlobalAttackHealSpellNum": rng.choice(HEAL_SPELLS),
        "nGlobalAttackHealSpellLVL": rng.choice([0, 5, 12, 40]),
        "nGlobalAttackHealRounds": rng.choice([1, 2, 7, 30, 50]),
        "nGlobalAttackHealManual": rng.choice([0, 25, 500]),
    }
    return a


def stats_for(race, level, spread=None):
    r = races[race] if race else None
    out = []
    for lo_k, hi_k in (("mSTR", "xSTR"), ("mINT", "xINT"), ("mWIL", "xWIL"), ("mAGL", "xAGL"), ("mHEA", "xHEA"), ("mCHM", "xCHM")):
        lo = r[lo_k] if r else 30
        hi = r[hi_k] if r else 100
        top = min(hi, lo + 8 + level // 2) if spread is None else hi
        out.append(rng.between(lo, max(lo, top)))
    return out


def floors_for(race):
    if not race:
        return [0] * 6
    r = races[race]
    return [r["mSTR"], r["mINT"], r["mWIL"], r["mAGL"], r["mHEA"], r["mCHM"]]


def make(cid, cls, race, level, **kw):
    c = {"cls": cls, "race": race, "level": level, "floors": floors_for(race)}
    c["stats"] = kw.pop("stats", None) or stats_for(race, level)
    c["equipped"] = kw.pop("equipped", None) or [0] * 20
    if "attack" not in kw:
        kw["attack"] = attack_cfg()
    c.update(kw)
    return {"id": cid, "forms": {"frmMain": forms_of(c)}, "globals": globals_of(c),
            "steps": [{"call": "RefreshAll"},
                      {"call": "PopulateCharacterProfile", "args": [prof(), True, True, 0, 0, False], "as": "profile"},
                      {"call": "PopulateCharacterProfile", "args": [prof(), True, True, 4, 0, False], "as": "profile_bs"}]}


def prof():
    return {"udt": "tCharacterProfile", "fields": {}, "out": True}


def rquests():
    qs = [q for q in range(12) if rng.chance(30)]
    opts = {k: rng.between(0, [8, 8, 8, 3][k]) for k in range(4) if rng.chance(60)}
    return qs, opts


def rbless(n=None):
    n = rng.between(1, 10) if n is None else n
    return [rng.choice(BLESS_KEY) for _ in range(n)] + [0] * (10 - n)


def rcarried():
    st = ["CARRIED", "CARRIED", "CARRIED x2", "CARRIED x3", "STASHED", "EQUIPPED", "CARRIED x2 (stash)"]
    return [(rng.choice(CARRY_POOL), rng.choice(st)) for _ in range(rng.between(1, 4))]


def adjust_some():
    adj = {}
    for _ in range(rng.between(1, 4)):
        adj[rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 19, 24, 30, 31, 33, 34, 37])] = rng.choice(
            [-12, -5, -1, 1, 2, 5, 8, 10, 15, 20])
    return adj


cases = []

# --- A. every class across three races; levels spread over 1/15/50/max; gear empty for the first race, full otherwise
for ci, cls in enumerate(STOCK_CLASSES):
    for k in range(3):
        race = STOCK_RACES[(ci * 3 + k * 5) % len(STOCK_RACES)]
        level = LEVELS[(ci + k) % 4]
        align = rng.between(0, 3)
        gear = [0] * 20 if k == 0 else pick_gear(cls, level, align)
        cases.append(make("grid-c%d-r%d-l%d" % (cls, race, level), cls, race, level, equipped=gear, align=align,
                          gfilter=1 if k != 1 else 0, bless=rbless() if k == 2 else None))

# --- B. level ladder for casters and fighters, full gear
for cls in (1, 7, 12, 15):
    for level in LEVELS:
        race = STOCK_RACES[(cls * 7 + level) % len(STOCK_RACES)]
        cases.append(make("ladder-c%d-l%d" % (cls, level), cls, race, level, equipped=pick_gear(cls, level, 0), align=0))

# --- C. bless slots with real bless spells
for i in range(20):
    cls = rng.choice(STOCK_CLASSES)
    level = rng.choice(LEVELS)
    race = rng.choice(STOCK_RACES)
    cases.append(make("bless-%02d" % i, cls, race, level, equipped=pick_gear(cls, level, 0, 60) if i % 2 else None,
                      bless=rbless(10 if i % 5 == 0 else None)))

# --- D. quests, with options
for i in range(15):
    cls = rng.choice(STOCK_CLASSES)
    level = rng.choice(LEVELS)
    qs, opts = rquests()
    if i < 6:
        qs, opts = [5], {0: i}          # the 2nd quest alone, option 0 meaning "let SetupClass pick the class default"
    cases.append(make("quests-%02d" % i, cls, rng.choice(STOCK_RACES), level, quests=qs, qopts=opts,
                      equipped=pick_gear(cls, level, 0, 50)))
cases.append(make("quests-all", 4, 5, 50, quests=list(range(12)), qopts={0: 3, 1: 2, 2: 1, 3: 1}, equipped=pick_gear(4, 50, 0)))

# --- E. anti-magic class (witchunter) with and without magical gear; other anti-magic gear for a mage
for i in range(5):
    level = LEVELS[i % 4]
    cases.append(make("antimagic-%d" % i, 2, rng.choice(STOCK_RACES), level, equipped=pick_gear(2, level, 0), gfilter=i % 2))

# --- F. add-weight and hidden class/race stats
for i, w in enumerate((0, 25, 300, 1500, 9000, 40)):
    cls = rng.choice(STOCK_CLASSES)
    cases.append(make("addweight-%d" % i, cls, rng.choice(STOCK_RACES), 30, add_weight=w, equipped=pick_gear(cls, 30, 0)))
for i in range(6):
    cls = rng.choice(STOCK_CLASSES)
    level = rng.choice(LEVELS)
    cases.append(make("hide-%d" % i, cls, rng.choice(STOCK_RACES), level, hide=True, equipped=pick_gear(cls, level, 0),
                      bless=rbless() if i % 2 else None))

# --- G. every attack type crossed with the heal types
for atype in range(8):
    for heal in range(5):
        if (atype + heal) % 2 and atype > 4:
            continue
        cls = rng.choice([1, 3, 4, 5, 7, 9, 12, 13, 15])
        level = rng.choice(LEVELS)
        cases.append(make("attack-%d-heal-%d" % (atype, heal), cls, rng.choice(STOCK_RACES), level,
                          attack=attack_cfg(atype, heal, backstab=(atype + heal) % 3 == 0),
                          equipped=pick_gear(cls, level, 0, 70), bless=rbless() if heal in (2, 3) else None))

# --- H. carried items, adjustments, minimum item level
for i in range(10):
    cls = rng.choice(STOCK_CLASSES)
    level = rng.choice(LEVELS)
    cases.append(make("carried-%d" % i, cls, rng.choice(STOCK_RACES), level, carried=rcarried(), adjust=adjust_some() if i % 3 == 0 else None,
                      minlvl=rng.choice([None, 10, 40]), equipped=pick_gear(cls, level, 0, 60)))

# --- I. edges: the "(none)" class and race, level 0, zero stats, extreme stats
cases.append(make("edge-noclass", 0, 3, 20, equipped=[0] * 20))
cases.append(make("edge-norace", 5, 0, 20))
cases.append(make("edge-neither", 0, 0, 1, stats=[0] * 6))
cases.append(make("edge-level0", 12, 5, 0, stats=[0, 0, 0, 80, 0, 0]))
cases.append(make("edge-zero-stats", 7, 2, 40, stats=[0] * 6, equipped=pick_gear(7, 40, 0)))
cases.append(make("edge-huge-stats", 9, 6, 255, stats=[250, 250, 250, 250, 250, 250], equipped=pick_gear(9, 255, 0), bless=rbless(10)))
cases.append(make("edge-low-stats", 3, 4, 1, stats=[1, 1, 1, 1, 1, 1]))
cases.append(make("edge-nolevel-gear", 14, 8, 0, equipped=pick_gear(14, 1, 0)))

# ---------------------------------------------------------------------------------- data pool
ITEM_FIELDS = ["Number", "Encum", "ItemType", "ArmourClass", "DamageResist", "WeaponType", "ArmourType", "Worn", "Accy", "StrReq",
               "Speed"]


def item_row(n):
    r = items[n]
    return [r[k] for k in ITEM_FIELDS] + [[r["ClassRest-%d" % k] for k in range(10) if r["ClassRest-%d" % k]], ab(r, 20)]


SPELL_FIELDS = ["Number", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists", "Targets", "Dur",
                "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc"]


def spell_row(n):
    r = spells[n]
    return [r[k] for k in SPELL_FIELDS] + [ab(r, 10)]


CLASS_FIELDS = ["Number", "MinHits", "MaxHits", "ExpTable", "MageryType", "MageryLVL", "WeaponType", "ArmourType", "CombatLVL"]
RACE_FIELDS = ["Number", "mSTR", "mINT", "mWIL", "mAGL", "mHEA", "mCHM", "HPPerLVL", "ExpTable", "BaseCP"]

data = {
    "item_fields": ITEM_FIELDS + ["ClassRest(nonzero)", "Abilities(ability,value)"],
    "items": [item_row(n) for n in sorted(x for x in used_items if x in items)],
    "spell_fields": SPELL_FIELDS + ["Abilities(ability,value)"],
    "spells": [spell_row(n) for n in sorted(used_spells)],
    "class_fields": CLASS_FIELDS + ["Abilities(ability,value)"],
    "classes": [[classes[n][k] for k in CLASS_FIELDS] + [ab(classes[n], 10)] for n in STOCK_CLASSES],
    "race_fields": RACE_FIELDS + ["Abilities(ability,value)"],
    "races": [[races[n][k] for k in RACE_FIELDS] + [ab(races[n], 10)] for n in STOCK_RACES],
}

FRM = {
    "file": "frmMain.frm",
    "decls": ["bless_Stats", "bless_StatText", "char_StatAdjustments", "bDontRefresh", "bDontRefreshInvenStats", "bAlreadyRefreshing"],
    "procs": ["RefreshAll", "SetupClass", "RefreshCPs", "CalcCharacterStats", "InvenResetStats", "AdjMainStatBonus", "RefreshCharBless",
              "InvenCalcEncum", "ItemIsUsableByChar", "ItemIsCarryActive", "ItemIsFlaggedCarried", "AccumItemStatBonus",
              "PastedStatIsBuffed", "RefreshMagic", "RefreshHitPoints", "RefreshMagicRes", "RefreshPicklocks", "RefreshDodge",
              "RefreshMainStatBonuses", "lblInvenCharStat_Change"],
    # display and later-card helpers: tool-tip text, the filter reset, the shop and monster panes, the colour passes
    "stub_procs": ["SortInvenToolTips", "TruncateStatTips", "GetDodgeSummary", "ResetFilterOptions", "RefreshShops",
                   "InvenColorCodeStats", "RefreshMonsterCombatGUI", "SetCharDefenseDescription", "GetEncumPercents"],
    "cuts": {
        # RefreshAll: everything after RefreshCombatHealingValues is list colouring and the lair-info reset
        "RefreshAll": {"cut": "Call RefreshListviewItemColors"},
        # CalcCharacterStats: stop before the damage-output label (GetDamageOutput, later cards) and the tool-tip loop,
        # but keep the out: section (RefreshMainStatBonuses, RefreshDodge, the encumbrance level)
        "CalcCharacterStats": {
            "cut": "nDmgOut = GetDamageOutput(0, 0, 0, 50, 0, , , , , , , , , , , , True)",
            "append": ['lblInvenCharStat(46).Caption = "0"'],
            "tail": ["out:", "On Error Resume Next", "bDontRefreshInvenStats = False", "Call RefreshMainStatBonuses",
                     "Call RefreshDodge", "Call lblInvenCharStat_Change(0)", "Exit Sub", "error:",
                     'Call HandleError("CalcCharacterStats")', "Resume out:", "End Sub"]},
        # the encumbrance level word and percentage; what follows is tool-tip placement
        "lblInvenCharStat_Change": {"cut": "tooltip:", "append": ["tooltip:", "End Select"]},
    },
}

READ = {
    "stat": "%c.lblInvenCharStat.Caption[0..46]", "tags": "%c.lblInvenCharStat.Tag[0..46]",
    "ctags": "%c.txtCharStats.Tag[0..5]", "labels": "%c.lblLabelArray.Caption[0..24]", "label_tags": "%c.lblLabelArray.Tag[0..24]",
    "cps": "%c.lblStatCalc.Caption", "cps_tag": "%c.lblStatCalc.Tag",
    "rest_tag": "%c.lblCharRestRate.Tag", "rest_cap": "%c.lblCharRestRate.Caption",
    "hp_tag": "%c.lblCharMaxHP.Tag", "hp_cap": "%c.lblCharMaxHP.Caption",
    "mana_rate_tag": "%c.lblCharManaRate.Tag", "mana_rate_cap": "%c.lblCharManaRate.Caption", "medi": "%c.txtCharManaRegen.Tag",
    "maxmana_tag": "%c.lblCharMaxMana.Tag", "maxmana_cap": "%c.lblCharMaxMana.Caption",
    "sc_tag": "%c.lblCharSC.Tag", "sc_cap": "%c.lblCharSC.Caption",
    "mr": "%c.txtCharMR.Text", "pick_tag": "%c.lblCharPicklocks.Tag", "pick_cap": "%c.lblCharPicklocks.Caption",
    "dodge_tag": "%c.lblCharDodge.Tag", "dodge_cap": "%c.lblCharDodge.Caption",
    "encum_cap": "%c.lblEncumLevel(1).Caption", "encum_tag": "%c.lblEncumLevel(1).Tag",
    "bless_cap": "%c.lblCharBless.Caption", "hpregen": "%c.txtCharHPRegen.Text", "manaregen": "%c.txtCharManaRegen.Text",
    # SetupClass
    "qopts": "%c.cmbCharQuestOpts.ListIndex[0..3]", "mlvl_idx": "%c.cmbSpellMageryLevel.ListIndex",
    "magery_idx": "%c.cmbSpellMagery.ListIndex", "antimagic": "%c.chkCharAntiMagic.Value", "fra_pick": "%c.lblCharPicklocks.Enabled",
    "fra_sc": "%c.lblCharSC.Enabled", "spellopt": "%c.chkSpellOptions(1).Value",
    "healvalue": "%g.nGlobalAttackHealValue", "healcost": "%g.nGlobalAttackHealCost", "healrounds": "%g.nGlobalAttackHealRounds",
    "heallvl": "%g.nGlobalAttackHealSpellLVL", "attack_accy_adj": "%g.nGlobalAttackAccyAdj",
}
for g in ("nGlobalCharAccyItems", "nGlobalCharAccyAbils", "nGlobalCharAccyOther", "nGlobalCharPlusDodge", "nGlobalCharPlusMR",
          "nGlobalCharVileWard", "nGlobalCharShadowAC", "nGlobalCharWornArmourType", "nGlobalCharQnDbonus",
          "nGlobalCharMageryLVL", "nGlobalCharMagery"):
    READ[g[len("nGlobal"):]] = "%g." + g
for g in ("Number", "Encum", "Accy", "Crit", "STR", "AGI", "MaxDmg", "BSaccy", "BSmindmg", "BSmaxdmg", "PunchSkill", "PunchAccy",
          "PunchDmg", "KickSkill", "KickAccy", "KickDmg", "JkSkill", "JkAccy", "JkDmg", "Stealth"):
    READ["weapon" + g] = "%g.nGlobalCharWeapon" + g
READ["class_stealth_tag"] = "%c.lblInvenStats.Tag[19..19]"

scn = {
    "family": "pipeline",
    "description": "Character pipeline (FAM-04/05): frmMain RefreshAll run whole for a loaded character, then PopulateCharacterProfile",
    "notes": [
        "Each case runs RefreshAll (cut from frmMain.frm, scenario key frm) and then PopulateCharacterProfile twice with "
        "bForceUseChar and bForceNoParty: attack kind 0 ('profile') and the backstab kind 4 with the current weapon ('profile_bs'); "
        "the profiles are the ByRef results in outputs['profile.refs'][0] and outputs['profile_bs.refs'][0] (lower-cased member names).",
        "RefreshAll is cut after RefreshCombatHealingValues (list colours and the lair-info reset are display). CalcCharacterStats is cut "
        "before its GetDamageOutput line (stat 46, later cards) but keeps its out: section; its tool-tip text helpers, "
        "SetCharDefenseDescription, RefreshShops and the colour passes are recorded no-ops.",
        "Controls: txtCharMaxStats(i).Tag holds the race stat floors (cmbGlobalRace_Click writes them), chkCharQuests(0..11), "
        "cmbCharQuestOpts(0..3), cmbEquip, lvItemManager rows, cmbCharBless; globals: nEquippedItem, char_StatAdjustments and the "
        "nGlobalAttack* choice. Class and race combos are one-entry lists with ListIndex 1 (ListIndex 0 is the '(none)' entry).",
        "Event cascades (a .Text change re-running a Refresh*) are not modelled by the shim; each step runs once.",
        "data holds the stock rows the cases use as field lists (items, spells) and all stock classes and races, for the Rust test.",
    ],
    "database": True,
    "frm": FRM,
    "data": data,
    "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0},
    "forms": {"frmMain": {
        "lblDatVer": {"Caption": "v1.11p"}, "lblInvenCharStat": arr({46: {}}), "lblLabelArray": arr({24: {}}),
        "txtCharStats": arr({5: {}}), "txtCharMaxStats": arr({5: {}}), "cmbGlobalClass": arr({0: {}}), "cmbGlobalRace": arr({0: {}}),
        "txtGlobalLevel": arr({0: {}}), "cmbCharBless": arr({9: {}}), "chkGlobalFilter": {"Value": 1},
        "txtCharHPRegen": {}, "txtCharManaRegen": {},
        "chkArmourType": arr({k: {"Value": 0} for k in range(7)}), "chkHanded": arr({k: {"Value": 0} for k in range(4)}),
        "fraChar": arr({k: {} for k in range(4)}), "chkWeaponOptions": arr({0: {"Value": 5}}),
        "chkSpellOptions": arr({1: {"Value": 5}}), "chkCharAntiMagic": {"Value": 5}, "chkArmourNonMagic": {"Value": 5},
        "lblInvenStats": arr({i: {"Caption": "Stat%d:" % i} for i in range(47)}),
        "cmbSpellMageryLevel": {"ListIndex": -1, "ListCount": 4}, "cmbSpellMagery": {"ListIndex": -1, "ListCount": 6},
        "chkEquipHold": arr({19: {}}), "picStats": arr({1: {}}), "lvItemManager": {"$listview": []},
        "cmbGlobalAlignment": {"ListIndex": 0}, "chkCharQuests": arr({11: {}}), "cmbCharQuestOpts": arr({3: {}}),
        "lblEncumLevel": arr({1: {}}), "optMonsterFilter": arr({1: {"Value": False}}),
        "txtMonsterLairFilter": arr({k: {} for k in range(9)}), "txtMonsterDamage": {},
    }},
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d items, %d spells" % (len(cases), len(data["items"]), len(data["spells"])), file=sys.stderr)

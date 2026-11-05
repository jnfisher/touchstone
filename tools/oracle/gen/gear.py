"""Writes fixtures/scenarios/gear.json: python3 -I tools/oracle/gen/gear.py fixtures/scenarios/gear.json
Deterministic (splitmix64 draws). Reads the stock SQLite for real Items/Spells/Classes/Races rows and
adds a few invented rows (numbers 9001+) for branches no stock row reaches. The rows the cases use are
written once into the scenario's "data" so the Rust test can rebuild them."""
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


rng = Rng(0x6EA4)

# ---------------------------------------------------------------------------------- stock rows
items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}


def ab(row, n):
    return [x for x in ([row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n)) if x[0] != 0]


# ---------------------------------------------------------------------------------- invented rows
def item(n, name, **kw):
    r = {"Number": n, "Name": name, "Encum": 0, "ItemType": 0, "Worn": 0, "ArmourClass": 0, "DamageResist": 0,
         "Accy": 0, "WeaponType": 0, "ArmourType": 0, "Min": 0, "Max": 0, "Speed": 0, "StrReq": 0}
    for k, v in kw.items():
        if k == "abil":
            for i, (a, val) in enumerate(v):
                r["Abil-%d" % i] = a
                r["AbilVal-%d" % i] = val
        elif k == "rest":
            for i, c in enumerate(v):
                r["ClassRest-%d" % i] = c
        else:
            r[k] = v
    return r


EXTRA_ITEMS = [
    item(9001, "tst blur ring", Worn=4, Encum=2, abil=[(10, 12)]),
    item(9002, "tst accy amulet", Worn=8, Encum=3, abil=[(22, 4), (22, 9), (22, 6)]),
    item(9003, "tst enc belt", Worn=10, Encum=40, abil=[(96, 15), (34, 3)]),
    item(9004, "tst might gloves", Worn=3, Encum=8, ArmourClass=10, ArmourType=2,
         abil=[(46, 6), (48, -2), (44, 3)]),
    item(9005, "tst arcane blade", ItemType=1, Encum=60, WeaponType=2, Min=3, Max=9, Accy=4, Speed=40,
         abil=[(142, 7), (28, 3), (4, 5), (58, 2), (116, 4), (117, 2), (118, 6), (27, 8), (29, 2), (89, 3), (92, 1),
               (30, 4), (90, 5), (93, 2), (35, 1), (91, 3), (94, 4)]),
    item(9006, "tst shield", Worn=12, Encum=70, ArmourClass=30, DamageResist=10, ArmourType=4, WeaponType=0,
         abil=[(2, 10), (7, 20), (36, 9), (25, 3), (116, 5), (4, 1)]),
    item(9007, "tst class helm", Worn=2, Encum=20, ArmourClass=20, ArmourType=3, rest=[3, 4], abil=[(135, 5)]),
    item(9008, "tst level ring", Worn=4, Encum=1, abil=[(135, 10), (136, 40), (22, 3)]),
    item(9009, "tst nowhere armour", ItemType=0, Worn=0, Encum=15, ArmourClass=10, DamageResist=5, Accy=6,
         ArmourType=1, abil=[(34, 2), (5, 5)]),
    item(9010, "tst special charm", ItemType=10, Encum=5, abil=[(36, 4), (34, 3), (22, 8), (96, 5), (10, 6)]),
    item(9011, "tst good cloak", Worn=7, Encum=12, ArmourClass=5, ArmourType=1, abil=[(97, 1), (36, 6)]),
    item(9012, "tst notevil cape", Worn=7, Encum=9, ArmourClass=5, ArmourType=1, abil=[(111, 1), (34, 2)]),
    item(9013, "tst magic plate", Worn=11, Encum=300, ArmourClass=60, DamageResist=22, ArmourType=9,
         abil=[(28, 1), (142, 6), (2, 4), (10, 21)]),
    item(9014, "tst heavy pack", ItemType=10, Encum=900, abil=[(96, 50)]),
    item(9015, "tst cursed ring", Worn=4, Encum=1, Accy=-3, ArmourClass=-5, abil=[(34, -4), (4, -2), (7, -10), (36, -6)]),
    item(9016, "tst great blade", ItemType=1, Encum=120, WeaponType=3, Min=8, Max=30, Accy=2, Speed=60, abil=[(28, 2), (4, 3)]),
    item(9017, "tst mace", ItemType=1, Encum=45, WeaponType=0, Min=2, Max=8, Accy=1, Speed=45, abil=[(142, -3)]),
    item(9018, "tst neutral charm", ItemType=10, Encum=2, abil=[(112, 1), (36, 3), (24, 5)]),
    item(9019, "tst notgood charm", ItemType=10, Encum=2, abil=[(110, 1), (34, 6)]),
    item(9020, "tst plain armour", ItemType=0, Worn=0, Encum=25, ArmourClass=20, ArmourType=5, rest=[7, 8, 9, 10, 11, 12, 13, 14, 15, 1],
         abil=[(7, 10)]),
    item(9101, "tst stat band", Worn=14, Encum=1, abil=[(46, 5), (44, -3), (45, 2), (48, 7), (47, 4), (49, 6)]),
    item(9102, "tst twin str", ItemType=10, Encum=1, abil=[(46, 3), (46, 4), (47, 0)]),
    item(9103, "tst ordinary", Worn=14, Encum=1, abil=[(36, 1)]),
]
EXTRA_SPELLS = [
    {"Number": 9001, "Name": "tst shadow accy", "Short": "tsa", "ReqLevel": 3, "Cap": 30, "ManaCost": 5, "MinBase": 2,
     "MaxBase": 6, "MinInc": 1, "MinIncLVLs": 2, "MaxInc": 2, "MaxIncLVLs": 3, "Dur": 60, "DurInc": 3, "DurIncLVLs": 5,
     "Abil-0": 9, "AbilVal-0": 0, "Abil-1": 22, "AbilVal-1": 7, "Abil-2": 4, "AbilVal-2": 0, "Abil-3": 10, "AbilVal-3": 0},
    {"Number": 9002, "Name": "tst accy stack", "Short": "tas", "ReqLevel": 0, "Cap": 0, "ManaCost": 8, "MinBase": 4,
     "MaxBase": 4, "Dur": 30, "Abil-0": 22, "AbilVal-0": 15, "Abil-1": 22, "AbilVal-1": 4, "Abil-2": 2, "AbilVal-2": -3,
     "Abil-3": 10, "AbilVal-3": 8, "Abil-4": 7, "AbilVal-4": 25, "Abil-5": 46, "AbilVal-5": 4, "Abil-6": 36, "AbilVal-6": 0},
]
EXTRA_RACES = [{"Number": 9001, "Name": "tst race", "HPPerLVL": 3, "Abil-0": 46, "AbilVal-0": 5, "Abil-1": 49, "AbilVal-1": -2,
                "Abil-2": 96, "AbilVal-2": 10, "Abil-3": 22, "AbilVal-3": 12, "Abil-4": 34, "AbilVal-4": -5,
                "Abil-5": 2, "AbilVal-5": 3, "Abil-6": 7, "AbilVal-6": 15, "Abil-7": 103, "AbilVal-7": 0}]
EXTRA_CLASSES = [{"Number": 9001, "Name": "tst class", "WeaponType": 8, "ArmourType": 9, "Abil-0": 45, "AbilVal-0": 4,
                  "Abil-1": 22, "AbilVal-1": 6, "Abil-2": 7, "AbilVal-2": 30, "Abil-3": 36, "AbilVal-3": 9,
                  "Abil-4": 142, "AbilVal-4": 3, "Abil-5": 4, "AbilVal-5": 2, "Abil-6": 102, "AbilVal-6": 0}]
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          **{"ClassRest-%d" % i: 0 for i in range(10)}, **r}
for r in EXTRA_SPELLS:
    spells[r["Number"]] = {**{k: 0 for k in spells[14] if k not in ("Name", "Short")}, **{"Name": "", "Short": ""}, **r}
for r in EXTRA_RACES:
    races[r["Number"]] = {**{k: 0 for k in races[1]}, **r}
for r in EXTRA_CLASSES:
    classes[r["Number"]] = {**{k: 0 for k in classes[1]}, **r}

# ---------------------------------------------------------------------------------- pools
SLOT_WORN = {0: [2], 1: [15], 2: [8], 3: [7], 4: [11], 5: [6], 6: [14, 17], 7: [14, 17], 8: [3], 9: [4, 13], 10: [4, 13],
             11: [10], 12: [9], 13: [5], 14: [16], 15: [12], 17: [18], 18: [19], 19: [1]}
by_slot = {s: [n for n, r in items.items() if r["Worn"] in w and r["ItemType"] in (0, 2, 3, 10) and r["Encum"] < 900]
           for s, w in SLOT_WORN.items()}
by_slot[16] = [n for n, r in items.items() if r["ItemType"] == 1 and r["Encum"] < 900]
by_slot = {s: v for s, v in by_slot.items() if v}
HEAVY = [n for n, r in items.items() if r["Encum"] >= 900 and r["Worn"] != 0][:6] + [9014]
CARRY_POOL = ([n for n, r in items.items() if r["ItemType"] == 10 and any(a[0] in (36, 34, 22, 96, 24, 25, 77, 13, 145, 4, 58, 69, 70, 88, 123, 66, 65, 5, 3)
                                                                       for a in ab(r, 20))][:40]
              + [n for n, r in items.items() if r["ItemType"] == 0 and r["Worn"] == 0][:12]
              + [n for n, r in items.items() if r["ItemType"] in (1, 3, 9)][:12]
              + [9009, 9010, 9014, 9018, 9019, 9020, 9102, 9004])
STAT_SLOTS = {2, 10, 3, 4, 5, 7, 13, 14, 22, 24, 25, 27, 29, 30, 34, 35, 36, 37, 40, 44, 45, 46, 47, 48, 49, 58, 65, 66, 67, 69, 70,
              77, 88, 89, 90, 91, 92, 93, 94, 96, 116, 117, 118, 123, 142, 145, 147, 165, 9, 3}
BLESS_POOL = [n for n, r in spells.items() if n < 9000 and any(a[0] in STAT_SLOTS for a in ab(r, 10))]
BLESS_KEY = [34, 52, 4, 1185, 384, 129, 14, 15, 23, 42, 7, 38, 127, 105, 141, 37, 144, 45, 97, 46, 96, 47, 98, 48, 99, 276, 1388,
             1389, 1055, 20, 21, 107, 108, 289, 64, 6, 9, 138, 269, 746, 1314, 869, 39, 302, 858, 879, 118, 307, 9001, 9002,
             100, 26, 130, 91, 134, 337, 586, 308, 312, 186, 792, 897, 5, 12]
BLESS_KEY = [n for n in BLESS_KEY if n in spells]

# ---------------------------------------------------------------------------------- case builders
used_items, used_spells, used_classes, used_races = set(), set(), set(), set()


def combo(n):
    return {"$array": {"0": {"ListIndex": 1, "ItemData": {"1": n}} if n else {"ListIndex": 0}}}


def equip_forms(equipped):
    g = {}
    for i, n in enumerate(equipped):
        if n:
            g[str(i)] = {"ListIndex": 0, "ItemData": [n]}
            used_items.add(n)
    return {"$array": g} if g else None


def lv(rows):
    out = []
    for item_text, subs in rows:
        out.append({"Text": str(item_text), "Sub": subs})
        try:
            used_items.add(int(item_text))
        except ValueError:
            pass
    return {"$listview": out}


def calc_case(cid, level=20, cls=0, race=0, stats=None, hide=False, equipped=None, carried=None, adjust=None,
              bless=None, ma=False, add_weight=None, align=0, minlvl=None, level_text=None):
    stats = stats or [60, 55, 50, 65, 70, 45]
    equipped = equipped or [0] * 20
    forms = {
        "cmbGlobalClass": combo(cls), "cmbGlobalRace": combo(race),
        "txtGlobalLevel": {"$array": {"0": {"Text": level_text if level_text is not None else str(level)}}},
        "txtCharStats": {"$array": {str(i): {"Text": str(v)} for i, v in enumerate(stats)}},
    }
    if cls:
        used_classes.add(cls)
    if race:
        used_races.add(race)
    ce = equip_forms(equipped)
    if ce:
        forms["cmbEquip"] = ce
    if hide:
        forms["chkInvenHideCharStats"] = {"Value": 1}
    if add_weight is not None:
        forms["chkInvenAddWeight"] = {"Value": 1}
        forms["txtInvenAddWeight"] = {"Text": str(add_weight)}
    if carried:
        forms["lvItemManager"] = lv(carried)
    if align:
        forms["cmbGlobalAlignment"] = {"ListIndex": align}
    if minlvl is not None:
        forms["txtGlobalMinLVL"] = {"Text": str(minlvl)}
    if bless:
        g = {}
        for i, n in enumerate(bless):
            if n:
                g[str(i)] = {"ListIndex": 0, "ItemData": [n]}
                used_spells.add(n)
        forms["cmbCharBless"] = {"$array": g}
    globs = {}
    if any(equipped):
        globs["nEquippedItem"] = equipped
    if adjust:
        arr = [0] * 47
        for k, v in adjust.items():
            arr[k] = v
        globs["char_StatAdjustments"] = arr
    if ma:
        globs["nGlobalAttackTypeMME"] = 4
    case = {"id": cid, "steps": [{"call": "CalcCharacterStats"}], "forms": {"frmMain": forms}}
    if globs:
        case["globals"] = globs
    return case


def random_equipped(heavy_pct=3, fill=70):
    eq = [0] * 20
    for s, pool in by_slot.items():
        if rng.chance(fill):
            eq[s] = rng.choice(pool)
    if rng.chance(25):
        eq[rng.choice([4, 9, 10, 0, 2, 11, 14])] = rng.choice(EXTRA_ITEMS)["Number"]
    if rng.chance(heavy_pct):
        eq[rng.choice([2, 3, 19])] = rng.choice(HEAVY)
    if rng.chance(8):
        eq[15] = 0
    return eq


CARRY_STATUS = ["CARRIED", "CARRIED", "CARRIED x2", "CARRIED x3", "carried x5", "CARRIED x0", "CARRIED x-2", "CARRIED xtwo",
                "CARRIED x2 (stash)", "STASHED", "EQUIPPED", "Carried"]


def random_carried():
    rows = []
    for _ in range(rng.between(1, 4)):
        n = rng.choice(CARRY_POOL)
        st = rng.choice(CARRY_STATUS)
        rows.append((n, ["", st]) if rng.chance(92) else (n, [st]))
    if rng.chance(15):
        rows.append((0, ["", "CARRIED"]))
    if rng.chance(10):
        rows.append((rng.choice(CARRY_POOL), ["", "CARRIED"]))
    return rows


def random_adjust():
    adj = {}
    for _ in range(rng.between(1, 5)):
        k = rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 19, 24, 30, 31, 33, 34, 37, 46, 43])
        adj[k] = rng.choice([-30, -12, -5, -1, 1, 2, 5, 8, 10, 15, 20, 37, 100])
    return adj


cases = []

# --- A. CalcCharacterStats (first half)
# A1: empty character and the class/race grids
cases.append(calc_case("calc-empty", level=1, stats=[0] * 6, level_text="0"))
cases.append(calc_case("calc-nolevel", level=0, cls=1, race=1, level_text=""))
for c in range(1, 16):
    cases.append(calc_case("calc-class-%d" % c, level=rng.between(1, 40), cls=c, race=1 + (c * 5) % 13,
                           stats=[rng.between(30, 100) for _ in range(6)]))
for r in range(1, 14):
    cases.append(calc_case("calc-race-%d" % r, level=rng.between(1, 40), cls=1 + (r * 7) % 15, race=r,
                           stats=[rng.between(30, 100) for _ in range(6)]))
for cls, race in ((9001, 0), (0, 9001), (9001, 9001), (9001, 2), (4, 9001), (7, 10)):
    cases.append(calc_case("calc-invented-%d-%d" % (cls, race), level=15, cls=cls, race=race))
    cases.append(calc_case("calc-invented-hide-%d-%d" % (cls, race), level=15, cls=cls, race=race, hide=True))
for c in (7, 15, 2):
    cases.append(calc_case("calc-hide-%d" % c, level=30, cls=c, race=10, hide=True))
# A2: the invented items one at a time, in their natural slots and a wrong one
solo = [(9001, 9), (9002, 2), (9003, 11), (9004, 8), (9005, 16), (9006, 15), (9007, 0), (9008, 9), (9015, 10), (9013, 4),
        (9016, 16), (9017, 16), (9005, 15), (9013, 16), (9006, 4), (9009, 12), (9010, 2)]
for n, slot in solo:
    for cls in (0, 4):
        eq = [0] * 20
        eq[slot] = n
        cases.append(calc_case("calc-solo-%d-s%d-c%d" % (n, slot, cls), level=25, cls=cls, race=3 if cls else 0, equipped=eq))
cases.append(calc_case("calc-solo-9005-ma", level=25, cls=15, race=2, equipped=[0] * 16 + [9005, 0, 0, 0], ma=True))
cases.append(calc_case("calc-solo-9017-ma", level=25, cls=1, race=1, equipped=[0] * 16 + [9017, 0, 0, 0], ma=True))
# two-handed weapon with shield, weapon in both hands, two rings, a blur armour with each armour type
cases.append(calc_case("calc-2h-shield", equipped=[0] * 15 + [9006, 9016, 0, 0, 0], cls=1, race=8))
cases.append(calc_case("calc-weapons-both", equipped=[0] * 15 + [9005, 9017, 0, 0, 0], cls=14, race=9))
cases.append(calc_case("calc-rings", equipped=[0] * 9 + [9001, 9002, 0, 0, 0, 0, 0, 0, 0, 0], cls=3, race=4))
for at, n in ((1, 9001), (3, 9007), (5, 9020), (9, 9013)):
    cases.append(calc_case("calc-blur-armour-%d" % at, level=30, cls=1, race=1, equipped=[9007, 0, 0, 0, 9013, 0, 0, 0, 0, 9001, 9010, 0, 0, 0, 0, 0, 0, 0, 0, 0]))
# A3: blur with each worn armour type and bless
for t, base in enumerate((0, 2, 4, 6, 8, 10)):
    pass
# A4: manual adjustments, one stat at a time and in mixes
for k in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
          32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46):
    cases.append(calc_case("calc-adjust-%d" % k, level=12, cls=2 if k % 2 else 0, race=5 if k % 3 else 0,
                           equipped=[0] * 4 + [rng.choice(by_slot[4])] + [0] * 15, adjust={k: -7 if k % 5 == 0 else 13}))
for i in range(6):
    cases.append(calc_case("calc-adjust-mix-%d" % i, level=rng.between(1, 50), cls=rng.between(0, 15), race=rng.between(0, 13),
                           equipped=random_equipped(), adjust=random_adjust(), bless=[rng.choice(BLESS_KEY) for _ in range(rng.between(0, 3))]))
# A5: bless spells
for i in range(18):
    cases.append(calc_case("calc-bless-%d" % i, level=rng.choice([1, 3, 8, 15, 27, 40, 80]), cls=rng.between(0, 15),
                           race=rng.between(0, 13), equipped=random_equipped() if i % 2 else None,
                           bless=[rng.choice(BLESS_KEY) if rng.chance(60) else 0 for _ in range(10)],
                           stats=[rng.between(20, 110) for _ in range(6)]))
for key in ([9001, 9002], [14, 22, 42], [4, 1185, 384], [47, 46, 276], [96, 97, 98, 99], [34, 52, 71, 9002]):
    if all(k in spells for k in key):
        cases.append(calc_case("calc-bless-set-%s" % "-".join(map(str, key)), level=33, cls=1, race=8, bless=key + [0] * (10 - len(key)),
                               equipped=[0] * 4 + [9013] + [0] * 15))
# A6: carried items
for i in range(26):
    cases.append(calc_case("calc-carried-%d" % i, level=rng.choice([1, 10, 25, 60]), cls=rng.between(0, 15), race=rng.between(0, 13),
                           equipped=random_equipped(), carried=random_carried(), align=rng.below(4),
                           minlvl=rng.choice([None, None, 0, 5, 20])))
# A7: random gear sets
for i in range(95):
    cases.append(calc_case("calc-random-%03d" % i, level=rng.choice([0, 1, 2, 5, 9, 14, 20, 28, 35, 44, 60, 80, 120]),
                           cls=rng.between(0, 15) if rng.chance(90) else 9001, race=rng.between(0, 13) if rng.chance(92) else 9001,
                           stats=[rng.between(15, 130) for _ in range(6)], hide=rng.chance(8), equipped=random_equipped(),
                           carried=random_carried() if rng.chance(25) else None, adjust=random_adjust() if rng.chance(18) else None,
                           bless=[rng.choice(BLESS_KEY if rng.chance(75) else BLESS_POOL) if rng.chance(45) else 0 for _ in range(10)] if rng.chance(35) else None,
                           ma=rng.chance(15), add_weight=rng.choice([0, 25, 300, 1500, 9000]) if rng.chance(10) else None,
                           align=rng.below(4) if rng.chance(30) else 0))
# heavy encumbrance
for i in range(4):
    eq = random_equipped(heavy_pct=0)
    eq[3] = HEAVY[i]
    cases.append(calc_case("calc-heavy-%d" % i, level=20, cls=1 + i, race=1 + i, equipped=eq, stats=[40 + 25 * i, 50, 50, 50, 50, 50],
                           bless=[34, 52, 4] if i % 2 else None, add_weight=200 if i == 3 else None))

# --- B. RefreshCharBless on its own: bless_Stats for ten slots, any worn-armour type
for i in range(70):
    nb = rng.between(1, 10)
    slots = [rng.choice(BLESS_KEY if rng.chance(70) else BLESS_POOL) if k < nb else 0 for k in range(10)]
    lvl = rng.choice([0, 1, 2, 4, 7, 11, 19, 30, 45, 99])
    forms = {"txtGlobalLevel": {"$array": {"0": {"Text": str(lvl)}}}}
    g = {}
    for k, n in enumerate(slots):
        if n:
            g[str(k)] = {"ListIndex": 0, "ItemData": [n]}
            used_spells.add(n)
    forms["cmbCharBless"] = {"$array": g}
    cases.append({"id": "bless-%03d" % i, "steps": [{"call": "RefreshCharBless"}], "forms": {"frmMain": forms},
                  "globals": {"nGlobalCharWornArmourType": rng.choice([0, 0, 1, 3, 4, 6, 8, 9, 12])},
                  "read": {"bless_Stats": "%g.bless_Stats", "slots": "%g.nGlobalCharBlessSpells"}})

# --- C. ItemIsUsableByChar
usable_pool = [n for n, r in items.items() if any(a[0] in (135, 136, 59, 28, 97, 98, 112, 110, 111, 113) for a in ab(r, 20))
               or any(r["ClassRest-%d" % k] for k in range(10))]
usable_pool = usable_pool[::7][:70] + [n for n, r in items.items() if r["ItemType"] == 0 and r["Worn"] in (12, 2, 11)][::9][:18] \
    + [n for n, r in items.items() if r["ItemType"] == 1][::25][:18] + [68, 100] + [n for n in range(9001, 9021)]
usable_pool = [n for n in usable_pool if n in items]
for i in range(220):
    n = rng.choice(usable_pool) if rng.chance(95) else rng.choice([0, 99999, -3])
    cls = rng.between(0, 15) if rng.chance(95) else 9001
    lvl = rng.choice([1, 4, 9, 10, 11, 20, 40, 41, 80, 999])
    f = {"cmbGlobalClass": combo(cls), "txtGlobalLevel": {"$array": {"0": {"Text": str(lvl)}}}}
    if cls:
        used_classes.add(cls)
    if n > 0:
        used_items.add(n)
    if rng.chance(70):
        f["cmbGlobalAlignment"] = {"ListIndex": rng.below(4)}
    mn = rng.choice([None, None, "0", "8", "30"])
    if mn:
        f["txtGlobalMinLVL"] = {"Text": mn}
    case = {"id": "usable-%03d" % i, "forms": {"frmMain": f}, "read": {},
            "proc": "ItemIsUsableByChar", "args": [n, rng.chance(30), rng.chance(25)]}
    if mn and rng.chance(50):
        eq = [0] * 20
        eq[rng.below(20)] = n
        case["globals"] = {"nEquippedItem": eq}
    cases.append(case)

# --- D. AccumItemStatBonus (and ItemIsCarryActive underneath)
accum_pool = [9101, 9102, 9103, 9004, 9010, 9005] + [n for n, r in items.items() if any(a[0] in (44, 45, 46, 47, 48, 49) for a in ab(r, 20))][:6] \
    + [n for n, r in items.items() if r["ItemType"] == 10][:10] + [n for n, r in items.items() if r["ItemType"] == 0 and r["Worn"] == 0][:6]
for i in range(70):
    n = rng.choice(accum_pool) if rng.chance(95) else rng.choice([0, 99999])
    cls = rng.between(0, 15)
    if cls:
        used_classes.add(cls)
    used_items.add(n)
    f = {"cmbGlobalClass": combo(cls), "txtGlobalLevel": {"$array": {"0": {"Text": str(rng.choice([1, 12, 40]))}}}}
    cases.append({"id": "accum-%03d" % i, "forms": {"frmMain": f}, "read": {},
                  "steps": [{"call": "AccumItemStatBonus", "as": "r",
                             "args": [n, rng.chance(60), rng.choice([0, 1, 2, 3, 7]),
                                      {"ref": "array:long", "bounds": [[0, 5]]}, {"ref": "array:string", "bounds": [[0, 5]]}]}]})

# --- E. ItemIsCarryActive / ItemIsFlaggedCarried
for i in range(50):
    n = rng.choice(CARRY_POOL + [9101, 9103]) if rng.chance(92) else rng.choice([0, -1, 99999])
    cls = rng.between(0, 15)
    if cls:
        used_classes.add(cls)
    used_items.add(n)
    rows = random_carried() + [(n, ["", rng.choice(CARRY_STATUS)])] if rng.chance(60) else random_carried()
    f = {"cmbGlobalClass": combo(cls), "txtGlobalLevel": {"$array": {"0": {"Text": str(rng.choice([1, 12, 40]))}}},
         "lvItemManager": lv(rows)}
    cases.append({"id": "carry-%03d" % i, "forms": {"frmMain": f}, "read": {},
                  "steps": [{"call": "ItemIsCarryActive", "args": [n], "as": "active"},
                            {"call": "ItemIsFlaggedCarried", "args": [n], "as": "flagged"}]})

# --- F. PastedStatIsBuffed
BUFF = [("Str: 55*", "Str"), ("Str: 55", "Str"), ("STR 55 *", "str"), ("Str:     *", "Str"), ("Int 40*  Wil 50", "Wil"),
        ("Int 40*  Wil 50", "Int"), ("Wil", "Wil"), ("Wil ", "Wil"), ("Wil*", "Wil"), ("", "Wil"), ("Agility 77 *", "Agility"),
        ("Agility 77 *", "agility"), ("Cha 12", "Cha"), ("Cha 12*", "Cha"), ("xx Cha*", "Cha"), ("Hea: 80  *", "Hea"),
        ("Hea: 80 x*", "Hea"), ("Str*Str", "Str"), ("Str Str*", "Str"), ("abc", ""), ("abc*", ""), ("*", "")]
for i, (s, label) in enumerate(BUFF):
    cases.append({"id": "buffed-%03d" % i, "read": {}, "proc": "PastedStatIsBuffed", "args": [s, label]})

# ---------------------------------------------------------------------------------- data pool
def item_row(n):
    r = items[n]
    return {"n": n, "name": r["Name"], "enc": r["Encum"], "t": r["ItemType"], "w": r["Worn"], "ac": r["ArmourClass"],
            "dr": r["DamageResist"], "wt": r["WeaponType"], "at": r["ArmourType"], "acc": r["Accy"],
            "cr": [r["ClassRest-%d" % k] for k in range(10)], "ab": ab(r, 20)}


def spell_row(n):
    r = spells[n]
    return {"n": n, "req": r["ReqLevel"], "cap": r["Cap"], "mn": r["MinBase"], "mx": r["MaxBase"], "mni": r["MinInc"],
            "mnl": r["MinIncLVLs"], "mxi": r["MaxInc"], "mxl": r["MaxIncLVLs"], "dur": r["Dur"], "di": r["DurInc"],
            "dl": r["DurIncLVLs"], "ab": ab(r, 10)}


data = {"items": [item_row(n) for n in sorted(x for x in used_items if x in items)],
        "spells": [spell_row(n) for n in sorted(used_spells)],
        "classes": [{"n": n, "wt": classes[n]["WeaponType"], "at": classes[n]["ArmourType"], "ab": ab(classes[n], 10)}
                    for n in sorted(used_classes)],
        "races": [{"n": n, "hp": races[n]["HPPerLVL"], "ab": ab(races[n], 10)} for n in sorted(used_races)]}

FRM = {
    "file": "frmMain.frm",
    "decls": ["bless_Stats", "bless_StatText", "char_StatAdjustments", "bDontRefresh", "bDontRefreshInvenStats"],
    "procs": ["CalcCharacterStats", "InvenResetStats", "AdjMainStatBonus", "RefreshCharBless", "InvenCalcEncum",
              "ItemIsUsableByChar", "ItemIsCarryActive", "ItemIsFlaggedCarried", "AccumItemStatBonus", "PastedStatIsBuffed"],
    "cuts": {"CalcCharacterStats": {"cut": "'stealth", "append": [
        "frmMain.tsProbe(0).Caption = nEncumPCT",
        "frmMain.tsProbe(1).Caption = nShadowAC",
        "frmMain.tsProbe(2).Caption = nWeaponHitMagic",
        "frmMain.tsProbe(3).Caption = bClassStealth",
        "frmMain.tsProbe(4).Caption = bRaceStealth"]}},
}
READ = {"stat": "%c.lblInvenCharStat.Caption[0..46]", "tags": "%c.txtCharStats.Tag[0..5]", "probe": "%c.tsProbe.Caption[0..4]"}
for g in ("nGlobalCharAccyItems", "nGlobalCharAccyAbils", "nGlobalCharAccyOther", "nGlobalCharPlusDodge", "nGlobalCharVileWard",
          "nGlobalCharShadowAC", "nGlobalCharPlusMR", "nGlobalCharWornArmourType", "nGlobalCharHitMagicNonWeapon",
          "nGlobalCharWeaponNumber", "nGlobalCharWeaponEncum", "nGlobalCharWeaponAccy", "nGlobalCharWeaponCrit",
          "nGlobalCharWeaponSTR", "nGlobalCharWeaponAGI", "nGlobalCharWeaponMaxDmg", "nGlobalCharWeaponBSaccy",
          "nGlobalCharWeaponBSmindmg", "nGlobalCharWeaponBSmaxdmg", "nGlobalCharWeaponPunchSkill", "nGlobalCharWeaponPunchAccy",
          "nGlobalCharWeaponPunchDmg", "nGlobalCharWeaponKickSkill", "nGlobalCharWeaponKickAccy", "nGlobalCharWeaponKickDmg",
          "nGlobalCharWeaponJkSkill", "nGlobalCharWeaponJkAccy", "nGlobalCharWeaponJkDmg", "nGlobalCharWeaponStealth"):
    READ[g[len("nGlobalChar"):]] = "%g." + g

scn = {
    "family": "gear",
    "description": "Gear aggregation I (FAM-04): the first half of frmMain CalcCharacterStats, RefreshCharBless, "
                   "ItemIsUsableByChar, ItemIsCarryActive, ItemIsFlaggedCarried, AccumItemStatBonus, PastedStatIsBuffed",
    "notes": [
        "CalcCharacterStats is run from frmMain.frm (scenario key frm) and cut before its 'stealth section; the cut is "
        "where P3-18 takes over. The appended lines copy the locals P3-18 needs into the tsProbe control: "
        "0 nEncumPCT, 1 nShadowAC, 2 nWeaponHitMagic, 3 bClassStealth, 4 bRaceStealth.",
        "stat = lblInvenCharStat(0..46).Caption, tags = txtCharStats(0..5).Tag (the stat with bless and gear applied).",
        "Control and global inputs are in each case's forms/globals; combo boxes hold the selected item in ListIndex 0 "
        "(cmbEquip, cmbCharBless) or ListIndex 1 with ItemData(1) = the number (class, race). A control array "
        "entry that is not listed reads as empty.",
        "Rows numbered 9001 and up are invented (db_extra) to reach branches no stock row has; data holds every row the "
        "cases use, for the Rust test.",
    ],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Spells": EXTRA_SPELLS, "Races": EXTRA_RACES, "Classes": EXTRA_CLASSES},
    "data": data,
    "forms": {"frmMain": {"lblDatVer": {"Caption": "v1.11p"}, "lblInvenCharStat": {"$array": {"46": {}}}, "tsProbe": {"$array": {"4": {}}}, "lblLabelArray": {"$array": {"24": {}}}, "lblInvenStats": {"$array": {"46": {}}},
                          "chkEquipHold": {"$array": {"19": {}}}, "picStats": {"$array": {"1": {}}},
                          "lvItemManager": {"$listview": []}, "cmbGlobalAlignment": {"ListIndex": 0}}},
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d items, %d spells" % (len(cases), len(data["items"]), len(data["spells"])), file=sys.stderr)

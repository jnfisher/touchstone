"""Writes fixtures/scenarios/equip.json: python3 -I tools/oracle/gen/equip.py fixtures/scenarios/equip.json
EQ tab rules (card P4-17): frmMain InvenEquipItem, InvenAddEquip, InvenSetupEquip, InvenClear, cmbEquip_Click,
LoadInvenItems, FilterInvenItems, cmdArmNextSlot_Click and the red-box flags of CalcCharacterStats.
Deterministic (splitmix64). Real Items/Classes rows from the stock SQLite plus invented rows numbered 9001+; the
rows the cases can touch are written once into the scenario's "data" so the Rust test can rebuild them."""
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


rng = Rng(0xE0171)

# ---------------------------------------------------------------------------------- rows
items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}


def ab(row, n):
    return [x for x in ([row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n)) if x[0] != 0]


def item(n, name, **kw):
    r = {"Number": n, "Name": name, "Encum": 0, "ItemType": 0, "Worn": 0, "ArmourClass": 0, "DamageResist": 0,
         "Accy": 0, "WeaponType": 0, "ArmourType": 0, "Min": 0, "Max": 0, "Speed": 0, "StrReq": 0, "In Game": 1}
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
    item(9001, "tst everywhere cloak", Worn=1),
    item(9002, "tst worn seventeen", Worn=17),
    item(9003, "tst eyes", Worn=18),
    item(9004, "tst face", Worn=19),
    item(9005, "tst ring thirteen", Worn=13),
    item(9006, "tst worn twenty", Worn=20),
    item(9007, "tst worn negative", Worn=-1),
    item(9008, "tst type three head", ItemType=3, Worn=2),
    item(9009, "tst special charm", ItemType=10, Worn=0),
    item(9010, "tst two-hand blade", ItemType=1, WeaponType=1),
    item(9011, "tst great axe", ItemType=1, WeaponType=3),
    item(9012, "tst mace", ItemType=1, WeaponType=0),
    item(9013, "tst hidden blade", ItemType=1, WeaponType=2, **{"In Game": 0}),
    item(9014, "tst hidden ring", Worn=4, **{"In Game": 0}),
    item(9015, "tst wrist a", Worn=14),
    item(9016, "tst wrist b", Worn=14),
    item(9017, "tst ring a", Worn=4),
    item(9018, "tst ring b", Worn=13),
    item(9019, "tst ring c", Worn=4),
    item(9020, "tst class helm", Worn=2, rest=[3, 4]),
    item(9021, "tst level ring", Worn=4, abil=[(135, 10)]),
    item(9022, "tst low level ring", Worn=13, abil=[(136, 5)]),
    item(9023, "tst good cloak", Worn=7, abil=[(97, 1)]),
    item(9024, "tst evil cloak", Worn=7, abil=[(98, 1)]),
    item(9025, "tst not evil cape", Worn=7, abil=[(111, 1)]),
    item(9026, "tst class ok", Worn=2, rest=[7], abil=[(59, 1)]),
    item(9027, "tst magic plate", Worn=11, abil=[(28, 1)]),
    item(9028, "tst shield", Worn=12),
    item(9029, "tst shield two", Worn=12),
    item(9030, "tst nowhere", Worn=0),
    item(9031, "tst neutral robe", Worn=11, abil=[(112, 1)]),
    item(9032, "tst not good gloves", Worn=3, abil=[(110, 1)]),
    item(9033, "tst min level two hand", ItemType=1, WeaponType=3, abil=[(135, 25)]),
]
EXTRA_CLASSES = [
    {"Number": 9001, "Name": "tst no magic", "Abil-0": 51, "AbilVal-0": 0},
    {"Number": 9002, "Name": "tst plain", "Abil-0": 0, "AbilVal-0": 0},
]
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          **{"ClassRest-%d" % i: 0 for i in range(10)}, **r}
for r in EXTRA_CLASSES:
    classes[r["Number"]] = {**{k: 0 for k in classes[1]}, **r}

EQ_ITEMS = {n: r for n, r in items.items() if r["ItemType"] in (0, 1)}


def by_worn(w, it=0):
    return [n for n, r in EQ_ITEMS.items() if r["ItemType"] == it and r["Worn"] == w]


def first(xs, k=0):
    return xs[k % len(xs)]


used_items = set()
used_classes = set()


def use(n):
    used_items.add(n)
    return n


def eq(n, unequip=False):
    return {"call": "InvenEquipItem", "args": [use(n), False, unequip]}


SETUP = {"call": "InvenSetupEquip"}
LOAD = {"call": "LoadInvenItems"}
READ_STATE = {"eq": "%g.nEquippedItem", "prompt": "%g.bPromptSave", "lists": "%combo.cmbEquip"}
READ_LISTS = {"eq": "%g.nEquippedItem", "lists": "%combo.cmbEquip"}
READ_EQ = {"eq": "%g.nEquippedItem"}

cases = []


def case(cid, steps, globs=None, forms=None, read=None):
    c = {"id": cid, "steps": steps}
    if globs:
        c["globals"] = globs
    if forms:
        c["forms"] = {"frmMain": forms}
    c["read"] = read or READ_STATE
    cases.append(c)


def add_step(n):
    r = items[n]
    return {"call": "InvenAddEquip", "args": [use(n), r["Name"], r["ItemType"], r["Worn"]]}


def select(slot, n, name):
    return {"select": "cmbEquip", "index": slot, "item": n, "as": name}


# ---------------------------------------------------------------------------------- A. slot mapping
for w in range(-1, 22):
    pool = by_worn(w)
    if not pool:
        continue
    for k in range(min(len(pool), 2)):
        n = pool[k * (len(pool) // 2 or 1)]
        case("map-armour-w%d-%d" % (w, k), [SETUP, eq(n)])
for wt in range(4):
    pool = by_worn(0, 1) + [n for n, r in EQ_ITEMS.items() if r["ItemType"] == 1 and r["WeaponType"] == wt]
    pool = [n for n in pool if EQ_ITEMS[n]["WeaponType"] == wt]
    for k in range(2):
        case("map-weapon-wt%d-%d" % (wt, k), [SETUP, eq(pool[k * (len(pool) // 2)])])
for t in (2, 3, 4, 5, 6, 7, 8, 9, 10):
    pool = [n for n, r in items.items() if r["ItemType"] == t]
    if pool:
        case("map-type%d" % t, [SETUP, eq(pool[0])])
case("map-zero", [SETUP, eq(1), {"call": "InvenEquipItem", "args": [0, False, False]}])
case("map-missing", [SETUP, {"call": "InvenEquipItem", "args": [999999, False, False]}])
case("map-negative", [SETUP, {"call": "InvenEquipItem", "args": [-5, False, False]}])
case("map-zero-unequip", [SETUP, {"call": "InvenEquipItem", "args": [0, False, True]}])
nowhere = first(by_worn(0))
case("map-nowhere-enabled", [SETUP, eq(nowhere)], forms={"Enabled": True})
case("map-nowhere-disabled", [SETUP, eq(nowhere)], forms={"Enabled": False})
case("map-nowhere-extra", [SETUP, eq(9030)], forms={"Enabled": True})
# unequip of an item that is not equipped falls through to the normal equip
case("unequip-absent", [SETUP, eq(first(by_worn(2)), True)])
case("unequip-present", [SETUP, eq(first(by_worn(2))), eq(first(by_worn(2)), True)])
case("unequip-weapon", [SETUP, eq(first(by_worn(0, 1))), eq(first(by_worn(0, 1)), True)])
case("equip-replaces", [SETUP, eq(by_worn(2)[0]), eq(by_worn(2)[1])])
case("equip-same-twice", [SETUP, eq(by_worn(2)[0]), eq(by_worn(2)[0])])

# ---------------------------------------------------------------------------------- B. fingers and wrists
RINGS = [9017, 9018, 9019, first(by_worn(4)), first(by_worn(13))]
WRISTS = [9015, 9016, first(by_worn(14)), first(by_worn(14), 3)]


def pair_state(kind, a, b):
    """steps that leave slots (lo, hi) as: none, only lo, only hi, both"""
    lo, hi = kind
    if a == "none":
        return []
    if a == "lo":
        return [eq(b[0])]
    if a == "hi":
        return [add_step(b[1]), select(hi, b[1], "pick")]
    return [eq(b[0]), eq(b[1])]


for st in ("none", "lo", "hi", "both"):
    for newk, new in (("new", 2), ("lo", 0), ("hi", 1), ("other", 3)):
        for un in (False, True):
            case("finger-%s-%s-%s" % (st, newk, "un" if un else "eq"),
                 [SETUP] + pair_state((9, 10), st, RINGS) + [eq(RINGS[new], un)])
for st in ("none", "lo", "hi", "both"):
    for newk, new in (("new", 2), ("lo", 0), ("hi", 1), ("other", 3)):
        for un in (False, True):
            for second in (False, True):
                case("wrist-%s-%s-%s-%s" % (st, newk, "un" if un else "eq", "w2" if second else "w1"),
                     [SETUP] + pair_state((6, 7), st, WRISTS) + [eq(WRISTS[new], un)],
                     globs={"bInvenUse2ndWrist": second})
# the second wrist already equipped with the first used for equips when use-2nd is on or off
for second in (False, True):
    case("wrist-three-%s" % second, [SETUP, eq(WRISTS[0]), eq(WRISTS[1]), eq(WRISTS[2]), eq(WRISTS[3])],
         globs={"bInvenUse2ndWrist": second})
    case("finger-three-%s" % second, [SETUP, eq(RINGS[0]), eq(RINGS[1]), eq(RINGS[2]), eq(RINGS[3]), eq(RINGS[4])],
         globs={"bInvenUse2ndWrist": second})

# ---------------------------------------------------------------------------------- C. select (cmbEquip_Click)
case("select-none-clears", [SETUP, eq(first(by_worn(2))), select(0, 0, "pick")])
case("select-in-list", [SETUP, add_step(first(by_worn(2))), select(0, first(by_worn(2)), "pick")])
case("select-wrong-slot", [SETUP, add_step(first(by_worn(2))), select(1, first(by_worn(2)), "pick")])
case("select-ring-slot10", [SETUP, add_step(9017), select(10, 9017, "pick")])
case("select-ring-both", [SETUP, add_step(9017), select(9, 9017, "p1"), select(10, 9017, "p2")])
case("select-wrist-slot7", [SETUP, add_step(9015), select(7, 9015, "pick")])
case("select-weapon", [SETUP, add_step(9010), select(16, 9010, "pick")])
case("select-offhand-weapon", [SETUP, add_step(9010), select(15, 9010, "pick")])
case("select-shield-weapon-slot", [SETUP, add_step(9028), select(16, 9028, "pick")])
case("select-same-again", [SETUP, add_step(9028), select(15, 9028, "a"), select(15, 9028, "b")])
case("select-switch", [SETUP, add_step(9028), add_step(9029), select(15, 9028, "a"), select(15, 9029, "b")])
case("select-after-load", [SETUP, LOAD, select(2, first(by_worn(8)), "a"), select(4, first(by_worn(11)), "b"),
                           select(9, 9017, "c"), select(16, first(by_worn(0, 1)), "d")], read=READ_EQ)
use(first(by_worn(8)))
use(first(by_worn(11)))
use(first(by_worn(0, 1)))
use(9017)

# ---------------------------------------------------------------------------------- D. lists and the global filter
for ig in (False, True):
    case("load-ingame-%s" % ig, [SETUP, LOAD], globs={"bOnlyInGame": ig}, read=READ_LISTS)
    case("load-equip-after-%s" % ig, [SETUP, LOAD, eq(9013), eq(9014), eq(9010), eq(9028)], globs={"bOnlyInGame": ig},
         read=READ_LISTS)


def combo_sel(n):
    return {"$array": {"0": {"ListIndex": 1, "ItemData": {"1": n}} if n else {"ListIndex": 0}}}


def char_forms(cls, level, align, minlvl=None):
    f = {"cmbGlobalClass": combo_sel(cls), "txtGlobalLevel": {"$array": {"0": {"Text": str(level)}}},
         "cmbGlobalAlignment": {"ListIndex": align}}
    if cls:
        used_classes.add(cls)
    if minlvl is not None:
        f["txtGlobalMinLVL"] = {"Text": str(minlvl)}
    return f


FILTER = {"call": "FilterInvenItems", "args": []}
UNFILTER = {"call": "FilterInvenItems", "args": [True]}
READ_FILTER = {"eq": "%g.nEquippedItem", "lists": "%combo.cmbEquip", "prompt": "%g.bPromptSave"}
for cls in [0, 1, 2, 3, 4, 7, 9, 11, 12, 15, 9001, 9002]:
    for level, align in ((1, 0), (20, 1), (60, 3)):
        case("filter-c%d-l%d-a%d" % (cls, level, align), [SETUP, LOAD, FILTER], forms=char_forms(cls, level, align),
             globs={"bOnlyInGame": level != 20}, read=READ_FILTER)
for level, align, minlvl in ((30, 2, 10), (30, 2, 40), (999, 0, None), (999, 1, None), (0, 0, None), (12, 3, 5)):
    case("filter-misc-l%d-a%d-m%s" % (level, align, minlvl), [SETUP, LOAD, FILTER], forms=char_forms(0, level, align, minlvl),
         read=READ_FILTER)
for i in range(14):
    cls = rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 9001])
    level, align = rng.choice([1, 5, 10, 25, 40, 70, 150]), rng.between(0, 3)
    pre = [first(by_worn(2), rng.below(40)), first(by_worn(11), rng.below(60)), first(by_worn(0, 1), rng.below(60)),
           first(by_worn(4), rng.below(10)), first(by_worn(14), rng.below(10)), rng.choice([9020, 9021, 9023, 9024, 9026, 9027, 9033])]
    steps = [SETUP, LOAD] + [eq(n) for n in pre[: rng.between(2, 6)]] + [FILTER]
    if i % 3 == 0:
        steps.append(UNFILTER)
    case("filter-equipped-%02d" % i, steps, forms=char_forms(cls, level, align), globs={"bOnlyInGame": bool(i % 2)}, read=READ_FILTER)
case("filter-then-equip-unusable", [SETUP, LOAD, FILTER, eq(9033), eq(9020)], forms=char_forms(3, 10, 1), read=READ_FILTER)
case("filter-then-unfilter", [SETUP, LOAD, FILTER, UNFILTER], forms=char_forms(3, 10, 1), read=READ_FILTER)

# ---------------------------------------------------------------------------------- E. clear
HOLD_POOL = [first(by_worn(2)), first(by_worn(1)) if by_worn(1) else 9001, first(by_worn(8)), first(by_worn(11)), 9017, 9018,
             first(by_worn(0, 1)), 9028, 9015, 9016]
for i in range(10):
    holds = {str(k): {"Value": 1 if rng.chance(40) else 0} for k in range(20)}
    pool = HOLD_POOL[: rng.between(3, len(HOLD_POOL))]
    case("clear-unheld-%02d" % i, [SETUP] + [eq(n) for n in pool] + [{"call": "InvenClear"}],
         forms={"chkEquipHold": {"$array": holds}}, read={**READ_EQ, "sel": "%combo.cmbEquip"})
case("clear-all-held", [SETUP, eq(9017), eq(9018), eq(first(by_worn(2))), {"call": "InvenClear"}],
     forms={"chkEquipHold": {"$array": {str(k): {"Value": 1} for k in range(20)}}}, read=READ_STATE)
case("setup-after-equips", [SETUP, eq(9017), eq(9018), eq(first(by_worn(2))), SETUP], read=READ_STATE)

# ---------------------------------------------------------------------------------- F. random sequences
POOL_ALL = [n for n in EQ_ITEMS if n < 9000][:]
SEQ_SLOTS = [by_worn(w) for w in (2, 15, 8, 7, 11, 6, 14, 3, 4, 13, 5, 10, 9, 12, 16, 12, 18, 19, 1)]
SEQ_SLOTS = [p for p in SEQ_SLOTS if p]
SEQ_WEAPONS = by_worn(0, 1)


def rand_item():
    r = rng.below(100)
    if r < 25:
        return rng.choice(RINGS)
    if r < 45:
        return rng.choice(WRISTS)
    if r < 60:
        return rng.choice(SEQ_WEAPONS + [9010, 9011, 9012])
    if r < 70:
        return rng.choice([9001, 9002, 9003, 9004, 9005, 9006, 9007, 9008, 9009, 9028, 9029, 9030])
    return rng.choice(rng.choice(SEQ_SLOTS))


for i in range(120):
    steps = [SETUP] + ([LOAD] if i % 6 == 0 else [])
    done = []
    for _ in range(rng.between(2, 9)):
        r = rng.below(100)
        if r < 60 or not done:
            n = rand_item()
            done.append(n)
            steps.append(eq(n))
        elif r < 80:
            steps.append(eq(rng.choice(done), True))
        elif r < 90:
            n = rand_item()
            steps.append(add_step(n))
            steps.append(select(rng.between(0, 19), n, "p%d" % len(steps)))
        else:
            steps.append(eq(rand_item(), True))
    case("seq-%03d" % i, steps, globs={"bInvenUse2ndWrist": bool(i % 2), "bOnlyInGame": bool(i % 5 == 0)},
         read=READ_EQ if i % 6 == 0 else READ_STATE, forms={"Enabled": bool(i % 7)})

# ---------------------------------------------------------------------------------- G. warning flags
# CalcCharacterStats paints chkEquipHold red for an item the character cannot use (ItemIsUsableByChar with the
# minimum-item-level rule off) and for weapon + off-hand when the weapon is two-handed.
STAT_FORMS = {
    "txtCharStats": {"$array": {str(i): {"Text": str(v)} for i, v in enumerate([60, 55, 50, 65, 70, 45])}},
    "cmbGlobalRace": combo_sel(2), "cmbCharBless": {"$array": {}},
}
READ_FLAGS = {"eq": "%g.nEquippedItem", "flags": "%c.chkEquipHold.ForeColor[0..19]"}


def flag_case(cid, equips, cls, level, align, minlvl=None):
    f = char_forms(cls, level, align, minlvl)
    f["cmbGlobalRace"] = combo_sel(2)
    case(cid, [SETUP] + [eq(n) for n in equips] + [{"call": "CalcCharacterStats"}], forms=f, read=READ_FLAGS)


FLAG_ITEMS = [9020, 9021, 9022, 9023, 9024, 9025, 9026, 9027, 9031, 9032, 9033, 9010, 9011, 9028, 9017, 9018]
for i, n in enumerate(FLAG_ITEMS):
    for cls, level, align in ((1, 20, 1), (3, 5, 3), (4, 30, 2), (9001, 12, 0)):
        flag_case("flags-%d-c%d-l%d" % (n, cls, level), [n], cls, level, align)
for i in range(30):
    pool = [rand_item() for _ in range(rng.between(2, 8))]
    pool = [n for n in pool if items[n]["Worn"] != 0 or items[n]["ItemType"] == 1]
    cls = rng.choice([0, 1, 2, 3, 4, 5, 7, 9, 11, 12, 15, 9001, 9002])
    flag_case("flags-random-%02d" % i, pool, cls, rng.choice([1, 8, 20, 35, 80]), rng.between(0, 3), rng.choice([None, None, 15, 40]))
flag_case("flags-twohand-offhand", [9010, 9028], 1, 20, 0)
flag_case("flags-twohand-alone", [9010], 1, 20, 0)
flag_case("flags-onehand-offhand", [9012, 9028], 1, 20, 0)
flag_case("flags-twohand-type3", [9011, 9029], 1, 20, 0)
flag_case("flags-offhand-then-twohand", [9028, 9011], 1, 20, 0)
flag_case("flags-none", [], 1, 20, 0)
flag_case("flags-nowhere", [9030], 1, 20, 0)
flag_case("flags-minlevel-ignored", [9021, 9033], 1, 40, 0, 50)
for lvl in (1, 10, 11, 24, 25, 26):
    flag_case("flags-level-%d" % lvl, [9021, 9033, 9022], 1, lvl, 0)
flag_case("flags-admin", [9021, 9033], 0, 999, 0)

# ---------------------------------------------------------------------------------- H. next armour slot
WORN_NAMES = ["Anywhere", "Nowhere", "Head", "Hands", "Finger", "Feet", "Arms", "Back", "Neck", "Legs", "Waist", "Torso",
              "Off-Hand", "Wrist", "Ears", "Worn", "Eyes", "Face", "Everywhere"]
NEXT_SUBS = {
    "Me.Enabled = False": "",
    "Me.MousePointer = vbHourglass": "",
    "Call cmdFilter_Click(4)": "",
    "If lvArmour.ListItems.count = 0 And nTry < (cmbArmourWorn.ListCount - 1) Then GoTo tryagain:":
        'If Mid(frmMain.lblEmpty.Tag, cmbArmourWorn.ListIndex + 1, 1) = "1" And nTry < (cmbArmourWorn.ListCount - 1) Then GoTo tryagain:',
    "Me.Enabled = True": "",
    "Me.MousePointer = vbNormal": "",
}


def next_case(cid, count, start, empty):
    """empty: string of 0/1 per list index, 1 = the filter finds no armour at that index"""
    names = (WORN_NAMES * 2)[:count]
    cases.append({"id": cid, "steps": [{"call": "cmdArmNextSlot_Click"}],
                  "forms": {"frmMain": {"cmbArmourWorn": {"$combo": True, "List": names, "ListIndex": start},
                                        "lblEmpty": {"Tag": empty}}},
                  "read": {"index": "%c.cmbArmourWorn.ListIndex"}})


for count in (19, 12, 6, 3, 2, 1):
    for start in sorted({0, 1, count // 2, count - 2, count - 1} & set(range(count))):
        for pat in ("none", "next", "all", "alt", "rand"):
            if pat == "none":
                empty = "0" * count
            elif pat == "next":
                empty = "".join("1" if k == (start + 1) % count else "0" for k in range(count))
            elif pat == "all":
                empty = "1" * count
            elif pat == "alt":
                empty = "".join("1" if k % 2 else "0" for k in range(count))
            else:
                empty = "".join("1" if rng.chance(70) else "0" for _ in range(count))
            next_case("next-n%d-s%d-%s" % (count, start, pat), count, start, empty)

# ---------------------------------------------------------------------------------- data
data = {
    "items": [{"n": n, "name": items[n]["Name"], "t": items[n]["ItemType"], "w": items[n]["Worn"], "wt": items[n]["WeaponType"], "at": items[n]["ArmourType"],
               "ig": items[n]["In Game"],
               "cr": [c for c in (items[n]["ClassRest-%d" % k] for k in range(10)) if c],
               "ab": [a for a in ab(items[n], 20) if a[0] in (135, 136, 59, 28, 97, 98, 112, 110, 111, 113)]}
              for n in sorted(EQ_ITEMS)],
    "classes": [{"n": n, "wt": classes[n]["WeaponType"], "at": classes[n]["ArmourType"],
                 "ab": [a for a in ab(classes[n], 10) if a[0] == 51]} for n in sorted(classes)],
}

FRM = {
    "file": "frmMain.frm",
    "decls": ["bless_Stats", "bless_StatText", "char_StatAdjustments", "bDontRefresh", "bDontRefreshInvenStats",
              "nGlobalRefreshDelay", "nLastEquipIndex"],
    "procs": ["InvenAddEquip", "InvenEquipItem", "InvenSetupEquip", "InvenClear", "cmbEquip_Click", "LoadInvenItems",
              "FilterInvenItems", "cmdArmNextSlot_Click",
              "CalcCharacterStats", "InvenResetStats", "AdjMainStatBonus", "RefreshCharBless", "InvenCalcEncum",
              "ItemIsUsableByChar", "ItemIsCarryActive", "ItemIsFlaggedCarried", "AccumItemStatBonus", "PastedStatIsBuffed"],
    "stub_procs": ["SortInvenToolTips", "TruncateStatTips", "GetDodgeSummary"],
    "cuts": {"CalcCharacterStats": {"cut": "Call RefreshMagic", "append": []}},
    "subs": {
        "InvenSetupEquip": {"Call ExpandCombo(cmbEquip(x), HeightOnly, DoubleWidth, framNav(4).hWnd)": ""},
        "InvenClear": {"Call cmdEquipButtons_Click(514) 'clears manual stat adjustments without prompt": ""},
        "cmdArmNextSlot_Click": NEXT_SUBS,
    },
}

scn = {
    "family": "equip",
    "description": "EQ tab rules (FEAT-EQ): frmMain InvenEquipItem, InvenAddEquip, InvenSetupEquip, InvenClear, cmbEquip_Click, "
                   "LoadInvenItems, FilterInvenItems, cmdArmNextSlot_Click and the red-box flags CalcCharacterStats sets",
    "notes": [
        "The procedures are cut out of frmMain.frm (scenario key frm) and run against the form shim. cmbEquip is a ComboBox "
        "array: changing ListIndex raises cmbEquip_Click as in VB6, AddItem appends (the real combos are Sorted, which only "
        "reorders the display), Clear leaves nothing selected.",
        "Steps: InvenEquipItem(n, False, unequip), InvenAddEquip, a select step (the user picks the list entry whose ItemData "
        "is `item` in cmbEquip(`index`); `as` records whether the entry existed), InvenSetupEquip, LoadInvenItems, "
        "FilterInvenItems, InvenClear, CalcCharacterStats (cut at Call RefreshMagic) and cmdArmNextSlot_Click.",
        "Reads: eq = nEquippedItem, prompt = bPromptSave, lists = per slot ListIndex and the ItemData of every list entry "
        "(table order), flags = chkEquipHold(0..19).ForeColor (255 is red, 0 is black or untouched).",
        "InvenSetupEquip's ExpandCombo call and InvenClear's call to cmdEquipButtons_Click(514) (manual stat adjustments, not "
        "equipment) are blanked; cmdArmNextSlot_Click's filter call and the empty-list test are replaced by a read of "
        "frmMain.lblEmpty.Tag (one 0/1 per list index, 1 = the armour filter finds nothing there).",
        "Rows numbered 9001 and up are invented (db_extra); data holds the armour and weapon rows the cases can touch "
        "(only the abilities ItemIsUsableByChar reads) and every class row.",
    ],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Classes": EXTRA_CLASSES},
    "data": data,
    "forms": {"frmMain": {
        "lblDatVer": {"Caption": "v1.11p"}, "lblInvenCharStat": {"$array": {"46": {}}}, "tsProbe": {"$array": {"4": {}}},
        "lblLabelArray": {"$array": {"24": {}}}, "lblInvenStats": {"$array": {"46": {}}},
        "cmbEquip": {"$array": {str(k): {} for k in range(20)}, "$combo": "cmbEquip_Click"},
        "chkEquipHold": {"$array": {str(k): {} for k in range(20)}}, "picStats": {"$array": {"1": {}}},
        "lvItemManager": {"$listview": []}, "cmbGlobalAlignment": {"ListIndex": 0},
        "cmbGlobalClass": combo_sel(0), "cmbGlobalRace": combo_sel(0),
        "txtGlobalLevel": {"$array": {"0": {"Text": "20"}}},
        "txtCharStats": {"$array": {str(i): {"Text": str(v)} for i, v in enumerate([60, 55, 50, 65, 70, 45])}},
        "chkCharQuests": {"$array": {"11": {}}}, "cmbCharQuestOpts": {"$array": {"3": {}}}, "chkGlobalFilter": {"Value": 0},
        "txtCharHPRegen": {}, "txtCharManaRegen": {}, "Enabled": True}},
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d items" % (len(cases), len(data["items"])), file=sys.stderr)

"""Writes fixtures/scenarios/find_best.json: python3 -I tools/oracle/gen/find_best.py fixtures/scenarios/find_best.json
EQ tab Find Best and Next Best (card P4-18): frmMain InvenFindBest and its helpers, driven through the Find Best
menu handlers and cmdEquipButtons_Click (buttons 11 and 12), on lists built by LoadInvenItems, FilterInvenItems and
InvenAddEquip. Deterministic. Real Items rows from the stock SQLite plus invented rows numbered 9001+; every
armour and weapon row is written once into the scenario's "data" so the Rust test can rebuild the item table."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}


def item(n, name, **kw):
    r = {"Number": n, "Name": name, "Encum": 0, "ItemType": 0, "Worn": 0, "ArmourClass": 0, "DamageResist": 0,
         "Accy": 0, "WeaponType": 0, "ArmourType": 0, "Min": 0, "Max": 0, "Speed": 0, "StrReq": 0, "In Game": 1,
         "Limit": 0}
    for k, v in kw.items():
        if k == "abil":
            for i, (a, val) in enumerate(v):
                r["Abil-%d" % i] = a
                r["AbilVal-%d" % i] = val
        else:
            r[k] = v
    return r


COLD, FIRE = 3, 5
# worn: 2 head, 4/13 finger, 14 wrist, 12 off-hand; weapon type 1 and 3 are two-handed
EXTRA_ITEMS = [
    item(9001, "tst great axe", ItemType=1, WeaponType=3, ArmourClass=30, Encum=40),
    item(9002, "tst short sword", ItemType=1, WeaponType=0, ArmourClass=10, Encum=10),
    item(9003, "tst tower shield", Worn=12, ArmourClass=25, Encum=50),
    item(9004, "tst buckler", Worn=12, ArmourClass=5, Encum=5),
    item(9005, "tst big shield", Worn=12, ArmourClass=40, Encum=60),
    item(9006, "tst two-hand blade", ItemType=1, WeaponType=1, ArmourClass=22, Encum=30),
    item(9007, "tst dagger", ItemType=1, WeaponType=2, ArmourClass=6, Encum=3),
    item(9010, "tst cold ring big", Worn=4, abil=[(COLD, 20)]),
    item(9011, "tst cold ring mid", Worn=13, abil=[(COLD, 15)]),
    item(9012, "tst cold ring none", Worn=4, abil=[(FIRE, 9)]),
    item(9013, "tst cold ring small", Worn=4, abil=[(COLD, 5)]),
    item(9014, "tst cold ring twin", Worn=13, abil=[(COLD, 15)]),
    item(9015, "tst cold wrist big", Worn=14, abil=[(COLD, 12)]),
    item(9016, "tst cold wrist mid", Worn=14, abil=[(COLD, 7), (COLD, 1)]),
    item(9017, "tst cold wrist low", Worn=14, abil=[(COLD, 3)]),
    item(9020, "tst unique helm", Worn=2, ArmourClass=30, DamageResist=10, Limit=1, Encum=20),
    item(9021, "tst plain helm", Worn=2, ArmourClass=20, DamageResist=5, Limit=0, Encum=20),
    item(9022, "tst lesser helm", Worn=2, ArmourClass=12, DamageResist=4, Limit=0, Encum=20),
    item(9023, "tst tie helm light", Worn=2, ArmourClass=20, DamageResist=5, Encum=10),
    item(9024, "tst tie helm same", Worn=2, ArmourClass=20, DamageResist=5, Encum=10),
    item(9025, "tst tie helm heavy", Worn=2, ArmourClass=20, DamageResist=5, Encum=40),
    item(9026, "tst light helm", Worn=2, ArmourClass=12, DamageResist=4, Encum=0),
    item(9027, "tst zero helm", Worn=2),
    item(9030, "tst strength amulet", Worn=8, abil=[(46, 5), (48, 1)]),
    item(9031, "tst strength charm", Worn=8, abil=[(46, 3), (92, 4)]),
    item(9032, "tst punching gloves", Worn=3, abil=[(92, 6), (89, 2)]),
    item(9033, "tst kicking boots", Worn=5, abil=[(93, 3), (90, 5)]),
    item(9028, "tst unique cloak", Worn=7, ArmourClass=15, Limit=2),
    item(9029, "tst cloak", Worn=7, ArmourClass=9),
]
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          "ClassRest-0": 0, **r}
EQ_ITEMS = {n: r for n, r in items.items() if r["ItemType"] in (0, 1)}

cases = []


def combo_sel(n):
    return {"$array": {"0": {"ListIndex": 1, "ItemData": {"1": n}} if n else {"ListIndex": 0}}}


def char_forms(cls, level, align):
    return {"cmbGlobalClass": combo_sel(cls), "txtGlobalLevel": {"$array": {"0": {"Text": str(level)}}},
            "cmbGlobalAlignment": {"ListIndex": align}}


SETUP = {"call": "InvenSetupEquip"}
LOAD = {"call": "LoadInvenItems"}
FILTER = {"call": "FilterInvenItems", "args": []}
CAP = {"eq": "%g.nEquippedItem", "last": "%g.nInvenLastIndex", "excl": "%g.nInvenExcludedItems"}
READ = {"eq": "%g.nEquippedItem", "lists": "%combos.cmbEquip", "last": "%g.nInvenLastIndex",
        "excl": "%g.nInvenExcludedItems"}


def add(n):
    r = items.get(n) or {"Name": "tst ghost", "ItemType": 0, "Worn": 2}     # a list entry the item table lacks
    return {"call": "InvenAddEquip", "args": [n, r["Name"], r["ItemType"], r["Worn"]]}


def eq(n):
    return {"call": "InvenEquipItem", "args": [n, False, False]}


def pick(slot, n, name):
    return {"select": "cmbEquip", "index": slot, "item": n, "as": name}


MENUS = {"armour": "mnuFindBestArmour_Click", "attack": "mnuFindBestAttack_Click", "resist": "mnuFindBestResist_Click",
         "stats": "mnuFindBestStats_Click", "mystics": "mnuFindBestMystics_Click", "attrib": "mnuFindBestAttrib_Click"}


def fb(menu, index, tag):
    return [{"call": "cmdEquipButtons_Click", "args": [11]},
            {"call": MENUS[menu], "args": [index], "as": "fb" + tag, "capture": CAP}]


def nb(tag):
    return [{"call": "cmdEquipButtons_Click", "args": [12], "as": "nb" + tag, "capture": CAP}]


def case(cid, steps, globs=None, forms=None):
    c = {"id": cid, "steps": steps, "read": READ}
    g = {"bOnlyInGame": True, "bInvenUse2ndWrist": True}
    g.update(globs or {})
    c["globals"] = g
    if forms:
        c["forms"] = {"frmMain": forms}
    cases.append(c)


def holds(*slots):
    return {"chkEquipHold": {"$array": {str(k): {"Value": 1} for k in slots}}}


def merge(*ds):
    out = {}
    for d in ds:
        out.update(d)
    return out


# ------------------------------------------------------------------------------ A. criteria on real lists
WARRIOR = merge(char_forms(1, 50, 1))
FULL = [SETUP, LOAD, FILTER]
SWEEP = [("armour", [0, 1, 3, 4, 6]), ("attack", [0, 1, 6]), ("resist", [0, 3]), ("stats", [1, 7]), ("mystics", [0])]
for menu, idxs in SWEEP:
    for ix in idxs:
        case("war50-%s-%d" % (menu, ix), FULL + fb(menu, ix, "1"), forms=WARRIOR)
for cls, nm, picks in ((15, "mystic50", [("armour", 0), ("mystics", 5), ("stats", 1), ("resist", 0)]),
                       (12, "mage50", [("stats", 4), ("attack", 4)])):
    for menu, ix in picks:
        case("%s-%s-%d" % (nm, menu, ix), FULL + fb(menu, ix, "1"), forms=char_forms(cls, 50, 2))
# the whole item table: no filter, the unique-item box on and off
case("all-armour-combo", [SETUP, LOAD] + fb("armour", 0, "1"))
case("all-armour-combo-nolimit", [SETUP, LOAD] + fb("armour", 0, "1"), forms={"chkInvenNoLimited": {"Value": 1}})
case("war50-chain-combo", FULL + fb("armour", 0, "1") + nb("2") + nb("3") + nb("4"), forms=WARRIOR)
case("war50-chain-stats-nolimit", FULL + fb("stats", 1, "1") + nb("2") + nb("3"),
     forms=merge(WARRIOR, {"chkInvenNoLimited": {"Value": 1}}))
case("war50-hold-chain", FULL + [eq(n) for n in (9020, 9028)] + fb("armour", 1, "1") + nb("2"),
     forms=merge(WARRIOR, holds(0, 13, 15, 16)))


def curated(*nums):
    return [SETUP] + [add(n) for n in nums]


# stock data has no attribute abilities on armour, so these use invented rows
case("attrib-strength", curated(9030, 9031) + fb("attrib", 4, "1") + nb("2") + nb("3"))
case("attrib-agility", curated(9030, 9031) + fb("attrib", 0, "1"))
case("attrib-charm-none", curated(9030, 9031) + fb("attrib", 1, "1"))
case("mystic-punch-dmg", curated(9031, 9032, 9033) + fb("mystics", 5, "1"))
case("mystic-kick-acy", curated(9031, 9032, 9033) + fb("mystics", 2, "1") + nb("2"))

# ------------------------------------------------------------------------------ B. empty and worthless
case("empty-lists", [SETUP] + fb("armour", 0, "1"))
case("never-set-up", fb("armour", 0, "1"))
case("worthless-items", curated(9027, 9021, 9022) + fb("resist", 0, "1"))
case("missing-item-in-list", curated(9021, 99999, 9022) + [pick(0, 99999, "pk")] + fb("armour", 0, "1"))
case("next-after-missing", curated(9021, 9022, 99999) + [pick(0, 99999, "pk")] + fb("armour", 0, "1") + nb("2"))
case("next-no-find-first", curated(9021, 9022) + [eq(9021)] + nb("1"))
case("next-nothing-worn-no-find-first", curated(9021, 9022) + nb("1"))
case("greatermud-only-vileward", curated(9021) + fb("armour", 6, "1"))
case("bail-keeps-last-index", curated(9021) + fb("armour", 1, "1") + fb("armour", 6, "2") + nb("3"))

# ------------------------------------------------------------------------------ C. two-hander and off-hand
W2 = [9001, 9002]
case("2h-beats-offhand", curated(*W2, 9003) + fb("armour", 1, "1"))          # weapon 30 vs shield 25: weapon wins
case("offhand-beats-2h", curated(*W2, 9005) + fb("armour", 1, "1"))          # shield 40 vs weapon 30: 1-hander
case("offhand-beats-2h-no-1h", curated(9001, 9006, 9005) + fb("armour", 1, "1"))
case("equal-2h-offhand", curated(9006, 9002, 9003) + fb("armour", 1, "1"))  # 22 vs 25
case("2h-ties-offhand", curated(9001, 9002, 9003, 9004) + fb("armour", 2, "1"))
case("2h-offhand-held-picks-weapon", curated(*W2, 9005) + fb("armour", 1, "1"), forms=holds(15))
case("2h-weapon-held-offhand-picks", curated(*W2, 9005) + [eq(9001)] + fb("armour", 1, "1"), forms=holds(16))
case("both-held", curated(*W2, 9005) + [eq(9001), eq(9005)] + fb("armour", 1, "1"), forms=holds(15, 16))
case("2h-worn-pass-misses-pair", curated(9001, 9003, 9020) + [eq(9001), eq(9003)] + fb("armour", 1, "1"),
     forms=holds(15, 16))
case("2h-worn-offhand-new", curated(9001, 9005) + [eq(9001), eq(9004)] + fb("armour", 1, "1"), forms=holds(16))
case("1h-with-offhand", curated(9002, 9007, 9003) + fb("armour", 1, "1"))
case("2h-chain", curated(*W2, 9003, 9004, 9005, 9006, 9007) + fb("armour", 1, "1") + nb("2") + nb("3") + nb("4"))
case("2h-chain-offhand-wins", curated(9001, 9002, 9006, 9005, 9004, 9007) + fb("armour", 1, "1") + nb("2") + nb("3"))

# ------------------------------------------------------------------------------ D. paired fingers and wrists
RINGS = [9010, 9011, 9012, 9013, 9014]
WRISTS = [9015, 9016, 9017]
case("rings-dupe", curated(*RINGS) + fb("resist", 2, "1"))
case("rings-twins", curated(9011, 9014, 9012) + fb("resist", 2, "1"))
case("rings-one-only", curated(9010) + fb("resist", 2, "1"))
case("rings-best-worn-in-10", curated(*RINGS) + [eq(9013), eq(9010)] + fb("resist", 2, "1"))
case("rings-best-worn-in-9", curated(*RINGS) + [eq(9010), eq(9013)] + fb("resist", 2, "1"))
case("rings-best-worn-both-impossible", curated(9010, 9013) + [pick(9, 9010, "a"), pick(10, 9010, "b")] + fb("resist", 2, "1"))
case("rings-slot10-held", curated(*RINGS) + [eq(9013), eq(9010)] + fb("resist", 2, "1"), forms=holds(10))
case("rings-slot9-held", curated(*RINGS) + [eq(9013), eq(9010)] + fb("resist", 2, "1"), forms=holds(9))
case("rings-slot10-held-best", curated(*RINGS) + [eq(9013), pick(10, 9010, "a")] + fb("resist", 2, "1"), forms=holds(10))
case("rings-chain", curated(*RINGS) + fb("resist", 2, "1") + nb("2") + nb("3") + nb("4"))
for second in (False, True):
    tag = "w2" if second else "w1"
    case("wrists-dupe-" + tag, curated(*WRISTS) + fb("resist", 2, "1"), globs={"bInvenUse2ndWrist": second})
    case("wrists-worn-7-" + tag, curated(*WRISTS) + [pick(7, 9015, "a")] + fb("resist", 2, "1"),
         globs={"bInvenUse2ndWrist": second})
    case("wrists-worn-6-" + tag, curated(*WRISTS) + [pick(6, 9017, "a")] + fb("resist", 2, "1"),
         globs={"bInvenUse2ndWrist": second})
    case("wrists-chain-" + tag, curated(*WRISTS) + fb("resist", 2, "1") + nb("2") + nb("3"),
         globs={"bInvenUse2ndWrist": second})

# ------------------------------------------------------------------------------ E. unique items, ties, values
for nolimit in (0, 1):
    case("unique-nolimit-%d" % nolimit, curated(9020, 9021, 9022, 9028, 9029) + fb("armour", 0, "1"),
         forms={"chkInvenNoLimited": {"Value": nolimit}})
case("unique-chain", curated(9020, 9021, 9022) + fb("armour", 0, "1") + nb("2") + nb("3") + nb("4"))
case("tie-enc-ratio", curated(9021, 9023, 9025) + fb("armour", 0, "1"))
case("tie-first-listed", curated(9025, 9024, 9023) + fb("armour", 0, "1"))
case("tie-zero-enc", curated(9022, 9026) + fb("armour", 0, "1"))
case("tie-chain", curated(9023, 9024, 9025, 9021) + fb("armour", 0, "1") + nb("2") + nb("3") + nb("4") + nb("5"))
case("ac-only", curated(9020, 9021, 9022) + fb("armour", 1, "1"))
case("dr-only", curated(9020, 9021, 9022) + fb("armour", 2, "1"))
case("better-worn-kept-by-find", curated(9021, 9022) + [eq(9020)] + fb("armour", 1, "1"))
case("next-from-worn-not-in-list", curated(9021, 9022) + [eq(9020), eq(9021)] + fb("armour", 1, "1") + nb("2"))
case("chain-exhausts", curated(9021, 9022) + fb("armour", 1, "1") + nb("2") + nb("3") + nb("4"))
case("find-resets-exclusions", curated(9021, 9022) + fb("armour", 1, "1") + nb("2") + fb("armour", 1, "3") + nb("4"))

# ------------------------------------------------------------------------------ data
KEEP = ("Number", "Name")
data = {"items": [[n, r["ItemType"], r["WeaponType"], r["Encum"], r["ArmourClass"], r["DamageResist"], r["Accy"], r["Limit"],
                   [[r["Abil-%d" % k], r["AbilVal-%d" % k]] for k in range(20) if r["Abil-%d" % k] != 0], r["Worn"]]
                  for n, r in sorted(EQ_ITEMS.items())]}

FRM = {
    "file": "frmMain.frm",
    "decls": ["bDontRefresh", "nGlobalRefreshDelay", "bInvenNextBest", "nInvenLastIndex", "nInvenExcludedItems"],
    "enums": ["enmFindBest"],
    "types": ["TypeEquipWinner", "TypeFindBestItem"],
    "me_is_form": True,
    "procs": ["InvenFindBest", "InvenFindBestEval", "InvenFindBestAbils", "InvenFindBestDupeOK", "InvenFindBestItemValue",
              "InvenFindBestSelectItem", "InvenFindBestExclude", "InvenFindBestIsExcluded",
              "mnuFindBestArmour_Click", "mnuFindBestAttack_Click", "mnuFindBestAttrib_Click", "mnuFindBestMystics_Click",
              "mnuFindBestResist_Click", "mnuFindBestStats_Click", "cmdEquipButtons_Click", "cmbEquip_Click",
              "InvenAddEquip", "InvenEquipItem", "InvenSetupEquip", "LoadInvenItems", "FilterInvenItems",
              "ItemIsUsableByChar", "ItemIsCarryActive", "ItemIsFlaggedCarried"],
    "stub_procs": ["RefreshAll"],
    "subs": {
        "InvenSetupEquip": {"Call ExpandCombo(cmbEquip(x), HeightOnly, DoubleWidth, framNav(4).hWnd)": ""},
        "cmdEquipButtons_Click": {"PopupMenu mnuFindBest": ""},
        # a DAO Field cached with Set follows the cursor; the shim's Fields() returns the value at that moment, so the
        # two reads of the cached Field objects read the same field of the current record directly
        "InvenFindBestItemValue": {"nAbil = fldAbil(z).Value": 'nAbil = tabItems.Fields("Abil-" & z)',
                                   "nTotal = nTotal + fldAbilVal(z).Value":
                                       'nTotal = nTotal + tabItems.Fields("AbilVal-" & z)'},
        "InvenFindBest": {"Me.MousePointer = vbHourglass": "", "Me.MousePointer = vbDefault": ""},
    },
}

scn = {
    "family": "find_best",
    "description": "EQ tab Find Best and Next Best (frmMain InvenFindBest and helpers, through the menu handlers and "
                   "cmdEquipButtons_Click 11 and 12)",
    "notes": [
        "Steps run MME's own procedures cut from frmMain.frm. A Find Best is the pair cmdEquipButtons_Click(11) (clears "
        "bInvenNextBest, PopupMenu blanked) and one mnuFindBest*_Click(Index); a Next Best is cmdEquipButtons_Click(12), "
        "which replays nInvenLastIndex.",
        "Each Find Best / Next Best step captures eq = nEquippedItem, last = nInvenLastIndex (index, menu) and excl = "
        "nInvenExcludedItems (a leading 0 is the ReDim placeholder; an unallocated array reads as the error it raises).",
        "Lists: LoadInvenItems / FilterInvenItems (a level-50 character) or InvenSetupEquip plus InvenAddEquip. The final "
        "read has every combo's ItemData in list order; the oracle's combos are not Sorted, so ties go to the first entry "
        "in table or insertion order.",
        "InvenFindBestItemValue reads the cached DAO Field objects (`Set fldAbil(z) = tabItems.Fields(..)`), which follow "
        "the recordset's cursor in DAO; the shim's Fields() returns a value, so those two reads go to the recordset directly.",
        "Rows numbered 9001 and up are invented (db_extra); data holds every armour and weapon row as [number, type, "
        "weapon type, encum, AC, DR, accuracy, limit, [[ability, value]..], worn].",
    ],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS},
    "data": data,
    "forms": {"frmMain": {
        "lblDatVer": {"Caption": "v1.11p"},
        "cmbEquip": {"$array": {str(k): {} for k in range(20)}, "$combo": "cmbEquip_Click"},
        "chkEquipHold": {"$array": {str(k): {} for k in range(20)}}, "chkInvenNoLimited": {"Value": 0},
        "mnuFindBestArmour": {"$array": {"6": {}}},
        "lvItemManager": {"$listview": []}, "cmbGlobalAlignment": {"ListIndex": 0},
        "cmbGlobalClass": combo_sel(0), "cmbGlobalRace": combo_sel(0),
        "txtGlobalLevel": {"$array": {"0": {"Text": "20"}}}, "chkGlobalFilter": {"Value": 0}, "Enabled": True}},
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d items" % (len(cases), len(data["items"])), file=sys.stderr)

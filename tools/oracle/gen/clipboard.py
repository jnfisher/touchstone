"""Writes fixtures/scenarios/clipboard.json (FEAT-CTX clipboard texts):
python3 -I tools/oracle/gen/clipboard.py

Runs frmMain CopyChartoClip (with the InvenCopytoClipboard block it appends), CopyMonsterToClipboard and
CopyShopToClipboard, and modMain CopyLVLinetoClipboard, unchanged, over ListView and control stand-ins that hold
what the form's controls would. The clipboard text each leaves (or the VB error that stopped it) is the fixture.
The stock Classes and Races ExpTable values the character cases use are written into "data" for the Rust test.
Needs the converted stock SQLite (tools/mdb-fetch.sh). Deterministic (splitmix64)."""
import glob
import json
import os
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "clipboard.json")
cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
dbs = sorted(glob.glob(os.path.join(cache, "mdb", "eba20f06*.sqlite")))
if not dbs:
    sys.exit("no converted database; run tools/mdb-fetch.sh")
con = sqlite3.connect(dbs[0])
con.row_factory = sqlite3.Row
M64 = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def int(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)

    def chance(self, pct):
        return self.int(1, 100) <= pct

    def pick(self, xs):
        return xs[self.int(0, len(xs) - 1)]


rng = Rng(0x1C11B)
CLASSES = {r["Number"]: dict(r) for r in con.execute("select Number, Name, ExpTable from Classes order by Number")}
RACES = {r["Number"]: dict(r) for r in con.execute("select Number, Name, ExpTable from Races order by Number")}
ITEMS = {r["Number"]: dict(r) for r in con.execute("select Number, Name from Items order by Number")}

# the names of lblInvenStats(4..46), which the stat list of InvenCopytoClipboard reads; the form's design-time captions
STAT_CAPTIONS = {
    4: "Encumbrance:", 5: "HitPoints:", 6: "Mana:", 7: "Crits:", 8: "Dodge:", 9: "SpellCast:", 10: "Accuracy:",
    11: "Max Damage:", 12: "Hit Magic:", 13: "BS Accuracy:", 14: "BS Min Damage:", 15: "BS Max Damage:",
    16: "HitPoint Regen:", 17: "Mana Regen:", 18: "Perception:", 19: "Stealth:", 20: "Prot. Evil:", 21: "Traps:",
    22: "Picklocks:", 23: "Illuminate:", 24: "Magic Resist:", 25: "Stone Resist:", 26: "Water Resist:",
    27: "Fire Resist:", 28: "Cold Resist:", 29: "Lightning Res:", 30: "Min Damage:", 31: "Quickness:",
    32: "Prot. Good:", 33: "Spell Damage:", 34: "DMG", 35: "DMG", 36: "DMG", 37: "Skill", 38: "Skill", 39: "Skill",
    40: "Accy", 41: "ACCY", 42: "ACCY", 43: "Punch", 44: "Kick", 45: "JmpKck", 46: "Attk:",
}
INVISIBLE = {32, 43, 44, 45}      # lblInvenCharStat(x).Visible = False in the designer
cases = []


def case(prefix, **kw):
    kw["id"] = "%s%03d" % (prefix, sum(1 for c in cases if c["id"].startswith(prefix)))
    cases.append(kw)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


# ------------------------------------------------------------------------------------------------ a. character
WEAR_SLOTS = list(range(20))
STAT_BOXES = {0: 0, 1: 1, 2: 2, 3: 3, 4: 4, 5: 5}      # txtCharStats index
LABEL_FOR_STAT = {0: 1, 1: 4, 2: 24, 3: 2, 4: 23, 5: 3}  # lblLabelArray index per txtCharStats index


def char_case(name="Syntax Blackvail", cp="0", cls=6, race=3, level=55, hp="671", ac="82", dr="27", mana=None, sc="174",
              picks=None, stats=(130, 90, 82, 116, 110, 51), bonus=(0, 0, 0, 0, 0, 0), enc=("350", "480", "- Light (72%)"),
              equip=None, derived=None, stats_only=False):
    c = {}
    c["txtCharName"] = {"Text": name}
    c["lblStatCalc"] = {"Tag": cp}
    c["cmbGlobalRace"] = arr({0: {"Text": RACES[race]["Name"], "ListIndex": 2, "ItemData": {"2": race}}})
    c["cmbGlobalClass"] = arr({0: {"Text": CLASSES[cls]["Name"], "ListIndex": 2, "ItemData": {"2": cls}}})
    c["txtGlobalLevel"] = arr({0: {"Text": str(level)}})
    c["lblCharMaxHP"] = {"Tag": hp}
    inv = {i: {"Caption": "0", "Visible": i not in INVISIBLE} for i in range(0, 47)}
    inv[0]["Caption"], inv[1]["Caption"] = enc[0], enc[1]
    inv[2].update({"Caption": ac, "Tag": ac})
    inv[3].update({"Caption": dr, "Tag": dr})
    for k, v in (derived or {}).items():
        inv[k]["Caption"] = str(v)
    c["lblInvenCharStat"] = arr(inv)
    c["lblInvenStats"] = arr({i: {"Caption": t} for i, t in STAT_CAPTIONS.items()})
    c["lblEncumLevel"] = arr({1: {"Tag": enc[2]}})
    c["fraChar"] = arr({2: {"Enabled": mana is not None}, 3: {"Enabled": picks is not None}})
    c["lblCharMaxMana"] = {"Tag": "" if mana is None else mana}
    c["lblCharSC"] = {"Tag": sc}
    c["lblCharPicklocks"] = {"Tag": "" if picks is None else picks}
    c["txtCharStats"] = arr({i: {"Tag": str(stats[i])} for i in range(6)})
    c["lblLabelArray"] = arr({LABEL_FOR_STAT[i]: {"Tag": str(bonus[i])} for i in range(6)})
    eq = {i: {"ListIndex": 0, "Text": "(none)"} for i in WEAR_SLOTS}
    for slot, num in (equip or {}).items():
        eq[slot] = {"ListIndex": 1, "Text": "%s (%d)" % (ITEMS[num]["Name"], num)}
    c["cmbEquip"] = arr(eq)
    step = {"call": "CopyChartoClip"}
    if stats_only is not None:
        step["args"] = [stats_only]
    return {"forms": {"frmMain": c}, "steps": [step], "read": {"clip": "%clip"}}


def real_items(n, kind):
    q = {"weapon": "ItemType = 1", "armour": "ItemType = 0 and Worn > 0", "other": "ItemType not in (0, 1)"}[kind]
    return [r["Number"] for r in con.execute("select Number from Items where %s and Name != '' order by Number limit %d" % (q, n))]


W, A, O = real_items(6, "weapon"), real_items(12, "armour"), real_items(4, "other")
EQUIP_SETS = [
    {16: W[0]},
    {0: A[0], 4: A[1], 12: A[2], 13: A[3], 16: W[1], 15: A[4]},
    {i: A[i % len(A)] for i in range(0, 14)},
    {1: A[5], 2: A[6], 14: A[7], 16: W[2], 9: O[0], 10: O[1], 17: A[8], 18: A[9], 19: O[2]},
    {},
]
DERIVED = [
    {},
    {5: 671, 6: 126, 7: 12, 8: 64, 10: 330, 11: 62, 24: 94, 25: 13, 29: 9},
    {5: 100, 18: 73, 19: 14, 22: 40, 31: 5, 33: 20, 32: 4},
    {34: 12, 35: 14, 36: 9, 37: 40, 38: 41, 39: 42, 40: 7, 41: 8, 42: 9, 43: 3, 44: 4, 45: 5, 46: 100},
    {x: x * 3 for x in range(4, 47)},
]
for k in range(10):
    cls, race = rng.pick(list(CLASSES)), rng.pick(list(RACES))
    case("a", **char_case(
        name=rng.pick(["Syntax Blackvail", "Bob", "X", "A Very Long Name Indeed", "Name With Spaces Inside"]),
        cp=rng.pick(["0", "12", "-3", "135", "1250"]), cls=cls, race=race, level=rng.int(1, 255),
        hp=str(rng.int(30, 9999)), ac=str(rng.int(0, 400)), dr=str(rng.int(0, 150)),
        mana=rng.pick([None, "84", "126", "1500"]), sc=rng.pick(["0", "174", "35"]), picks=rng.pick([None, "0", "81", "130"]),
        stats=tuple(rng.int(10, 255) for _ in range(6)), bonus=tuple(rng.pick([0, 0, 5, -2]) for _ in range(6)),
        enc=(str(rng.int(0, 900)), str(rng.int(100, 1200)), rng.pick(["- None (0%)", "- Light (10%)", "- Medium (45%)", "- Heavy (99%)"])),
        equip=rng.pick(EQUIP_SETS), derived=rng.pick(DERIVED), stats_only=rng.pick([False, True, None])))
# the lines that String() cannot pad: a name or class that is too long gives VB error 5 and nothing is copied
case("a", **char_case(name="N" * 33))
case("a", **char_case(name="N" * 34))
case("a", **char_case(cp="12345678"))
case("a", **char_case(hp="1" * 13))
case("a", **char_case(cls=max(CLASSES, key=lambda n: len(CLASSES[n]["Name"])), race=max(RACES, key=lambda n: len(RACES[n]["Name"]))))
case("a", **char_case(equip=EQUIP_SETS[1], derived=DERIVED[1], stats_only=True))
case("a", **char_case(equip=EQUIP_SETS[2], derived=DERIVED[3], stats_only=False, mana="1500", picks="130"))
case("a", **char_case(level=1, equip={16: W[3]}, derived={}, stats_only=False, ac="0", dr="0"))
# an equipped item whose combo text has no "(": Mid() with a negative length
bad = char_case(equip={16: W[0]}, stats_only=False)
bad["forms"]["frmMain"]["cmbEquip"]["$array"]["16"]["Text"] = "no number here"
case("a", **bad)
named = char_case(equip={16: W[0], 0: A[0]}, stats_only=False)
named["forms"]["frmMain"]["cmbEquip"]["$array"]["16"]["Text"] = "mace (blessed) (%d)" % W[0]
case("a", **named)

# ------------------------------------------------------------------------------------------------ b. monster
MON_ROWS = [
    ["Name", "Giant Rat (12)"], ["Number", "12"], ["Level", "3"], ["HP", "20"], ["Exp", "100"], ["Hit Points Regen", "1 per 6 seconds"],
    ["Armour Class/DR", "10/0"], ["Max Damage Out", "6"], ["Alignment", "Evil"], ["", ""], ["Name of Group", "rats"],
    ["Abilities", "Hide, Backstab"],
]


def mon_lv(rows, name="lvMonsterDetail"):
    return {"cliplv": {"name": name, "columns": [["Info", 1500], ["Value", 3000]],
                       "rows": [{"Text": r[0], "Sub": r[1:]} for r in rows]}}


for rows, name_only in (
        (MON_ROWS, False), (MON_ROWS, True), (MON_ROWS[2:], True), (MON_ROWS[2:], False),
        ([["Name", "Dragon (the great)"], ["x", "y"]], True), ([["Name", "(unnamed)"]], True),
        ([["Name", "Plain"], ["A very long label here", "value"], ["Sixteen chars!!!", "v"], ["Seventeen chars!!!", "v"]], False),
        ([["Monster Name", " Padded (7)"]], True), ([["NAME", "Upper Case (1)"]], True), ([], False), ([], True),
        ([["Name", ""]], False), ([["", ""]], False), ([["Name", "x"], ["Broken"]], False), ([["Broken"], ["Name", "x"]], True)):
    case("b", forms={"frmMain": {}}, steps=[{"call": "CopyMonsterToClipboard", "args": [mon_lv(rows)] + ([True] if name_only else [])}],
         read={"clip": "%clip"})

# ------------------------------------------------------------------------------------------------ c. shop
TYPES = {10: "Weapons/Armour", 8: "Training", 5: "Healing"}


def shop_case(stock, training=False, name_only=False, selected=True, locs=("Room 1/497", "Room 3/12"), detail="Sells things.",
              shop_name="General Store", typ="Item", number="3"):
    lvs = {"cliplv": {"name": "lvShops", "columns": [["#", 500], ["Name", 2300], ["Type", 1050]],
                      "rows": [{"Text": number, "Sub": [shop_name, typ], "Sel": selected}, {"Text": "99", "Sub": ["Other", "Item"]}]}}
    if training:
        cols = [["LVL", 1000], ["Cost", 4000]]
        rows = [{"Text": str(lv), "Sub": [c]} for lv, c in stock]
    else:
        cols = [["#", 700], ["Name", 2000], ["Max", 550], ["Regen", 1650], ["Cost", 5000]]
        rows = [{"Text": str(n), "Sub": [nm, mx, rg, cost]} for n, nm, mx, rg, cost in stock]
    forms = {"lvShops": lvs["cliplv"] and {"$cliplv": lvs["cliplv"]},
             "txtShopDetail": {"Text": detail},
             "lvShopLoc": {"$cliplv": {"name": "lvShopLoc", "columns": [["Assigned To", 3700]], "rows": [{"Text": t} for t in locs]}},
             "lvShopDetail": {"$cliplv": {"name": "lvShopDetail", "columns": cols, "rows": rows}}}
    return {"forms": {"frmMain": forms}, "steps": [{"call": "CopyShopToClipboard", "args": [True] if name_only else []}],
            "read": {"clip": "%clip"}}


STOCK = [(217, "iron dagger", "5", "20% for 2 per 1h", "50 copper"), (218, "steel mace", "2", "no regen", "1,200 copper (12 Gold)"),
         (4, "x" * 29, "10", "100% for 1 per 1d2h30m", "1 Copper"), (300, "wooden shield (small)", "1", "no regen", "Free")]
TRAIN = [(1, "Free"), (2, "300 copper (3 Silver)"), (3, "12,000 copper (1.2 Platinum)")]
case("c", **shop_case(STOCK))
case("c", **shop_case(STOCK, name_only=True))
case("c", **shop_case(TRAIN, training=True, shop_name="Warrior Training Room", typ="Training", number="14"))
case("c", **shop_case(TRAIN, training=True, name_only=True))
case("c", **shop_case([], locs=()))
case("c", **shop_case(STOCK, selected=False))
case("c", **shop_case(STOCK, locs=("Room 1/1",), detail=""))
case("c", **shop_case(STOCK[:2], locs=("Room 1/497", "Room 3/12", "Room 4/4", "Room 5/5")))
case("c", **shop_case([(1, "y" * 30, "1", "no regen", "1 Copper")]))      # 30 characters: String(0) pads nothing
case("c", **shop_case([(1, "y" * 31, "1", "no regen", "1 Copper")]))      # 31: String(-1) is VB error 5
case("c", **shop_case([(1, "n", "1", "a long regen text here", "1 Copper")]))
case("c", **shop_case([(123456, "n", "1", "no regen", "1 Copper")]))
case("c", **shop_case(STOCK, shop_name="Wëird", detail="line one\r\nline two"))

# ------------------------------------------------------------------------------------------------ d. CopyLVLinetoClipboard


def lv(name, columns, rows):
    return {"cliplv": {"name": name, "columns": columns, "rows": rows}}


def r(text, subs=(), sel=True):
    d = {"Text": text, "Sub": list(subs)}
    if sel:
        d["Sel"] = True
    return d


WCOLS = [["#", 0], ["Name", 2200], ["Type", 900], ["Min", 600], ["Max", 600], ["Speed", 2], ["Avg", 700]]
WROWS = [r("217", ["iron dagger (217)", "1H", "3", "7", "30", "5"]), r("218", ["steel mace", "2H", "4", "12", "42", "8"], False),
         r("219", ["bronze axe", "1H", "2", "9", "35", "6.5"]), r("220", ["", "", "", "", "", ""])]
DETAIL = "A sharp blade.\r\nCompared to the equipped weapon: +2 damage"
REFS5 = [r("Shop", ["General Store (3)"], False), r("Monster", ["Giant Rat"], False), r("Lair", ["x"], False)]
REFS6 = REFS5 + [r("Chest", ["A", "B"], False), r("", ["blank first column"], False), r("Item", [], False)]


def lines(name, cols, rows, detail=None, locs=None, exclude=None, name_only=None):
    args = [lv(name, cols, rows),
            {"textbox": detail} if detail is not None else {"missing": True},
            lv("loc", [["Where", 3000]], locs) if locs is not None else {"missing": True},
            exclude if exclude is not None else -1]
    if name_only is not None:
        args.append(name_only)
    while args and args[-1] == {"missing": True}:
        args.pop()
    return {"forms": {"frmMain": {}}, "steps": [{"call": "CopyLVLinetoClipboard", "args": args}], "read": {"clip": "%clip"}}


case("d", **lines("lvWeapons", WCOLS, WROWS))
case("d", **lines("lvWeapons", WCOLS, WROWS, detail=DETAIL))
case("d", **lines("lvWeapons", WCOLS, WROWS, detail=DETAIL, locs=REFS5))
case("d", **lines("lvWeapons", WCOLS, WROWS, detail=DETAIL, locs=REFS6))
case("d", **lines("lvWeapons", WCOLS, WROWS, detail=DETAIL, locs=REFS6, name_only=True))
case("d", **lines("lvWeaponCompare", WCOLS, WROWS, locs=[], detail=""))
case("d", **lines("lvWeapons", WCOLS, [r("217", ["a"], False)], detail=DETAIL))
case("d", **lines("lvWeapons", WCOLS, []))
case("d", **lines("lvSpells", [["#", 0], ["Name", 1800], ["Lvl", 500], ["Mana", 500], ["Short", 800], ["Learn", 500], ["Cap", 500], ["Note", 900]],
                  [r("14", ["fire bolt", "5", "3", "fbol", "yes", "20", "n"]), r("15", ["ice bolt", "6", "4", "ibol", "", "21", "m"])],
                  detail="Damages for 5-9.", exclude=7))
case("d", **lines("lvSpells", [["#", 0], ["Name", 1800], ["Lvl", 500], ["Mana", 500], ["Short", 800], ["Learn", 500], ["Cap", 500], ["Note", 900]],
                  [r("14", ["fire bolt", "5", "3", "fbol", "yes", "20", "n"]), r("15", ["ice bolt", "6", "4", "ibol", "", "21", "m"])],
                  detail="d", exclude=7, name_only=True))
LOC_COLS = [["Where", 3000], ["What", 3000]]
LOC_ROWS = [r("Item: iron dagger", ["Shop: General Store"]), r("Spell: fire bolt", ["Room: 1/497"]), r("plain text", ["no colon"]),
            r("Room cmd 5", ["Room cmd north"]), r("x", ["Room action: pull the lever"]), r("a:b:c", ["d:e:f"])]
for nm in ("lvMapLoc", "lvSpellLoc", "lvShopLoc", "lvSpellCompareLoc"):
    case("d", **lines(nm, LOC_COLS[:1], LOC_ROWS, name_only=True))
    case("d", **lines(nm, LOC_COLS, LOC_ROWS))
for nm in ("lvWeaponLoc", "lvArmourLoc", "lvItemManagerLoc", "lvOtherItemLoc"):
    case("d", **lines(nm, LOC_COLS, LOC_ROWS, exclude=0, name_only=True))
case("d", **lines("lvWeaponLoc", LOC_COLS, [r("Item", [])], exclude=0, name_only=True))
case("d", **lines("lvClasses", [["#", 400], ["Name", 1500], ["Exp", 700]], [r("1", ["Warrior", "100"]), r("2", ["Mage", "120"])], name_only=True))
case("d", **lines("lvClasses", [["#", 400], ["Name", 1500], ["Exp", 700]], [r("1", ["Warrior", "100"]), r("2", ["Mage", "120"])]))
case("d", **lines("lvShopDetail", [["#", 700], ["Name", 2000], ["Max", 550]], [r("217", ["iron dagger", "5"])], name_only=True))
case("d", **lines("lvShopDetail", [["#", 700], ["Name", 2000], ["Max", 550]], [r("217", ["iron dagger", "5"]), r("218", ["mace", "2"])]))
many = [r(str(i), ["thing %d" % i, "1"]) for i in range(1, 106)]
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000], ["N", 500]], many))
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000], ["N", 500]], many, name_only=True))
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000]], [r("1", ["a"], False)], name_only=True))
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000]], [r("1", [], True)], name_only=True))     # Name column needs a sub-item: error 35600
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000]], [r("1", [], True)]))
case("d", **lines("lvWeapons", [["#", 100], ["Name", 1000]], [r("   ", ["  "], True)], name_only=True))    # blank text is not copied

used_c = sorted({c["forms"]["frmMain"]["cmbGlobalClass"]["$array"]["0"]["ItemData"]["2"] for c in cases if c["id"].startswith("a") and "cmbGlobalClass" in c["forms"]["frmMain"]})
used_r = sorted({c["forms"]["frmMain"]["cmbGlobalRace"]["$array"]["0"]["ItemData"]["2"] for c in cases if c["id"].startswith("a") and "cmbGlobalRace" in c["forms"]["frmMain"]})
scn = {
    "family": "clipboard",
    "description": "FEAT-CTX clipboard texts: frmMain CopyChartoClip (+InvenCopytoClipboard), CopyMonsterToClipboard, CopyShopToClipboard and "
                   "modMain CopyLVLinetoClipboard over ListView and control stand-ins.",
    "notes": [
        "Each case reads what the code left on the clipboard (%clip); a VB error that stopped a procedure shows in events and leaves it empty. "
        "SetClipboardText ignores a blank text, as the real procedure does.",
        "A ListView is {$cliplv: {name, columns: [[Text, Width]], rows: [{Text, Sub, Sel}]}}; a column of width 0 is hidden. "
        "CopyLVLinetoClipboard clicks each row first (frmMain.lvX_ItemClick); here that does nothing, so every row shares the one "
        "detail text and reference list the case gives. The Rust function takes them per row.",
        "The character cases fill the controls CopyChartoClip reads (Tag strings of the Char tab, lblInvenCharStat captions, the equipment "
        "combos); lblInvenCharStat(32, 43, 44, 45) are invisible, as in the designer. data holds the stock ExpTable of the classes and "
        "races the cases use, for CalcExpNeededByRaceClass. Generated by tools/oracle/gen/clipboard.py.",
    ],
    "database": True,
    "paste_procs": True,
    "frm": [{"procs": ["CopyChartoClip", "InvenCopytoClipboard", "CopyMonsterToClipboard", "CopyShopToClipboard"]}],
    "data": {"classes": [{"n": n, "name": CLASSES[n]["Name"], "exp": CLASSES[n]["ExpTable"]} for n in used_c],
             "races": [{"n": n, "name": RACES[n]["Name"], "exp": RACES[n]["ExpTable"]} for n in used_r]},
    "forms": {"frmMain": {}},
    "tolerance": {"default": 0},
    "cases": cases,
}
with open(OUT, "w", newline="\n") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("wrote", OUT, len(cases), "cases", file=sys.stderr)

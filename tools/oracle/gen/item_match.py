"""Writes fixtures/scenarios/item_match.json (FAM-22, second half: parsed-item matching and the Item
Manager rows):   python3 -I tools/oracle/gen/item_match.py

Runs modItemParse (AddSectionItems, PopulateItemManagerFromParsed, LV_AddRowByItemNumber and the shop
pricing under them), modListViewExt ParseActionAndQty and modMMudDatabase GetShopRoomNames over the
stock database plus a few invented rows (numbers 9001 and up). The rows the cases can reach are copied
into the scenario's "data" once, for the Rust test. Needs the converted stock SQLite
(tools/mdb-fetch.sh). Deterministic (splitmix64); every pasted name is invented or taken from the
stock item table, never from a game log."""
import glob
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "item_match.json")
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

    def sample(self, xs, n):
        xs = list(xs)
        out = []
        for _ in range(min(n, len(xs))):
            out.append(xs.pop(self.int(0, len(xs) - 1)))
        return out


# ------------------------------------------------------------------------------ stock rows
ITEMS = {r["Number"]: dict(r) for r in con.execute("select * from Items order by rowid")}
SHOPS = {r["Number"]: dict(r) for r in con.execute("select * from Shops order by rowid")}
CLASSES = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by rowid")}


def of(r):
    return r.get("Obtained From") or ""


def ab(r, n=20):
    return [[r["Abil-%d" % i], r["AbilVal-%d" % i]] for i in range(n) if r.get("Abil-%d" % i)]


# ------------------------------------------------------------------------------ invented rows
def item(n, name, **kw):
    r = {"Number": n, "Name": name, "Encum": 5, "ItemType": 0, "Worn": 0, "Price": 100, "Currency": 0,
         "Gettable": 0, "In Game": 1, "Obtained From": "", "WeaponType": 0, "ArmourType": 0}
    r.update(kw)
    return r


MK = {}
for n, s in SHOPS.items():
    MK.setdefault(s["Markup%"], []).append(n)
TIE = sorted(next(v for k, v in sorted(MK.items()) if k > 0 and len(v) >= 3 and k < 400))[:3]

EXTRA_ITEMS = [
    item(9001, "tst plain sword", ItemType=1, WeaponType=2, Encum=40, **{"Obtained From": "Shop #5"}),
    item(9002, "tst spaced shops", Gettable=1, **{"Obtained From": "  Shop   #  12  ,   shop #3 (SELL) , SHOP#7 , shop"}),
    item(9003, "tst big shop number", Gettable=1, **{"Obtained From": "Shop #99999999999"}),
    item(9004, "tst zero shops", Gettable=1, **{"Obtained From": "Shop #0, Shop #00, Shop, Shop #, Shop #(sell)"}),
    item(9005, "tst missing shop", Gettable=1, **{"Obtained From": "Shop #5000"}),
    item(9006, "tst huge shop number", Gettable=1, **{"Obtained From": "Shop #40000, Shop #5"}),
    item(9007, "tst bad item type", ItemType=40000, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9008, "tst bad worn", ItemType=0, Worn=40000, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9009, "tst bad weapon type", ItemType=1, WeaponType=9, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9010, "tst odd item type", ItemType=99, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9011, "tst odd worn", ItemType=0, Worn=30, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9012, "tst npc drop", **{"Obtained From": "NPC #12(5%)", "Price": 50}),
    item(9013, "tst textblock prize", **{"Obtained From": "Textblock #5(2%)", "Price": 50}),
    item(9014, "tst room only", **{"Obtained From": "Room 1/334"}),
    item(9015, "tst sell only", Currency=2, Price=4, Gettable=1, **{"Obtained From": "Shop(sell) #9, Shop(sell) #5"}),
    item(9016, "tst nogen", Gettable=1, **{"Obtained From": "Shop(nogen) #123, Shop #5"}),
    item(9017, "tst dupe", Gettable=1, Price=100, **{"Obtained From": "Shop #5"}),
    item(9018, "TST DUPE", Gettable=1, Price=20, **{"Obtained From": "Shop #3"}),
    item(9019, "tst dupe ", Gettable=1, Price=1, **{"In Game": 0, "Obtained From": "Shop #3"}),
    item(9020, "tst free thing", Gettable=1, Price=0, **{"Obtained From": "Shop #5"}),
    item(9021, "tst brass key", ItemType=7, Gettable=1, Price=5, **{"Obtained From": "Shop #6"}),
    item(9022, "tst runic thing", Currency=4, Price=50, Gettable=1, **{"Obtained From": "Shop #5"}),
    item(9023, "tst no source", Gettable=1),
    item(9024, "tst tie buy", Gettable=1, Price=100,
         **{"Obtained From": "Shop #%d, Shop #%d, Shop #%d" % (TIE[2], TIE[0], TIE[1])}),
    item(9025, "tst tie sell", Gettable=1, Price=100,
         **{"Obtained From": "Shop(sell) #%d, Shop(sell) #%d" % (TIE[1], TIE[0])}),
    item(9026, "tst room shops", Gettable=1, Price=300, **{"Obtained From": "Shop #9001, Shop #9002, Shop #9003"}),
    item(9027, "tst room shops two", Gettable=1, Price=300, **{"Obtained From": "Shop #9004, Shop #9005"}),
    item(9028, "tst unnamed room shop", Gettable=1, Price=300, **{"Obtained From": "Shop #9006"}),
    item(9029, "tst level gate", Gettable=1, Price=70, Worn=2, ItemType=0, ArmourType=1,
         **{"Obtained From": "Shop #5", "Abil-0": 135, "AbilVal-0": 20}),
    item(9030, "tst class gate", Gettable=1, Price=70, ItemType=1, WeaponType=3,
         **{"Obtained From": "Shop #5, Shop(sell) #3", "ClassRest-0": 1, "ClassRest-1": 7}),
    item(9031, "tst evil only", Gettable=1, Price=70, Worn=8, **{"Obtained From": "Shop #5", "Abil-0": 98, "AbilVal-0": 0}),
    item(9032, "tst magical thing", Gettable=1, Price=70, Worn=8, **{"Obtained From": "Shop #5", "Abil-0": 28, "AbilVal-0": 0}),
    item(9033, "tst cheap and sell", Gettable=1, Price=2, **{"Obtained From": "Shop #5, Shop(sell) #3"}),
    item(9034, "tst many shops", Gettable=1, Price=900, Currency=1,
         **{"Obtained From": "Shop #3, Shop #5, Shop #6, Shop #9, Shop #10, Shop #12, Shop #44, Shop(sell) #4"}),
]
EXTRA_SHOPS = [
    {"Number": 9001, "Name": "tst shop rooms", "ShopType": 0, "Markup%": 25, "In Game": 1,
     "Assigned To": "Room 1/334, Room 1/99999, Room 0/5, Room x/5, Room 6/1334"},
    {"Number": 9002, "Name": "tst shop npc", "ShopType": 0, "Markup%": 0, "In Game": 1, "Assigned To": "NPC #12"},
    {"Number": 9003, "Name": "tst shop blank", "ShopType": 0, "Markup%": 150, "In Game": 1, "Assigned To": ""},
    {"Number": 9004, "Name": "tst shop one room", "ShopType": 0, "Markup%": 5, "In Game": 1, "Assigned To": "Room 1/2190"},
    {"Number": 9005, "Name": "  tst spaced room  ", "ShopType": 0, "Markup%": 40, "In Game": 1,
     "Assigned To": "  room 1/334  ,ROOM 1/2190, Room 15/850"},
    {"Number": 9006, "Name": "tst unnamed room", "ShopType": 0, "Markup%": 10, "In Game": 1, "Assigned To": "Room 15/850"},
]
for r in EXTRA_ITEMS:
    ITEMS[r["Number"]] = dict(r)
for r in EXTRA_SHOPS:
    SHOPS[r["Number"]] = dict(r)
STOCK_ITEM_NUMBERS = sorted(n for n in ITEMS if n < 9000)


def norm(s):
    return s.strip(" ").lower()


NAME_ROWS = {}
for n, r in ITEMS.items():
    NAME_ROWS.setdefault(norm(r["Name"]), []).append(n)

# ------------------------------------------------------------------------------ what the cases touch
used_names = set()
used_numbers = set()
used_classes = set()


def reg(name):
    """Registers an item name a case may look up (its rows go into the Rust data)."""
    k = norm(name)
    if k in NAME_ROWS:
        used_names.add(k)


def regn(n):
    used_numbers.add(n)


def shop_tokens(r):
    return [t.strip().lower() for t in of(r).split(",") if t.strip().lower().startswith("shop")]


def pick_by(pred, stride, limit, extra_ok=True):
    pool = [n for n in sorted(ITEMS) if pred(ITEMS[n]) and (extra_ok or n < 9000)]
    return pool[::stride][:limit]


CAT = {
    "multi": pick_by(lambda r: len([t for t in shop_tokens(r) if "(sell)" not in t]) >= 2, 9, 14, False),
    "one": pick_by(lambda r: len(shop_tokens(r)) == 1 and r["Gettable"] == 1 and r["Price"] > 0, 17, 16, False),
    "sell": pick_by(lambda r: any("(sell)" in t for t in shop_tokens(r)), 7, 12, False),
    "nogen": pick_by(lambda r: any("nogen" in t for t in shop_tokens(r)), 9, 8, False),
    "npc": pick_by(lambda r: "npc #" in of(r).lower() and not shop_tokens(r), 25, 8, False),
    "room": pick_by(lambda r: r["Gettable"] == 0 and "npc #" not in of(r).lower() and "textblock #" not in of(r).lower(), 40, 8, False),
    "free": pick_by(lambda r: r["Price"] == 0 and shop_tokens(r), 5, 5, False),
    "key": pick_by(lambda r: r["ItemType"] == 7 and shop_tokens(r), 1, 3, False),
    "armour": pick_by(lambda r: r["ItemType"] == 0 and shop_tokens(r) and r["Worn"] > 0, 31, 10, False),
    "weapon": pick_by(lambda r: r["ItemType"] == 1 and shop_tokens(r), 19, 10, False),
    "notingame": pick_by(lambda r: r["In Game"] == 0, 6, 8, False),
    "pricey": pick_by(lambda r: r["Price"] >= 5000 and shop_tokens(r), 3, 5, False),
    "extra": sorted(n for n in ITEMS if n >= 9000),
}
DUPE_NAMES = [k for k, v in NAME_ROWS.items() if len(v) > 1 and any(n < 9000 for n in v)][:40]
CAT["dupe"] = [NAME_ROWS[k][0] for k in DUPE_NAMES]
ALL_POOL = []
for k, v in CAT.items():
    ALL_POOL += v
ALL_POOL = sorted(set(ALL_POOL))
for n in ALL_POOL:
    reg(ITEMS[n]["Name"])
    regn(n)

# ------------------------------------------------------------------------------ cases
cases = []
_n = [0]


def add(prefix, **kw):
    kw["id"] = "%s%03d" % (prefix, _n[0])
    _n[0] += 1
    cases.append(kw)
    return kw


rng = Rng(0x3939A5)
CHARMS = ["0", "50", "100", "", "abc", "70.5", "-5", "255", "32767"]
SLOTS = ["Head", "Weapon Hand", "Off-Hand", "Torso", "Legs", "Feet", "Back", "Neck", "Ears", "Wrist", "Finger", "Waist", "Hands"]


def combo(n):
    return {"$array": {"0": {"ListIndex": 1, "ItemData": {"1": n}} if n else {"ListIndex": 0}}}


CHARS = [
    {"cls": 0, "lvl": 999, "al": 0, "min": "0"}, {"cls": 1, "lvl": 20, "al": 1, "min": "0"},
    {"cls": 12, "lvl": 12, "al": 3, "min": "0"}, {"cls": 7, "lvl": 40, "al": 2, "min": "8"},
    {"cls": 3, "lvl": 5, "al": 1, "min": "0"}, {"cls": 15, "lvl": 60, "al": 0, "min": "30"},
    {"cls": 9001, "lvl": 1, "al": 0, "min": "0"},
]


def char_forms(ch, charm):
    f = {"cmbGlobalClass": combo(ch["cls"]), "txtGlobalLevel": {"$array": {"0": {"Text": str(ch["lvl"])}}},
         "cmbGlobalAlignment": {"ListIndex": ch["al"]}, "txtGlobalMinLVL": {"Text": ch["min"]},
         "txtCharStats": {"$array": {"5": {"Tag": charm}}}}
    if ch["cls"] and ch["cls"] < 9000:
        used_classes.add(ch["cls"])
    return f


def eq_globals(ch, rng_):
    eq = [0] * 20
    for _ in range(rng_.int(0, 3)):
        eq[rng_.int(0, 19)] = rng_.pick(ALL_POOL)
    return eq


def itemmatch_udt(n):
    r = ITEMS[n]
    obt = of(r)
    g = 1 if (r["Gettable"] == 1 or re.search(r"npc #", obt, re.I) or
              (re.search(r"textblock #", obt, re.I) and not re.search(r"room ", obt, re.I))) else 0
    return {"udt": "ItemMatch", "fields": {"Number": n, "name": r["Name"], "ItemType": r["ItemType"], "Worn": r["Worn"],
                                           "WeaponType": r["WeaponType"], "encum": r["Encum"], "ObtainedFrom": obt,
                                           "Gettable": g}}


# --- A. ExtractFirstNumber
EFN = ["", "shop #12", "shop #012", "12", "a1b2", "shop # 3 (sell)", "x", "   ", "9", "0", "00", "shop #99999999999",
       "shop #2147483647", "shop #2147483648", "shop 12 13", "(sell) 5", "12abc34", "a-5", "1,5", "shop#7", "007",
       "shop #" + "0" * 30 + "5", "shop #" + "9" * 400, "x1.5y", "abc", "#", "-1"]
for t in EFN:
    add("a", proc="ExtractFirstNumber", args=[t])

# --- B. ExtractShopsFromObtainedFrom (real texts and invented ones)
OBT = [
    "", "   ", "Shop #5", "shop #5", "Shop #5, Shop #3", "Shop(sell) #10, Shop #122", "Shop(nogen) #167, Textblock #889(2%)",
    "Shop #5,Shop #5", "Shop #5, Shop(sell) #5", "SHOP  #  12 ( SELL )", "shop #12 (sell)", "Shop # 12 (SELL), x", "Shop (sell) #3",
    "Room 1/334", "NPC #12", "Item #5(2%), Monster #3(4%)", "Shop", "Shop #", "Shop #0", "Shop #0, Shop #7", "shopping #4", "shops #5",
    "Shop #99999999999", "Shop #5, Shop #99999999999", "Shop #2147483647", "Shop#7", "Shop7", "Shop(sell)#3(sell)", " , , Shop #2 , ",
    "Shop #3,,Shop #4", "xShop #5", "Shop #5 Shop #6", "Shop #5; Shop #6", "shop #1.5", "shop #1e3", "shop #-3", "shop #\t4",
]
real = [of(ITEMS[n]) for n in pick_by(lambda r: shop_tokens(r), 11, 40, False)]
for t in OBT + real:
    add("b", steps=[{"call": "ExtractShopsFromObtainedFrom", "as": "x", "args": [t, {"ref": "array:shoptoken"}]}])

# --- C. ParseActionAndQty
FLAGS = ["", " ", "CARRIED", "carried", "carried x3", "CARRIED X3", "Carried   x3", "STASH x2", "stash x0", "stash x-4", "x5", "x",
         "a x", "a x5x", "a x 5", "a x5.5", "a x2.5", "a x1e2", "a x&H10", "a x99999999999", "a x2147483647", "a x2147483648",
         "a x 0", "STASHx2", "stashX4", "stash2", "stashx", "xx5", "ax5b", "ax", "a5", "5", "x5 x6", "keep x2 x3", "  keep  x2  ",
         "keep x2 ", "keep x 2", "keep x+2", "keep x-2", "keep x2,5", "keep x1,000", "pick x12abc", "equipped", "EQUIPPED x1",
         "mule1x3", "mule x3x", "x3 mule", "a b c x9"]
for t in FLAGS:
    for q in rng.sample([0, 1, 2, 5, -3, 7, 99999], 2):
        add("c", steps=[{"call": "ParseActionAndQty", "as": "x", "args": [t, {"ref": "string", "value": ""},
                                                                         {"ref": "long", "value": 0}, q]}])

# --- D. GetItemsByExactNameArr
name_probes = []
for n in rng.sample(ALL_POOL, 40):
    nm = ITEMS[n]["Name"]
    name_probes.append(nm)
    if rng.chance(40):
        name_probes.append(nm.upper())
    if rng.chance(30):
        name_probes.append("  " + nm + "   ")
for k in DUPE_NAMES[:10]:
    name_probes.append(k)
name_probes += ["", "   ", "no such item at all", "tst dupe", "TST dupe", "tst  dupe", "iron", "tst dupe  "]
for nm in name_probes[:75]:
    reg(nm)
    add("d", globals={"bOnlyInGame": rng.chance(50)},
        steps=[{"call": "GetItemsByExactNameArr", "as": "x", "args": [nm, {"ref": "array:itemmatch"}]}])

# --- E. EvaluateBestPriceForHit
for n in rng.sample(ALL_POOL, 70):
    charm = rng.pick([0, 0, 50, 100, 1, 99])
    add("e", globals={"bHideRecordNumbers": rng.chance(35)},
        steps=[{"call": "EvaluateBestPriceForHit", "as": "x", "args": [
            itemmatch_udt(n), charm, {"ref": "string", "value": ""}, {"ref": "string", "value": ""},
            {"ref": "long", "value": 0}, {"ref": "boolean", "value": False}, {"ref": "long", "value": 0}]}])

# --- F. GetBestShopNumForItem
for n in rng.sample(ALL_POOL, 50) + [0, 99999, -3, 9003]:
    regn(n)
    charm = rng.pick([0, 0, 50, 100, 20])
    add("f", globals={"bHideRecordNumbers": rng.chance(35)},
        steps=[{"call": "GetBestShopNumForItem", "as": "x", "args": [
            n, charm, {"ref": "boolean", "value": False}, {"ref": "double", "value": 0.0},
            {"ref": "string", "value": ""}, {"ref": "string", "value": ""}]}])

# --- G. SumInvOrCarriedQtyForName
LV_ROWS = [
    {"Text": "1", "Sub": ["Rope", "", "", "2", "Inventory"]}, {"Text": "2", "Sub": ["rope", "CARRIED x3", "", "1", "Inventory"]},
    {"Text": "3", "Sub": ["ROPE", "carried", "", "0", "Ground"]}, {"Text": "4", "Sub": ["rope", "CARRIED x2", "", "", "Equipped"]},
    {"Text": "5", "Sub": ["rope ", "CARRIED", "5"]}, {"Text": "6", "Sub": ["rope", "STASH x4", "9", "9", "Ground"]},
    {"Text": "7", "Sub": ["rope", "carried X7", "", "", "Ground"]}, {"Text": "8", "Sub": ["rope"]},
    {"Text": "9", "Sub": ["rope", "carried x", "", "x", "Ground"]}, {"Text": "10", "Sub": ["rope", "CARRIED xa", "", "-3", "Ground"]},
    {"Text": "11", "Sub": ["rope", "", "", "1.6", "inventory stash"]}, {"Text": "12", "Sub": ["rope", "CARRIED x99999999999", "", "", ""]},
    {"Text": "13", "Sub": ["rope", "", "", "2147483647", "Inventory"]}, {"Text": "14", "Sub": ["rope", "", "", "2147483647", "Inventory"]},
    {"Text": "15", "Sub": ["lantern", "carried x2", "", "", "Ground"]}, {"Text": "16", "Sub": ["lantern", "", "", "4", "Inventory"]},
]
for k in range(30):
    sel = rng.sample(LV_ROWS[:12] + LV_ROWS[14:], rng.int(0, 8))
    if k in (27, 28):
        sel = LV_ROWS[12:14]
    if k == 29:
        sel = [LV_ROWS[11]]
    nm = rng.pick(["rope", "ROPE", "Rope", "lantern", "none", "rope "])
    add("g", forms={"frmMain": {"lvItemManager": {"$listview": sel}}},
        steps=[{"call": "SumInvOrCarriedQtyForName", "as": "x", "args": [{"control": "lvItemManager"}, nm]}])

# --- H. PopulateItemManagerFromParsed (AddSectionItems, AddListViewRowsForItem, AddOneRow under it)
JUNK = ["wooden spoon of doom", "xyzzy", "mystery meat", "bag of holding", "a thing"]
for j in JUNK:
    assert norm(j) not in NAME_ROWS


def name_variant(n):
    nm = ITEMS[n]["Name"]
    r = rng.int(1, 10)
    if r <= 6:
        return nm.lower()
    if r <= 8:
        return nm.upper()
    return "  " + nm.lower() + "  "


def entry(base, kind):
    reg(base)
    q = rng.int(1, 10)
    if kind == "equipped":
        return "%s (%s)" % (base, rng.pick(SLOTS))
    if q <= 5:
        return base
    return "%s (%d)" % (base, rng.int(1, 6))


def parsed_udt(r):
    pools = {"equipped": CAT["armour"] + CAT["weapon"] + CAT["extra"][:6],
             "inventory": ALL_POOL, "keys": CAT["key"] + [9021, 9021], "ground": ALL_POOL}
    f = {}
    for k, key in (("equipped", "sEquipped"), ("inventory", "sInventory"), ("keys", "sKeys"), ("ground", "sGround")):
        if r.chance(12):
            f[key] = [] if r.chance(50) else [""]
            continue
        lst = []
        for _ in range(r.int(1, 6)):
            if r.chance(10):
                lst.append(entry(r.pick(JUNK), k))
            else:
                lst.append(entry(name_variant(r.pick(pools[k])), k))
        f[key] = lst
    return {"udt": "ItemParseResult", "fields": f}


def existing_rows(r):
    out = []
    for i in range(r.int(0, 5)):
        n = r.pick(ALL_POOL)
        regn(n)
        nm = ITEMS[n]["Name"]
        src = r.pick(["Inventory", "Inventory", "Ground", "Equipped"])
        flag = r.pick(["", "", "CARRIED", "CARRIED x2", "STASH"])
        out.append({"Text": str(n), "Sub": [nm, flag, "", str(r.int(0, 4)), src]})
    return out


for i in range(120):
    r = rng
    ch = r.pick(CHARS)
    charm = r.pick(["0", "0", "50", "100", "80"]) if i % 15 else r.pick(["abc", "99999", "-5", "70.5"])
    forms = char_forms(ch, charm)
    forms["lvItemManager"] = {"$listview": existing_rows(r)}
    answers = r.pick([[], [], [7], [6, 7], [6, 6, 6], [7, 6, 6], [6, 7, 6], [7, 7, 7]])
    add("h", forms={"frmMain": forms}, globals={"nEquippedItem": eq_globals(ch, r), "bOnlyInGame": r.chance(30),
                                                 "bHideRecordNumbers": r.chance(35)},
        msgbox=answers, read={"rows": "%lv.lvItemManager"},
        steps=[{"call": "PopulateItemManagerFromParsed", "args": [parsed_udt(r), {"control": "lvItemManager"}, r.chance(20)]}])

# --- I. LV_AddRowByItemNumber
FLAG_ARGS = ["", "", "CARRIED", "carried x3", "STASH x2", "x5", "keep", "bad x99999999999", "stash x1"]
SRC_ARGS = ["Inventory", "Inventory", "Ground", "Equipped", "", "Manual"]
for i in range(80):
    n = rng.pick(ALL_POOL + [0, 99999, -2, 9014, 9021, 9007, 9008])
    regn(n)
    ch = rng.pick(CHARS)
    charm = rng.pick(["0", "50", "100", "abc", "99999"]) if i % 6 else "0"
    forms = char_forms(ch, charm)
    forms["lvItemManager"] = {"$listview": existing_rows(rng) if rng.chance(30) else []}
    force = rng.pick([0, 0, 0, 3, 5, 9001, 9004, 77777, -4])
    add("i", forms={"frmMain": forms}, globals={"nEquippedItem": eq_globals(ch, rng), "bHideRecordNumbers": rng.chance(35)},
        read={"rows": "%lv.lvItemManager"},
        steps=[{"call": "LV_AddRowByItemNumber", "args": [n, rng.pick(SRC_ARGS), rng.pick(FLAG_ARGS),
                                                           rng.pick([0, 1, 2, 7, 9999, 10000, 50000, -3]), force]}])

# --- K. GetShopRoomNames
shop_nums = sorted(SHOPS)
for n in rng.sample([s for s in shop_nums if s < 9000], 36) + [9001, 9002, 9003, 9004, 9005, 9006, 0, 99999, -5, 8, 23, 38]:
    for lim, nn in rng.sample([(0, False), (0, True), (1, False), (2, True), (3, False)], 2):
        add("k", steps=[{"call": "GetShopRoomNames", "as": "x", "args": [n, lim, nn]}])

# ------------------------------------------------------------------------------ the Rust data
rows = {n for n in used_numbers if n in ITEMS}
for k in used_names:
    rows.update(NAME_ROWS[k])


def item_row(n):
    r = ITEMS[n]
    return {"n": n, "name": r["Name"], "t": r["ItemType"], "w": r["Worn"], "wt": r["WeaponType"], "at": r["ArmourType"],
            "enc": r["Encum"], "price": r["Price"], "cur": r["Currency"], "g": r["Gettable"], "ig": r["In Game"],
            "of": r.get("Obtained From") or None, "cr": [r.get("ClassRest-%d" % k) or 0 for k in range(10)], "ab": ab(r)}


shops_used, rooms_used = set(), set()
for n in rows:
    for t in shop_tokens(ITEMS[n]):
        m = re.search(r"(\d+)", t)
        if m and int(m.group(1)) in SHOPS:
            shops_used.add(int(m.group(1)))
for n in shop_nums:
    if n >= 9000:
        shops_used.add(n)
for c in cases:
    if c["id"][0] in "k":
        shops_used.add(c["steps"][0]["args"][0])
    if c["id"][0] == "i":
        shops_used.add(c["steps"][0]["args"][4])
shops_used = {s for s in shops_used if s in SHOPS}
for s in shops_used:
    for part in (SHOPS[s].get("Assigned To") or "").split(","):
        m = re.search(r"(\d+)/(\d+)", part)
        if m and "room" in part.lower():
            rooms_used.add((int(m.group(1)), int(m.group(2))))
ROOMS = {(r["Map Number"], r["Room Number"]): r["Name"]
         for r in con.execute("select \"Map Number\", \"Room Number\", Name from Rooms")}
data = {
    "items": [item_row(n) for n in sorted(rows, key=lambda n: (n >= 9000, list(ITEMS).index(n)))],
    "shops": [{"n": s, "name": SHOPS[s]["Name"], "markup": SHOPS[s]["Markup%"], "asg": SHOPS[s].get("Assigned To") or None}
              for s in sorted(shops_used)],
    "rooms": [{"m": m, "r": r, "name": ROOMS.get((m, r))} for (m, r) in sorted(rooms_used) if (m, r) in ROOMS],
    "classes": [{"n": n, "wt": CLASSES[n]["WeaponType"], "at": CLASSES[n]["ArmourType"], "ab": ab(CLASSES[n], 10)}
                for n in sorted(used_classes)],
}
# items must keep MME's table order: stock rows by rowid, then the invented ones
order = {n: i for i, n in enumerate(ITEMS)}
data["items"].sort(key=lambda r: order[r["n"]])

scn = {
    "family": "item_match",
    "description": "FAM-22 second half (modItemParse): parsed names matched to Items rows, best shop and value, Item Manager "
                   "rows (AddSectionItems, PopulateItemManagerFromParsed, LV_AddRowByItemNumber), ExtractShopsFromObtainedFrom, "
                   "EvaluateBestPriceForHit, GetBestShopNumForItem, SumInvOrCarriedQtyForName; ParseActionAndQty; GetShopRoomNames.",
    "notes": [
        "Needs the stock database plus invented rows (db_extra: Items 9001+, Shops 9001+); data holds the rows the cases can "
        "reach (every Items row whose name is looked up, the shops they list, those shops' rooms, the classes used) for the Rust "
        "test, in table order.",
        "The Item Manager is frmMain.lvItemManager, a mutable ListView shim; a case reads it back with %lv.lvItemManager "
        "(rows: text and sub items with the Value and Worn tags). LV_AssignRowSeqIfMissing (hidden row-sequence column) is a no-op.",
        "ItemIsUsableByChar is cut from frmMain.frm and reached as frmMain.ItemIsUsableByChar from AddOneRow; the character is "
        "the cmbGlobalClass, txtGlobalLevel, cmbGlobalAlignment and txtGlobalMinLVL controls plus nEquippedItem.",
        "A case's msgbox lists the MsgBox answers in the order MME asks (6 Yes, 7 No); without it the default button answers "
        "(Yes, Yes, and No for the clear question). Boxes and handled errors are in events.",
        "The charm is frmMain.txtCharStats(5).Tag. Generated by tools/oracle/gen/item_match.py.",
    ],
    "database": True,
    "modules": ["modItemParse.bas"],
    "frm": [
        {"file": "frmMain.frm", "procs": ["ItemIsUsableByChar"]},
        {"file": "modListViewExt.bas", "procs": ["ParseActionAndQty"]},
    ],
    "db_extra": {"Items": EXTRA_ITEMS, "Shops": EXTRA_SHOPS},
    "data": data,
    "forms": {"frmMain": {"lvItemManager": {"$listview": []}, "cmbGlobalAlignment": {"ListIndex": 0},
                          "txtCharStats": {"$array": {"5": {"Tag": "0"}}}}},
    "tolerance": {"default": 0},
    "cases": cases,
}
with open(OUT, "w", newline="\n") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("wrote", OUT, len(cases), "cases,", len(data["items"]), "items,", len(data["shops"]), "shops", file=sys.stderr)

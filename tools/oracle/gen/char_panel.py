"""Writes fixtures/scenarios/char_panel.json: python3 -I tools/oracle/gen/char_panel.py fixtures/scenarios/char_panel.json
Char panel derivations (card P3-19): frmMain RefreshCPs, RefreshHitPoints, RefreshMagic, RefreshPicklocks,
RefreshMagicRes, RefreshDodge, RefreshMainStatBonuses, SetupClass, lblInvenCharStat_Change,
CharStatAdjustmentPrompt and the mana-per-round total of RefreshCharBless. Same conventions as gear2.py:
deterministic (splitmix64), real rows from the stock SQLite plus invented rows numbered 9001+; the rows the
cases use are written once into the scenario's "data" so the Rust test can rebuild them."""
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


rng = Rng(0x19C4A)

# ---------------------------------------------------------------------------------- rows
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}


def ab(row, n):
    return [x for x in ([row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n)) if x[0] != 0]


def abil(*pairs):
    d = {}
    for i, (a, v) in enumerate(pairs):
        d["Abil-%d" % i] = a
        d["AbilVal-%d" % i] = v
    return d


# races: BaseCP and ExpTable are all 100 / small in stock; invented rows reach other bases and an Integer overflow
EXTRA_RACES = [
    {"Number": 9001, "Name": "tst cheap race", "BaseCP": 50, "ExpTable": 20, "HPPerLVL": 2},
    {"Number": 9002, "Name": "tst zero race", "BaseCP": 0, "ExpTable": 0, "HPPerLVL": -3},
    {"Number": 9003, "Name": "tst rich race", "BaseCP": 250, "ExpTable": -30, "HPPerLVL": 5},
    {"Number": 9004, "Name": "tst big exp race", "BaseCP": 120, "ExpTable": 32700, "HPPerLVL": 0},
]
# classes: magery kinds and levels (clamped at 3), anti-magic 51, picklocks 37, HP rolls, an exp table that overflows
EXTRA_CLASSES = [
    {"Number": 9001, "Name": "tst mage-ish", "MinHits": 2, "MaxHits": 9, "ExpTable": 30, "MageryType": 3, "MageryLVL": 5,
     **abil((51, 0), (37, 0))},
    {"Number": 9002, "Name": "tst picker", "MinHits": 5, "MaxHits": 0, "ExpTable": -100, "MageryType": 0, "MageryLVL": 2, **abil((37, 0))},
    {"Number": 9003, "Name": "tst kai", "MinHits": 0, "MaxHits": 0, "ExpTable": 10, "MageryType": 5, "MageryLVL": 0, **abil((29, 1))},
    {"Number": 9004, "Name": "tst bardish", "MinHits": 7, "MaxHits": 1, "ExpTable": 200, "MageryType": 4, "MageryLVL": 3,
     **abil((1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6), (7, 7), (8, 8), (9, 9), (37, 0))},
    {"Number": 9005, "Name": "tst odd magery", "MinHits": 3, "MaxHits": 3, "ExpTable": 0, "MageryType": 7, "MageryLVL": 1,
     **abil((0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (51, 0))},
    {"Number": 9006, "Name": "tst huge exp", "MinHits": 4, "MaxHits": 4, "ExpTable": 32700, "MageryType": 1, "MageryLVL": 2},
    {"Number": 9007, "Name": "tst huge hp", "MinHits": 32000, "MaxHits": 32000, "ExpTable": 0, "MageryType": 2, "MageryLVL": 3},
    {"Number": 9008, "Name": "tst big hp", "MinHits": 30000, "MaxHits": 2000, "ExpTable": 0, "MageryType": 1, "MageryLVL": 3},
]
# spells for the mana-per-round total: durations 0 (divides by zero), 1, negative, level-scaled; costs 0 and large
EXTRA_SPELLS = [
    {"Number": 9001, "Name": "tst zero dur", "Short": "tzd", "ReqLevel": 1, "Cap": 0, "ManaCost": 6, "MinBase": 2, "MaxBase": 4,
     "Dur": 0, **abil((4, 3))},
    {"Number": 9002, "Name": "tst short dur", "Short": "tsd", "ReqLevel": 5, "Cap": 20, "ManaCost": 7, "MinBase": 1, "MaxBase": 3,
     "Dur": 1, "DurInc": 1, "DurIncLVLs": 3, **abil((10, 8), (22, 5))},
    {"Number": 9003, "Name": "tst free", "Short": "tfr", "ReqLevel": 0, "Cap": 0, "ManaCost": 0, "MinBase": 4, "MaxBase": 8,
     "Dur": 90, **abil((46, 0), (7, 40))},
    {"Number": 9004, "Name": "tst negative dur", "Short": "tnd", "ReqLevel": 0, "Cap": 0, "ManaCost": 9, "MinBase": 1, "MaxBase": 1,
     "Dur": -4, **abil((36, 3))},
    {"Number": 9005, "Name": "tst dear", "Short": "tdr", "ReqLevel": 12, "Cap": 40, "ManaCost": 32000, "MinBase": 3, "MaxBase": 9,
     "Dur": 7, "DurInc": 2, "DurIncLVLs": 4, "MinInc": 1, "MinIncLVLs": 2, "MaxInc": 2, "MaxIncLVLs": 3, **abil((22, 0), (4, 0), (9, 0))},
    {"Number": 9006, "Name": "tst odd cost", "Short": "toc", "ReqLevel": 0, "Cap": 0, "ManaCost": 5, "MinBase": 0, "MaxBase": 0,
     "Dur": 11, **abil((34, 2))},
    {"Number": 9007, "Name": "tst nothing", "Short": "tno", "ReqLevel": 0, "Cap": 0, "ManaCost": 3, "MinBase": 0, "MaxBase": 0,
     "Dur": 33},
]
for r in EXTRA_RACES:
    races[r["Number"]] = {**{k: 0 for k in races[1]}, **r}
for r in EXTRA_CLASSES:
    classes[r["Number"]] = {**{k: 0 for k in classes[1]}, **r}
for r in EXTRA_SPELLS:
    spells[r["Number"]] = {**{k: 0 for k in spells[14] if k not in ("Name", "Short")}, **{"Name": "", "Short": ""}, **r}

STOCK_CLASSES = [n for n in classes if n < 9000]
INV_CLASSES = [n for n in classes if n >= 9000]
STOCK_RACES = [n for n in races if n < 9000]
INV_RACES = [n for n in races if n >= 9000]
STAT_SLOTS = {2, 10, 3, 4, 5, 7, 13, 14, 22, 24, 25, 27, 29, 30, 34, 35, 36, 37, 40, 44, 45, 46, 47, 48, 49, 58, 65, 66, 67, 69, 70,
              77, 88, 89, 90, 91, 92, 93, 94, 96, 116, 117, 118, 123, 142, 145, 147, 165, 9}
BLESS_POOL = [n for n, r in spells.items() if n < 9000 and any(a[0] in STAT_SLOTS for a in ab(r, 10))]
BLESS_KEY = [34, 52, 4, 1185, 384, 129, 14, 15, 23, 42, 7, 38, 127, 105, 141, 37, 144, 45, 97, 46, 96, 47, 98, 48, 99, 276, 1388,
             1389, 1055, 20, 21, 107, 108, 289, 64, 6, 9, 138, 269, 746, 1314, 869, 39, 302, 858, 879, 118, 307, 100, 26, 130,
             91, 134, 337, 586, 308, 312, 186, 792, 897, 5, 12]
BLESS_KEY = [n for n in BLESS_KEY if n in spells]
BLESS_ALL = BLESS_KEY + [9001, 9002, 9003, 9004, 9005, 9006, 9007]

used_classes, used_races, used_spells = set(), set(), set()


# ---------------------------------------------------------------------------------- form builders
def combo(n, kind):
    """A class or race combo: ListIndex 1 with ItemData(1) = n, or the first entry (ListIndex 0, number 0)."""
    if n is None:
        return None
    if n:
        (used_classes if kind == "c" else used_races).add(n)
        return {"$array": {"0": {"ListIndex": 1, "ItemData": {"1": n}}}}
    return {"$array": {"0": {"ListIndex": 0}}}


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def case(cid, call, args=None, forms=None, globs=None, read=None):
    c = {"id": cid, "steps": [{"call": call, **({"args": args} if args is not None else {})}], "forms": {"frmMain": forms or {}}}
    if globs:
        c["globals"] = globs
    if read:
        c["read"] = read
    return c


def level_form(level):
    return {"txtGlobalLevel": arr({0: {"Text": level}})}


def num(x):
    return str(x)


cases = []

# ---------------------------------------------------------------------------------- RefreshCPs
def cps_case(cid, texts, maxes, race, cls, level):
    f = {"txtCharStats": arr({i: {"Text": t} for i, t in enumerate(texts)}),
         "txtCharMaxStats": arr({i: {"Tag": t} for i, t in enumerate(maxes)}), **level_form(level)}
    if race is not None:
        f["cmbGlobalRace"] = combo(race, "r")
    if cls is not None:
        f["cmbGlobalClass"] = combo(cls, "c")
    return case(cid, "RefreshCPs", forms=f)


# one stat at a time over the cost bands (10 points per level of cost, capped at 10 per point)
for d in (-5, 0, 1, 5, 9, 10, 11, 19, 20, 21, 45, 50, 79, 80, 81, 89, 90, 91, 95, 99, 100, 101, 105, 110, 111, 150, 250, 1000):
    st = [60, 55, 50, 65, 70, 45]
    st[d % 6] = 50 + d
    cases.append(cps_case("cps-delta-%d" % d, [num(x) for x in st], ["50"] * 6, 1, 1, "20"))
# all six stats raised: level required grows by steps; level text 0/1 uses the loop result
for lvl in ("0", "1", "2", "9", "10", "11", "19", "20", "21", "50", "99", "100", "255", "500", "501", "", "abc", "12.5", "7.5"):
    cases.append(cps_case("cps-level-%s" % (lvl or "empty"), ["75", "80", "70", "90", "85", "72"], ["50", "55", "50", "60", "55", "50"],
                          3, 4, lvl))
for add in (0, 3, 17, 40, 90, 150, 300, 500):
    cases.append(cps_case("cps-all-%d" % add, [num(50 + add)] * 6, ["50"] * 6, 2, 5, "0"))
# exp requirement: every stock class and race, then invented rows
for c in STOCK_CLASSES + INV_CLASSES + [0]:
    cases.append(cps_case("cps-class-%d" % c, ["70", "72", "74", "76", "78", "80"], ["50"] * 6, 1 + c % 13, c, rng.choice(["0", "5", "30"])))
for r in STOCK_RACES + INV_RACES:
    cases.append(cps_case("cps-race-%d" % r, ["70", "72", "74", "76", "78", "80"], ["50"] * 6, r, 1 + r % 15, rng.choice(["0", "5", "30"])))
cases.append(cps_case("cps-race-none", ["70"] * 6, ["50"] * 6, None, 1, "20"))
cases.append(cps_case("cps-norace-noclass", ["70"] * 6, ["50"] * 6, 0, 0, "20"))
cases.append(cps_case("cps-race0-class1", ["70"] * 6, ["50"] * 6, 0, 1, "20"))
cases.append(cps_case("cps-class0-race1", ["70"] * 6, ["50"] * 6, 1, 0, "20"))
# text oddities: fractions, signs, junk, big values
for i, (t, m) in enumerate((("50.5", "50"), ("50.4", "50"), ("51.5", "50"), ("abc", "50"), ("", ""), ("-5", "20"), ("60", "50.5"),
                            ("1e2", "50"), ("&H64", "50"), ("99999", "50"), ("327670", "50"), ("330000", "50"), ("12345678", "50"))):
    cases.append(cps_case("cps-text-%d" % i, [t] + ["60"] * 5, [m] + ["50"] * 5, 1, 1, "20"))
for i in range(40):
    st = [rng.between(15, 140) for _ in range(6)]
    mx = [rng.between(30, 100) for _ in range(6)]
    cases.append(cps_case("cps-random-%02d" % i, [num(x) for x in st], [num(x) for x in mx],
                          rng.choice(STOCK_RACES + INV_RACES), rng.choice(STOCK_CLASSES + INV_CLASSES + [0]),
                          rng.choice(["0", "1", "5", "13", "27", "60", "140", "300"])))

# ---------------------------------------------------------------------------------- RefreshHitPoints
def hp_case(cid, level, cls, health, regen, bonus):
    f = {**level_form(level), "txtCharStats": arr({4: {"Tag": health}}), "txtCharHPRegen": {"Text": regen},
         "lblInvenCharStat": arr({5: {"Caption": bonus}}), "cmbGlobalClass": combo(cls, "c")}
    return case(cid, "RefreshHitPoints", forms=f)


for c in STOCK_CLASSES + INV_CLASSES + [0]:
    lvl = rng.choice(["1", "20"])
    cases.append(hp_case("hp-class-%d-l%s" % (c, lvl), lvl, c, "60", "0", "0"))
for lvl in ("0", "1", "2", "5", "10", "25", "50", "100", "255", "500", "", "x", "12.5", "13.5", "40000", "-3"):
    cases.append(hp_case("hp-level-%s" % (lvl or "empty"), lvl, 6, "70", "15", "12"))
for h in ("0", "1", "49", "50", "51", "65", "80", "100", "150", "255", "", "70.5", "-10", "40000", "3000000000", "1e3"):
    cases.append(hp_case("hp-health-%s" % (h or "empty"), "30", 4, h, "0", "0"))
for rg in ("0", "10", "-10", "-100", "-101", "50", "200", "12.5", "99999", "", "3000000000"):
    cases.append(hp_case("hp-regen-%s" % (rg or "empty"), "30", 3, "80", rg, "0"))
for b in ("0", "5", "-7", "12.5", "-0.5", "100", ""):
    cases.append(hp_case("hp-bonus-%s" % (b or "empty"), "20", 1, "60", "0", b))
for i in range(25):
    cases.append(hp_case("hp-random-%02d" % i, str(rng.choice([0, 1, 3, 9, 14, 22, 37, 60, 90, 180, 400])),
                         rng.choice(STOCK_CLASSES + INV_CLASSES + [0]), str(rng.between(20, 140)), str(rng.choice([0, 0, 5, 20, -10, 60])),
                         str(rng.choice([0, 0, 0, 3, -4, 30, 12]))))

# ---------------------------------------------------------------------------------- RefreshMagic
def magic_case(cid, level, cls, int_t, wil_t, cha_t, regen, mana_b, sc_b):
    f = {**level_form(level), "txtCharStats": arr({1: {"Tag": int_t}, 2: {"Tag": wil_t}, 5: {"Tag": cha_t}}),
         "txtCharManaRegen": {"Text": regen}, "lblInvenCharStat": arr({6: {"Caption": mana_b}, 9: {"Caption": sc_b}}),
         "cmbGlobalClass": combo(cls, "c")}
    return case(cid, "RefreshMagic", forms=f)


for c in STOCK_CLASSES + INV_CLASSES + [0]:
    lvl = rng.choice(["1", "25"])
    cases.append(magic_case("magic-class-%d-l%s" % (c, lvl), lvl, c, "80", "70", "60", "0", "0", "0"))
for c in (3, 5, 9, 10, 12, 13, 15, 9003, 9004):
    for lvl in ("0", "3", "11", "50", "120", "300"):
        cases.append(magic_case("magic-grid-%d-l%s" % (c, lvl), lvl, c, str(rng.between(30, 140)), str(rng.between(30, 140)),
                                str(rng.between(30, 140)), str(rng.choice([0, 25, -30, 100])), "0", "0"))
for mb, sb in (("0", "0"), ("15", "0"), ("-9", "4"), ("0", "-3"), ("12.5", "7.5"), ("", ""), ("100000", "-2"), ("2147483647", "5")):
    cases.append(magic_case("magic-bonus-%s-%s" % (mb or "e", sb or "e"), "30", 12, "90", "50", "40", "10", mb, sb))
for rg in ("0", "10", "-10", "-100", "-150", "50", "33.5", "99999", "", "abc", "3000000000"):
    for c in (12, 15):
        cases.append(magic_case("magic-regen-%d-%s" % (c, rg or "empty"), "30", c, "90", "50", "40", rg, "0", "0"))
for t in ("0", "1", "-5", "255", "999", "40000", "3000000000", "12.5", "", "70.5"):
    cases.append(magic_case("magic-stat-%s" % (t or "empty"), "40", 13, t, t, t, "0", "0", "0"))
for i in range(25):
    cases.append(magic_case("magic-random-%02d" % i, str(rng.choice([0, 1, 4, 9, 22, 41, 77, 150])), rng.choice(STOCK_CLASSES + INV_CLASSES + [0]),
                            str(rng.between(10, 150)), str(rng.between(10, 150)), str(rng.between(10, 150)),
                            str(rng.choice([0, 0, 15, -20, 100])), str(rng.choice([0, 0, 8, -5])), str(rng.choice([0, 0, 6, -2]))))

# ---------------------------------------------------------------------------------- RefreshPicklocks
for i, (lvl, agl, intl, cha, bonus) in enumerate(
        [(1, 50, 50, 50, "0"), (0, 0, 0, 0, "0"), (15, 70, 60, 40, "0"), (16, 70, 60, 40, "0"), (17, 70, 60, 40, "5"),
         (30, 95, 80, 60, "-12"), (100, 120, 100, 80, "30"), (255, 200, 200, 200, "0"), (12, 40, 45, 35, "2.5"), (25, 65, 55, 50, ""),
         (40000, 60, 60, 60, "0"), (20, 3000000000, 60, 60, "0"), (20, 60, 3000000000, 60, "0"), (20, 60, 60, 60, "-100000"),
         (14, 33, 22, 11, "-1"), (8.5, 50, 50, 50, "0"), (9.5, 50.5, 50.5, 50, "0")]):
    f = {**level_form(str(lvl)), "txtCharStats": arr({3: {"Tag": str(agl)}, 1: {"Tag": str(intl)}, 5: {"Tag": str(cha)}}),
         "lblInvenCharStat": arr({22: {"Caption": bonus}})}
    cases.append(case("pick-%02d" % i, "RefreshPicklocks", forms=f))
for i in range(30):
    f = {**level_form(str(rng.choice([0, 1, 5, 14, 15, 16, 22, 33, 48, 90, 160]))),
         "txtCharStats": arr({3: {"Tag": str(rng.between(10, 150))}, 1: {"Tag": str(rng.between(10, 150))}, 5: {"Tag": str(rng.between(10, 150))}}),
         "lblInvenCharStat": arr({22: {"Caption": str(rng.choice([0, 0, 4, -3, 15]))}})}
    cases.append(case("pick-random-%02d" % i, "RefreshPicklocks", forms=f))

# ---------------------------------------------------------------------------------- RefreshMagicRes
for i, (it, wl, pm) in enumerate(
        [(50, 50, 0), (0, 0, 0), (100, 100, 10), (30, 90, -20), (255, 255, 99), (1, 1, 1), (3, 2, -2), (40000, 50, 0), (3000000000, 50, 0),
         (50, 3000000000, 0), (50.5, 50.5, 0), (49.5, 51.5, 3), (-30, -30, 0), (75, 60, 2147483647), (75, 60, -2147483647)]):
    f = {"txtCharStats": arr({1: {"Tag": str(it)}, 2: {"Tag": str(wl)}})}
    cases.append(case("mr-%02d" % i, "RefreshMagicRes", forms=f, globs={"nGlobalCharPlusMR": pm},
                      read={"mr": "%c.txtCharMR.Text"}))
for i in range(25):
    f = {"txtCharStats": arr({1: {"Tag": str(rng.between(0, 200))}, 2: {"Tag": str(rng.between(0, 200))}})}
    cases.append(case("mr-random-%02d" % i, "RefreshMagicRes", forms=f, globs={"nGlobalCharPlusMR": rng.choice([0, 0, 5, -8, 40])},
                      read={"mr": "%c.txtCharMR.Text"}))

# ---------------------------------------------------------------------------------- RefreshDodge
for i, tag in enumerate(["", "0", "12", "-4", "7.5", "100", "250.25", "abc", "1e2", "99999999999", "0.0001", "33"]):
    for cls in (1, 0):
        f = {"cmbGlobalClass": combo(cls, "c"), "lblInvenCharStat": arr({8: {"Tag": tag}})}
        cases.append(case("dodge-%02d-c%d" % (i, cls), "RefreshDodge", forms=f,
                          read={"dodge_tag": "%c.lblCharDodge.Tag", "dodge_cap": "%c.lblCharDodge.Caption"}))

# ---------------------------------------------------------------------------------- RefreshMainStatBonuses
LABEL_IDX = [1, 4, 24, 2, 23, 3]  # lblLabelArray indexes of STR INT WIL AGL HEA CHM
for i, tags in enumerate([["0"] * 6, ["3", "-2", "0", "7", "-9", "1"], ["12.5", "0.4", "-0.5", "1.5", "2.5", "-1.5"], ["", "x", "5", "100000", "-100000", "0"],
                          ["3000000000", "1", "1", "1", "1", "1"], ["0", "0", "0", "0", "0", "-1"], ["1"] * 6, ["-1"] * 6]):
    f = {"lblLabelArray": arr({LABEL_IDX[k]: {"Tag": t} for k, t in enumerate(tags)}), "txtCharStats": arr({0: {"Text": "61"}, 5: {"Text": "45"}})}
    cases.append(case("labels-%02d" % i, "RefreshMainStatBonuses", forms=f,
                      read={"labels": "%c.lblLabelArray.Caption[0..24]"}))
for i in range(12):
    tags = [str(rng.choice([0, 0, 1, -1, 4, -6, 11, 25])) for _ in range(6)]
    f = {"lblLabelArray": arr({LABEL_IDX[k]: {"Tag": t} for k, t in enumerate(tags)})}
    cases.append(case("labels-random-%02d" % i, "RefreshMainStatBonuses", forms=f, read={"labels": "%c.lblLabelArray.Caption[0..24]"}))

# ---------------------------------------------------------------------------------- SetupClass
SETUP_READ = {"qopts": "%c.cmbCharQuestOpts.ListIndex[0..3]", "antimagic": "%c.chkCharAntiMagic.Value",
              "nonmagic": "%c.chkArmourNonMagic.Value", "weaponopt": "%c.chkWeaponOptions(0).Value",
              "spellopt": "%c.chkSpellOptions(1).Value", "mlvl_idx": "%c.cmbSpellMageryLevel.ListIndex",
              "magery_idx": "%c.cmbSpellMagery.ListIndex", "fra": "%c.fraChar.Enabled[2..3]",
              "lblpick": "%c.lblCharPicklocks.Enabled", "lblsc": "%c.lblCharSC.Enabled",
              "armour": "%c.chkArmourType.Value[0..6]", "handed": "%c.chkHanded.Value[0..3]",
              "mageryLVL": "%g.nGlobalCharMageryLVL", "magery": "%g.nGlobalCharMagery"}


def setup_case(cid, cls, quests=(0, 0, 0, 0), opts=(0, 0, 0, 0), mlc=4, mc=6, text=None):
    """quests: chkCharQuests(5, 9, 10, 11).Value; opts: cmbCharQuestOpts(0..3).ListIndex."""
    f = {"chkCharQuests": arr({5: {"Value": quests[0]}, 9: {"Value": quests[1]}, 10: {"Value": quests[2]}, 11: {"Value": quests[3]}}),
         "cmbCharQuestOpts": arr({k: {"ListIndex": o} for k, o in enumerate(opts)}),
         "cmbSpellMageryLevel": {"ListCount": mlc, "ListIndex": -1}, "cmbSpellMagery": {"ListCount": mc, "ListIndex": -1}}
    if cls is not None:
        c = combo(cls, "c")
        if text is not None:
            c["$array"]["0"]["Text"] = text
        f["cmbGlobalClass"] = c
    return case(cid, "SetupClass", forms=f, read=SETUP_READ)


for c in STOCK_CLASSES + INV_CLASSES + [0, 16, 99]:
    for k, (q, o) in enumerate((((0, 0, 0, 0), (0, 0, 0, 0)), ((1, 1, 1, 1), (4, 3, 2, 1)), ((1, 0, 1, 0), (0, 2, 0, 3)),
                                ((0, 1, 0, 1), (2, 0, 3, 0)))):
        cases.append(setup_case("setup-c%d-%d" % (c, k), c, q, o))
cases.append(setup_case("setup-noindex", None, (0, 0, 0, 0), (3, 3, 3, 3)))
cases.append(setup_case("setup-nonetext", 4, (0, 0, 0, 0), (3, 3, 3, 3), text="(none)"))
cases.append(setup_case("setup-nonetext-0", 0, (0, 0, 0, 0), (3, 3, 3, 3), text="(none)"))
for c in (1, 5, 9, 15, 9001, 9003):
    for mlc, mc in ((0, 0), (2, 3), (4, 6), (6, 2)):
        cases.append(setup_case("setup-lists-c%d-%d-%d" % (c, mlc, mc), c, (0, 0, 0, 0), (0, 0, 0, 0), mlc, mc))

# ---------------------------------------------------------------------------------- lblInvenCharStat_Change (encumbrance level)
for cur, mx in ((0, 0), (1, 0), (5, 0), (0.5, 0), (0, 100), (10, 100), (16, 100), (17, 100), (33, 100), (34, 100), (50, 100), (66, 100), (67, 100),
                (99, 100), (100, 100), (150, 100), (400, 100), (16.9, 100), (33.9, 100), (66.9, 100), (1, 3), (2, 3), (1, 6), (5, 7), (13, 19),
                (-5, 100), (5, -100), (0, -50), (123456789, 1), (123456, 1), (9999999, 100000), (100, 0.01), (1e30, 1e-5), (7, 0.5),
                (88, 261), (1234, 5678), (3, 8)):
    for ix in (0, 1):
        f = {"lblInvenCharStat": arr({0: {"Caption": num(cur)}, 1: {"Caption": num(mx)}}), "lblEncumLevel": arr({1: {}}),
             "txtCharStats": arr({0: {"Tag": "60"}}), "picStats": arr({0: {}, 1: {}})}
        cases.append(case("encum-%s-%s-i%d" % (cur, mx, ix), "lblInvenCharStat_Change", [ix], forms=f,
                          read={"lvl_cap": "%c.lblEncumLevel(1).Caption", "lvl_tag": "%c.lblEncumLevel(1).Tag"}))
for i in range(30):
    mx = rng.between(0, 900)
    cur = rng.between(0, 1200)
    f = {"lblInvenCharStat": arr({0: {"Caption": num(cur)}, 1: {"Caption": num(mx)}}), "lblEncumLevel": arr({1: {}}),
         "txtCharStats": arr({0: {"Tag": "60"}}), "picStats": arr({0: {}, 1: {}})}
    cases.append(case("encum-random-%02d" % i, "lblInvenCharStat_Change", [rng.choice([0, 1])], forms=f,
                      read={"lvl_cap": "%c.lblEncumLevel(1).Caption", "lvl_tag": "%c.lblEncumLevel(1).Tag"}))

# ---------------------------------------------------------------------------------- CharStatAdjustmentPrompt
ANSWERS = ["", "5", "-5", "0", "-9999", "-9999.5", "-10000", "-12345", "9999", "10000", "99999", "abc", " 12 ", "1.55", "2.5", "3.5", "2.4",
           "1e3", "&H10", "+7", "-0", "40000", "-40000", "12abc", "  ", "0.04", "3276.8", "32767", "214748364.7", "9999.4"]
for ix in (0, 2, 3, 10, 34, 46):
    for k, ans in enumerate(ANSWERS):
        cur = rng.choice([0, 0, 5, -20, 120, 7, 25])
        arr_adj = [0] * 47
        arr_adj[ix] = cur
        cases.append(case("adj-%d-%02d" % (ix, k), "CharStatAdjustmentPrompt", [ix],
                          forms={"tsInput": {"Caption": ans}},
                          globs={"char_StatAdjustments": arr_adj}, read={"adj": "%g.char_StatAdjustments", "dirty": "%g.bPromptSave"}))
# a current value equal to the answer (and the AC/DR tenth rounding)
for ix, cur, ans in ((2, 50, "5"), (3, 55, "5.5"), (2, 55, "5.5"), (3, 3, "0.3"), (2, -10, "-1"), (5, 8, "8"), (5, 8, "8.4"), (10, 0, "0")):
    arr_adj = [0] * 47
    arr_adj[ix] = cur
    cases.append(case("adj-same-%d-%d-%s" % (ix, cur, ans), "CharStatAdjustmentPrompt", [ix],
                      forms={"tsInput": {"Caption": ans}},
                      globs={"char_StatAdjustments": arr_adj}, read={"adj": "%g.char_StatAdjustments", "dirty": "%g.bPromptSave"}))

# ---------------------------------------------------------------------------------- RefreshCharBless: mana per round
def bless_case(cid, level, slots):
    g = {}
    for k, n in enumerate(slots):
        if n:
            g[str(k)] = {"ListIndex": 0, "ItemData": [n]}
            used_spells.add(n)
    return case(cid, "RefreshCharBless", forms={**level_form(level), "cmbCharBless": {"$array": g}},
                globs={"nGlobalCharWornArmourType": rng.choice([0, 3, 6, 9])},
                read={"mana": "%c.lblCharBless.Caption", "slots": "%g.nGlobalCharBlessSpells"})


cases.append(bless_case("bless-none", "20", [0] * 10))
for n in BLESS_ALL:
    lvl = rng.choice(["1", "9", "30", "70"])
    cases.append(bless_case("bless-one-%d-l%s" % (n, lvl), lvl, [n] + [0] * 9))
for lvl in ("", "0", "2", "5", "12", "25", "40", "99", "200", "abc", "17.5"):
    for j in range(3):
        slots = [rng.choice(BLESS_ALL if rng.chance(80) else BLESS_POOL) if rng.chance(70) else 0 for _ in range(10)]
        cases.append(bless_case("bless-set-l%s-%d" % (lvl or "empty", j), lvl, slots))
for i in range(25):
    slots = [rng.choice(BLESS_KEY if rng.chance(75) else BLESS_POOL) if rng.chance(55) else 0 for _ in range(10)]
    cases.append(bless_case("bless-random-%02d" % i, str(rng.choice([1, 3, 8, 15, 27, 40, 80])), slots))
# a failing spell (duration 0) after good ones, and before them
cases.append(bless_case("bless-fail-last", "20", [34, 52, 9001, 0, 0, 0, 0, 0, 0, 0]))
cases.append(bless_case("bless-fail-first", "20", [9001, 34, 52, 0, 0, 0, 0, 0, 0, 0]))
cases.append(bless_case("bless-neg-dur", "20", [9004, 34, 0, 0, 0, 0, 0, 0, 0, 0]))
cases.append(bless_case("bless-dear", "30", [9005, 9005, 9005, 9005, 9005, 9005, 9005, 9005, 9005, 9005]))

# ---------------------------------------------------------------------------------- data pool
for c in STOCK_CLASSES:
    used_classes.add(c)
for r in STOCK_RACES:
    used_races.add(r)


def spell_row(n):
    r = spells[n]
    return {"n": n, "req": r["ReqLevel"], "cap": r["Cap"], "mn": r["MinBase"], "mx": r["MaxBase"], "mni": r["MinInc"],
            "mnl": r["MinIncLVLs"], "mxi": r["MaxInc"], "mxl": r["MaxIncLVLs"], "dur": r["Dur"], "di": r["DurInc"],
            "dl": r["DurIncLVLs"], "mana": r["ManaCost"], "ab": ab(r, 10)}


data = {"spells": [spell_row(n) for n in sorted(used_spells)],
        "classes": [{"n": n, "min": classes[n]["MinHits"], "max": classes[n]["MaxHits"], "exp": classes[n]["ExpTable"],
                     "mt": classes[n]["MageryType"], "ml": classes[n]["MageryLVL"], "ab": ab(classes[n], 10)} for n in sorted(used_classes) if n in classes],
        "races": [{"n": n, "hp": races[n]["HPPerLVL"], "cp": races[n]["BaseCP"], "exp": races[n]["ExpTable"]} for n in sorted(used_races)]}

FRM = {
    "file": "frmMain.frm",
    "decls": ["bless_Stats", "bless_StatText", "char_StatAdjustments", "bDontRefresh", "bDontRefreshInvenStats"],
    "procs": ["RefreshCPs", "RefreshHitPoints", "RefreshMagic", "RefreshPicklocks", "RefreshMagicRes", "RefreshDodge",
              "RefreshMainStatBonuses", "RefreshCharBless", "SetupClass", "lblInvenCharStat_Change", "CharStatAdjustmentPrompt"],
    "stub_procs": ["GetDodgeSummary", "SetCharDefenseDescription", "ResetFilterOptions", "PopUpChooseCombatGUI", "RefreshAll"],
    # the only tool-tip work in lblInvenCharStat_Change after the level text is display; cut there
    "cuts": {"lblInvenCharStat_Change": {"cut": "tooltip:", "append": ["tooltip:", "End Select"]}},
    # the InputBox answer comes from a control
    "subs": {"CharStatAdjustmentPrompt": {
        'sNew = Trim(InputBox("Enter " & UCase(sStat) & " Adjustment" & vbCrLf & vbCrLf & sExtra, "Stat Override", nCur))':
            "sNew = Trim(frmMain.tsInput.Caption)"}},
}
READ = {"cps": "%c.lblStatCalc.Caption", "cps_tag": "%c.lblStatCalc.Tag",
        "rest_tag": "%c.lblCharRestRate.Tag", "rest_cap": "%c.lblCharRestRate.Caption",
        "hp_tag": "%c.lblCharMaxHP.Tag", "hp_cap": "%c.lblCharMaxHP.Caption",
        "mana_rate_tag": "%c.lblCharManaRate.Tag", "mana_rate_cap": "%c.lblCharManaRate.Caption", "medi": "%c.txtCharManaRegen.Tag",
        "maxmana_tag": "%c.lblCharMaxMana.Tag", "maxmana_cap": "%c.lblCharMaxMana.Caption",
        "sc_tag": "%c.lblCharSC.Tag", "sc_cap": "%c.lblCharSC.Caption",
        "pick_tag": "%c.lblCharPicklocks.Tag", "pick_cap": "%c.lblCharPicklocks.Caption"}

scn = {
    "family": "char_panel",
    "description": "Char panel derivations (FAM-05): frmMain RefreshCPs, RefreshHitPoints, RefreshMagic, RefreshPicklocks, "
                   "RefreshMagicRes, RefreshDodge, RefreshMainStatBonuses, SetupClass, lblInvenCharStat_Change, "
                   "CharStatAdjustmentPrompt and the mana-per-round total of RefreshCharBless",
    "notes": [
        "Each case calls one procedure cut out of frmMain.frm (scenario key frm) and reads back the captions, tags and "
        "globals it writes; the step's call name says which. Controls MME reads are in the case's forms.",
        "lblInvenCharStat_Change is cut at its 'tooltip:' label (what follows is tool-tip placement) and only the "
        "encumbrance-level indexes 0 and 1 are run; indexes 2, 3, 8 and 24 call RefreshMagicRes and RefreshDodge, "
        "which have their own cases.",
        "CharStatAdjustmentPrompt: the InputBox call is replaced (scenario subs) by a read of frmMain.tsInput.Caption; "
        "an empty answer is MME's Cancel. PopUpChooseCombatGUI and RefreshAll are stubs.",
        "SetupClass: ResetFilterOptions is a stub; ListCount of the two spell-magery combos is an input, and an unset "
        "ListIndex reads -1.",
        "RefreshCharBless: only lblCharBless.Caption (the mana-per-round total) is new here; bless_Stats is the gear family.",
        "Rows numbered 9001 and up are invented (db_extra); data holds every row the cases use, for the Rust test.",
    ],
    "database": True,
    "frm": FRM,
    "db_extra": {"Spells": EXTRA_SPELLS, "Races": EXTRA_RACES, "Classes": EXTRA_CLASSES},
    "data": data,
    "forms": {"frmMain": {"lblDatVer": {"Caption": "v1.11p"}, "lblInvenCharStat": arr({46: {}}), "lblLabelArray": arr({24: {}}),
                          "txtCharStats": arr({5: {}}), "txtCharMaxStats": arr({5: {}}), "cmbGlobalClass": arr({0: {}}),
                          "cmbGlobalRace": arr({0: {}}), "txtGlobalLevel": arr({0: {}}), "cmbCharBless": arr({9: {}}),
                          "chkGlobalFilter": {"Value": 0}, "txtCharHPRegen": {}, "txtCharManaRegen": {},
                          "chkArmourType": arr({k: {"Value": 0} for k in range(7)}), "chkHanded": arr({k: {"Value": 0} for k in range(4)}),
                          "fraChar": arr({k: {} for k in range(4)}), "chkWeaponOptions": arr({0: {"Value": 5}}),
                          "chkSpellOptions": arr({1: {"Value": 5}}), "chkCharAntiMagic": {"Value": 5}, "chkArmourNonMagic": {"Value": 5},
                          "lblInvenStats": arr({i: {"Caption": "Stat%d:" % i} for i in range(34)}),
                          "cmbSpellMageryLevel": {"ListIndex": -1}, "cmbSpellMagery": {"ListIndex": -1}}},
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d spells, %d classes, %d races" % (len(cases), len(data["spells"]), len(data["classes"]), len(data["races"])), file=sys.stderr)

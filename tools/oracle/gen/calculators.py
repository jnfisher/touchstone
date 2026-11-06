"""Writes fixtures/scenarios/calculators.json (FAM-27, the four calculator windows):
    python3 -I tools/oracle/gen/calculators.py fixtures/scenarios/calculators.json

CalcSwings (frmSwingCalc), CalcBS (frmBSCalc), CalcCoin / ConvertCoin (frmCoinConvert) and CalcExp
(frmExpCalc) are procedures of their forms; the scenario's `frm` key cuts them out of the .frm files
and their controls resolve to the frmMain shim (one calculator per case). The first case, "table",
carries the Items, Classes and Races rows the cases look up in compact form; "sn-*" and "pe-*" cases
record the weapon cast-spell text CalculateAttack asks other cards' procedures for.
Bulk cases are seeded samples (splitmix64 in the oracle), so the scenario stays small. Deterministic."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row
MISSING = {"t": "missing"}

# ---------------------------------------------------------------------------------- rows
items = {}
for r in con.execute("select * from Items order by Number"):
    items[r["Number"]] = dict(r)
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}


def ab(row, n=20):
    return [(row["Abil-%d" % i], row["AbilVal-%d" % i]) for i in range(n)]


def item(n, name, **kw):
    r = {"Number": n, "Name": name, "ItemType": 1, "Min": 4, "Max": 9, "Speed": 1500, "StrReq": 0, "WeaponType": 0}
    for k, v in kw.items():
        if k == "abil":
            for i, (a, val) in enumerate(v):
                r["Abil-%d" % i] = a
                r["AbilVal-%d" % i] = val
        else:
            r[k] = v
    return r


# invented rows (9001+) reach branches no stock row has
EXTRA_ITEMS = [
    item(9001, "tst all four", abil=[(116, 5), (4, 5), (117, 3), (118, 4), (27, 6)]),
    item(9002, "tst negatives", abil=[(116, 2), (117, -3), (118, -2), (4, -1), (27, -4)]),
    item(9003, "tst two hand", WeaponType=1, Min=10, Max=20, abil=[(116, 4)]),
    item(9004, "tst two hand 3", WeaponType=3, Min=1, Max=2, abil=[(116, 7), (27, 2), (118, 2)]),
    item(9005, "tst zero and twice", abil=[(116, 3), (117, 0), (4, 3), (4, 4), (118, 5), (118, -9), (14, 6)]),
    item(9006, "tst last slot", abil=[(0, 0)] * 19 + [(116, 1)]),
    item(9007, "tst no bs", abil=[(4, 8), (117, 5)]),
    item(9008, "tst big max", Min=30000, Max=32760, abil=[(116, 1)]),
    item(9009, "tst min over max", Min=50, Max=2, abil=[(116, 1)]),
    item(9010, "tst slow", Speed=0, StrReq=0, abil=[(116, 1)]),
    item(9011, "tst heavy", Speed=1000, StrReq=150, abil=[(116, 1), (27, 9)]),
    item(9012, "tst plain", ItemType=0, abil=[(116, 2)]),
]
EXTRA_CLASSES = [{"Number": 9001, "Name": "tst big exp", "ExpTable": 32700, "CombatLVL": 3},
                 {"Number": 9002, "Name": "tst small exp", "ExpTable": -32768, "CombatLVL": 2},
                 {"Number": 9003, "Name": "tst mid exp", "ExpTable": 20000, "CombatLVL": 5}]
EXTRA_RACES = [{"Number": 9001, "Name": "tst big exp", "ExpTable": 32700},
               {"Number": 9002, "Name": "tst small exp", "ExpTable": -32768},
               {"Number": 9003, "Name": "tst mid exp", "ExpTable": 20000}]
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          **{"Encum": 0, "Accy": 0}, **r}
for r in EXTRA_CLASSES:
    classes[r["Number"]] = {**{k: 0 for k in classes[1]}, **r}
for r in EXTRA_RACES:
    races[r["Number"]] = {**{k: 0 for k in races[1]}, **r}

# ---------------------------------------------------------------------------------- pools
weapons = [n for n, r in items.items() if r["ItemType"] == 1 and n < 9000]
cast_w = [n for n in weapons if any(a == 43 for a, v in ab(items[n]))]
bs_w = [n for n in weapons if any(a == 116 for a, v in ab(items[n]))]
stat_w = [n for n in weapons if any(a in (4, 117, 118, 27) and v != 0 for a, v in ab(items[n]))]
twohand_w = [n for n in weapons if items[n]["WeaponType"] in (1, 3)]
others = [n for n, r in items.items() if r["ItemType"] != 1 and n < 9000][::211][:5]

# swing pool: a spread over speed and strength requirement, some two-handed, some casting spells
by_speed = sorted(weapons, key=lambda n: (items[n]["Speed"], items[n]["StrReq"], n))
swing_core = by_speed[::9][:42] + sorted(weapons, key=lambda n: (-items[n]["StrReq"], n))[:6] + twohand_w[::17][:5]
swing_cast = cast_w[::23][:7]
swing_pool = sorted(set(swing_core + swing_cast + [9010, 9011]))
SWING_ODD = [99999, 0, -4]                       # no such item (Seek fails), and numbers below the table
# backstab pool: weapons with bs accuracy (some with stat abilities), some without, the invented ones
bs_core = sorted(set(bs_w[::11][:18] + stat_w[::2][:16] + twohand_w[::13][:4]
                     + [n for n in weapons if n not in bs_w][::37][:5] + others[:2]))
bs_pool = sorted(set(bs_core + [n for n in items if 9001 <= n <= 9012]))
spells = sorted({v for n in swing_pool for a, v in ab(items[n]) if a == 43})
BONUSES = [0, 25]


def flat(abil):
    f = [x for pair in abil for x in pair]
    while f and f[-1] == 0:
        f.pop()
    return f


table_nums = sorted(set(swing_pool + bs_pool + others))
cases = [{"id": "table",
          "items": [[n, items[n]["Encum"], items[n]["Accy"], items[n]["StrReq"], items[n]["Speed"], items[n]["Min"],
                     items[n]["Max"], items[n]["WeaponType"], flat(ab(items[n]))] for n in table_nums],
          "item_fields": ["Number", "Encum", "Accy", "StrReq", "Speed", "Min", "Max", "WeaponType", "AbilitiesFlat"],
          "classes": [[n, r["CombatLVL"], r["ExpTable"], [r["Abil-%d" % i] for i in range(10)]] for n, r in classes.items()],
          "races": [[n, r["ExpTable"], [r["Abil-%d" % i] for i in range(10)]] for n, r in races.items()],
          "class_fields": ["Number", "CombatLVL", "ExpTable", "Abil-0..9"], "race_fields": ["Number", "ExpTable", "Abil-0..9"]}]
for s in spells:
    cases.append({"id": "sn-%d" % s, "proc": "GetSpellName", "args": [s, False]})
    for b in BONUSES:
        cases.append({"id": "pe-%d-%d" % (s, b), "proc": "PullSpellEQ",
                      "args": [True, 0, s, MISSING, MISSING, MISSING, True, MISSING, MISSING, MISSING, MISSING, b]})

samples, grids = [], []


def arr(n, **kw):
    return {"$array": {str(i): dict(kw) for i in range(n)}}


def opts(n, on):
    """optSpeed / optCoin*: radio buttons, exactly one (or none) selected."""
    return {"$array": {str(i): {"Value": (i == on)} for i in range(n)}}


def txt(name, var):
    return {name: {"Text": var}}


# ---------------------------------------------------------------------------------- swings
SW_READ = {"qnd": "%c.lblQND.Caption", "energy": "%c.lblEnergy.Caption", "encum": "%c.lblEncum.Caption",
           "encum_tag": "%c.lblEncum.Tag", "raw": "%c.lblRawSwing.Caption", "raw_tag": "%c.lblRawSwing.Tag",
           "avg6": "%c.txtTrueAVG(6).Text", "eu": "%c.lblEU.Caption[0..9]", "swing": "%c.txtSwing.Text[0..9]",
           "dmg0": "%c.lblSwingDamage(0).Caption", "dmg1": "%c.lblSwingDamage(1).Caption",
           "speed_text": "%c.txtSpeed.Text"}
SPEED_OPTS = [opts(4, 1), opts(4, 1), opts(4, 0), opts(4, 2), opts(4, 3), opts(4, 3), opts(4, -1)]
LV = ["1", "5", "12", "20", "30", "30", "50", "80", "100", "150", "255", "0", "", "abc", "2.5", "-3", "9999"]
AGI = ["10", "40", "60", "90", "100", "120", "150", "200", "255", "255", "0", "", "-150", "-40", "300", "1e2"]
STR = ["0", "20", "45", "60", "80", "100", "100", "120", "150", "180", "255", "", "-10", "x"]
ENC = ["0", "0", "100", "500", "1000", "2000", "2600", "4800", "6000", "9000", "", "-50", "abc"]
MAXENC = ["4800", "4800", "1000", "2600", "6000", "100", "0", "", "-5", "999999", "5000000", "x"]
SPD = ["100", "85", "125", "60", "150", "0", "-10", "-1", "abc", "", "40000", "32767", "150.6", "99.5", "100.5"]
draw = {"w": {"choice": swing_pool * 4 + SWING_ODD}, "wli": {"choice": [0] * 30 + [-1]},
        "comb": {"choice": [0, 1, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 8, 12]},
        "slow": {"choice": [0, 0, 1]}, "bash": {"choice": [0, 0, 0, 1]},
        "lv": {"choice": LV}, "agi": {"choice": AGI}, "str": {"choice": STR},
        "enc": {"choice": ENC}, "maxenc": {"choice": MAXENC}, "spd": {"choice": SPD}, "opt": {"choice": SPEED_OPTS}}
sw_forms = {"cmbWeapon": {"ListIndex": "$wli", "ItemData": ["$w"]}, "cmbCombat": {"ListIndex": 0, "ItemData": ["$comb"]},
            "chkSlowness": {"Value": "$slow"}, "chkBashing": {"Value": "$bash"},
            "txtEncum": {"Text": "$enc"}, "txtMaxEncum": {"Text": "$maxenc"}, "txtLevel": {"Text": "$lv"},
            "txtAgility": {"Text": "$agi"}, "txtStrength": {"Text": "$str"}, "txtSpeed": {"Text": "$spd"}, "optSpeed": "$opt"}
samples.append({"name": "swing", "seed": 4301, "count": 180, "draw": draw,
                "case": {"steps": [{"call": "CalcSwings"}], "read": SW_READ,
                         "forms": {"frmMain": {**sw_forms, "chkGlobalFilter": {"Value": 0}}}}})
# sensible inputs, so most of the draws reach the end of the procedure with a real weapon
draw2 = dict(draw)
draw2.update({"lv": {"choice": ["5", "12", "20", "30", "40", "50", "80", "100", "150", "255"]},
              "agi": {"choice": ["40", "60", "80", "100", "120", "150", "200", "255"]},
              "str": {"choice": ["30", "60", "80", "100", "120", "150", "200"]},
              "spd": {"choice": ["100", "85", "125", "50", "75", "110", "150"]},
              "w": {"choice": swing_pool}, "wli": {"choice": [0]}})
samples.append({"name": "swingok", "seed": 4302, "count": 90, "draw": draw2,
                "case": {"steps": [{"call": "CalcSwings"}], "read": SW_READ,
                         "forms": {"frmMain": {**sw_forms, "chkGlobalFilter": {"Value": 0}}}}})

# global filter on: PopulateCharacterProfile fills the profile from the main window's panels
G_NAMES = ["Number", "Encum", "Accy", "Crit", "STR", "AGI", "MaxDmg", "BSaccy", "BSmindmg", "BSmaxdmg", "PunchSkill",
           "PunchAccy", "PunchDmg", "KickSkill", "KickAccy", "KickDmg", "JkSkill", "JkAccy", "JkDmg", "Stealth"]
gdraw = dict(draw2)
gdraw.update({"cls": {"int": [1, 15]}, "race": {"int": [1, 13]},
              "crit": {"choice": ["0", "0", "5", "12", "25", "40", "41", "55", "80", "130", "-10"]},
              "pmax": {"choice": ["0", "3", "12", "-3", "40"]}, "pmin": {"choice": ["0", "2", "-2", "9"]},
              "pbsa": {"choice": ["0", "4", "-5", "30"]}, "pbsmin": {"choice": ["0", "6", "-4"]},
              "pbsmax": {"choice": ["0", "9", "-5", "40"]}, "stl": {"choice": ["0", "30", "80", "150", "255"]},
              "acc": {"choice": ["0", "8", "40.5", "90", "150", "260", "400"]}, "sdb": {"choice": ["0", "0", "25", "25"]},
              "int": {"choice": ["10", "50", "120", "200"]}, "cha": {"choice": ["10", "60", "200"]},
              "ecur": {"choice": ["0", "500", "2000", "4800"]}, "emax": {"choice": ["0", "20", "4800", "6000"]},
              "maskill": {"n": 3, "choice": ["0", "0", "1", "8", "25", "-2"]},
              "maaccy": {"n": 3, "choice": ["0", "5", "-3", "40"]}, "madmg": {"n": 3, "choice": ["0", "2", "-1", "30"]},
              "qnd": {"choice": [0, 0, 3, 10, 20]}, "aabil": {"int": [-20, 60]}, "aoth": {"int": [-20, 60]},
              "aitem": {"int": [-20, 60]}})
for n in G_NAMES:
    lo, hi = {"Encum": (0, 300), "Accy": (-10, 40), "Crit": (-5, 15), "MaxDmg": (-5, 20), "Stealth": (-10, 30)}.get(n, (-5, 15))
    gdraw["g" + n] = {"n": 2, "choice": ([0, 0] + swing_pool + [9010]) if n == "Number" else None, "int": None}
    if n == "Number":
        gdraw["g" + n] = {"n": 2, "choice": [0, 0, 0] + swing_pool}
    else:
        gdraw["g" + n] = {"n": 2, "int": [lo, hi]}
stat = lambda i, k, v: {str(i): {k: v}}
panel = {"0": {"Caption": "$ecur"}, "1": {"Caption": "$emax"}, "7": {"Tag": "$crit"}, "10": {"Tag": "$acc"},
         "11": {"Tag": "$pmax"}, "13": {"Tag": "$pbsa"}, "14": {"Tag": "$pbsmin"}, "15": {"Tag": "$pbsmax"},
         "19": {"Tag": "$stl"}, "30": {"Tag": "$pmin"}, "33": {"Tag": "$sdb"},
         "34": {"Tag": "$madmg0"}, "35": {"Tag": "$madmg1"}, "36": {"Tag": "$madmg2"},
         "37": {"Tag": "$maskill0"}, "38": {"Tag": "$maskill1"}, "39": {"Tag": "$maskill2"},
         "40": {"Tag": "$maaccy0"}, "41": {"Tag": "$maaccy1"}, "42": {"Tag": "$maaccy2"}}
# list draws are split into one variable per element by the generator below (the oracle substitutes whole values)
for base in ("maskill", "maaccy", "madmg"):
    for k in range(3):
        gdraw["%s%d" % (base, k)] = {"choice": gdraw[base]["choice"]}
    del gdraw[base]
gforms = {**sw_forms, "chkGlobalFilter": {"Value": 1}, "txtGlobalLevel": {"$array": {"0": {"Text": "$lv"}}},
          "cmbGlobalClass": {"$array": {"0": {"ListIndex": 0, "ItemData": ["$cls"]}}},
          "cmbGlobalRace": {"$array": {"0": {"ListIndex": 0, "ItemData": ["$race"]}}},
          "lblInvenCharStat": {"$array": panel},
          "txtCharStats": {"$array": {"1": {"Tag": "$int"}, "5": {"Tag": "$cha"}}}}
gglobals = {"nGlobalCharWeapon" + n: "$g" + n for n in G_NAMES}
gglobals.update({"nGlobalCharQnDbonus": "$qnd", "nGlobalCharAccyAbils": "$aabil", "nGlobalCharAccyOther": "$aoth",
                 "nGlobalCharAccyItems": "$aitem"})
samples.append({"name": "swingfilt", "seed": 4303, "count": 130, "draw": gdraw,
                "case": {"steps": [{"call": "CalcSwings"}], "read": SW_READ, "forms": {"frmMain": gforms},
                         "globals": gglobals}})

# ---------------------------------------------------------------------------------- backstab
BS_READ = {"dmg": "%c.lblDMG.Caption", "max_adj": "%c.lblMaxAdj.Caption", "bsmin_adj": "%c.lblBSMinAdj.Caption",
           "bsmax_adj": "%c.lblBSMaxAdj.Caption", "stealth_adj": "%c.lblStealthAdj.Caption",
           "t_bsmin": "%c.txtBSMinDMG.Text", "t_bsmax": "%c.txtBSMaxDMG.Text", "t_level": "%c.txtLevel.Text",
           "t_stealth": "%c.txtStealth.Text", "t_maxdmg": "%c.txtMaxDMG.Text"}
PLUS = ["0", "0", "0", "5", "12", "30", "100", "999", "1000", "1001", "5000", "-7", "", "abc", "7.5", "40.4"]
BLV = ["1", "10", "30", "45", "60", "100", "100", "255", "0", "1000", "1500", "-5", "", "12.5"]
BST = ["0", "20", "50", "80", "100", "150", "255", "300", "1000", "2000", "-20", "", "37"]
BSTR = ["0", "50", "100", "100", "110", "150", "200", "255", "500", "1000", "200000", "-30", "", "x"]
bdraw = {"w": {"choice": bs_pool * 2 + [99999, 0]}, "wli": {"choice": [0] * 30 + [-1]},
         "bsmin": {"choice": PLUS}, "bsmax": {"choice": PLUS}, "lv": {"choice": BLV}, "stl": {"choice": BST},
         "mx": {"choice": PLUS}, "str": {"choice": BSTR}, "cs": {"choice": [0, 1]},
         "gnum": {"n": 2, "choice": [0, 0, 0] + bs_pool}, "gmax": {"n": 2, "choice": [0, 0, 5, -3, 12, 40]},
         "gbmin": {"n": 2, "choice": [0, 0, 4, -2, 9]}, "gbmax": {"n": 2, "choice": [0, 0, 6, -5, 14]},
         "gstl": {"n": 2, "choice": [0, 0, 10, -8, 25]}}
bs_forms = {"cmbWeapon": {"ListIndex": "$wli", "ItemData": ["$w"]}, "txtBSMinDMG": {"Text": "$bsmin"},
            "txtBSMaxDMG": {"Text": "$bsmax"}, "txtLevel": {"Text": "$lv"}, "txtStealth": {"Text": "$stl"},
            "txtMaxDMG": {"Text": "$mx"}, "txtStrength": {"Text": "$str"}, "chkClassStealth": {"Value": "$cs"}}
bs_globals = {"nGlobalCharWeaponNumber": "$gnum", "nGlobalCharWeaponMaxDmg": "$gmax", "nGlobalCharWeaponBSmindmg": "$gbmin",
              "nGlobalCharWeaponBSmaxdmg": "$gbmax", "nGlobalCharWeaponStealth": "$gstl"}
for name, filt, seed, count in (("bs", 0, 4311, 90), ("bsfilt", 1, 4312, 190)):
    samples.append({"name": name, "seed": seed, "count": count, "draw": bdraw,
                    "case": {"steps": [{"call": "CalcBS"}], "read": BS_READ,
                             "forms": {"frmMain": {**bs_forms, "chkGlobalFilter": {"Value": filt}}},
                             "globals": bs_globals}})
# the equipped weapon is the one being looked at: its own stats are not subtracted
bdraw_same = dict(bdraw)
bdraw_same.update({"gnum": {"n": 1, "choice": bs_pool}})
bs_globals_same = dict(bs_globals)
bs_globals_same["nGlobalCharWeaponNumber"] = ["$w", 0]
samples.append({"name": "bssame", "seed": 4313, "count": 30, "draw": {k: v for k, v in bdraw.items() if k != "gnum"},
                "case": {"steps": [{"call": "CalcBS"}], "read": BS_READ,
                         "forms": {"frmMain": {**bs_forms, "chkGlobalFilter": {"Value": 1}}},
                         "globals": bs_globals_same}})
# sensible numbers, so the damage line is reached for every pooled weapon
sens = dict(bdraw)
sens.update({"bsmin": {"choice": ["0", "5", "12"]}, "bsmax": {"choice": ["0", "8", "20"]}, "lv": {"choice": ["20", "45", "100"]},
             "stl": {"choice": ["0", "60", "120"]}, "mx": {"choice": ["0", "6"]}, "str": {"choice": ["80", "100", "140", "200"]},
             "w": {"choice": bs_pool}, "wli": {"choice": [0]}})
samples.append({"name": "bsok", "seed": 4314, "count": 60, "draw": sens,
                "case": {"steps": [{"call": "CalcBS"}], "read": BS_READ,
                         "forms": {"frmMain": {**bs_forms, "chkGlobalFilter": {"Value": 1}}}, "globals": bs_globals}})

# ---------------------------------------------------------------------------------- coins
COIN_READ = {"text": "%c.txtCoin(%d).Text", "other": "%c.txtCoin(%d).Text", "cur": "%g.eCurrentCoin"}
COINS = [1, 10, 100, 10000, 1000000]
CT = ["0", "1", "5", "10", "100", "1234", "99999", "1e3", "2.5", "0.5", "0.000001", "3.33333333", "-7", "", "abc", "12abc",
      "9999999999", "99999999999", "1e15", "123456789012", "0.00000001", "7", "250"]
for name, src, dst, seed, with_force in (("coin01", 0, 1, 4321, False), ("coin10", 1, 0, 4322, False),
                                         ("coin11", 1, 1, 4323, True), ("coin00", 0, 0, 4324, True)):
    case = {"steps": [{"call": "CalcCoin", "args": [src, dst] + (["$force"] if with_force else [])}],
            "forms": {"frmMain": {"optCoinTop": "$top", "optCoinBottom": "$bot",
                                  "txtCoin": {"$array": {"0": {"Text": "$t0"}, "1": {"Text": "$t1"}}}}},
            "read": {"top": "%c.txtCoin(0).Text", "bottom": "%c.txtCoin(1).Text", "cur": "%g.eCurrentCoin"}}
    samples.append({"name": name, "seed": seed, "count": 36,
                    "draw": {"top": {"choice": [opts(5, k) for k in (0, 1, 2, 3, 4, -1)]},
                             "bot": {"choice": [opts(5, k) for k in (0, 1, 2, 3, 4, -1)]},
                             "t0": {"choice": CT}, "t1": {"choice": CT},
                             "force": {"choice": [0, 0, 1, 10, 100, 10000, 1000000, 5, -10]}},
                    "case": case})
# ConvertCoin on its own: every pair of coins, a few amounts
conv = []
for amount in ("1", "250", "12345.678", "0.000001", "99999999999"):
    for a in COINS:
        for b in COINS:
            conv.append((amount, a, b))
for amount, a, b in conv[::3]:
    cases.append({"id": "convert-%s-%d-%d" % (amount, a, b), "proc": "ConvertCoin", "args": [float(amount), a, b], "read": {}})
cases.append({"id": "convert-zero-to", "proc": "ConvertCoin", "args": [5.0, 10, 0], "read": {}})
cases.append({"id": "convert-zero-from", "proc": "ConvertCoin", "args": [5.0, 0, 10], "read": {}})
cases.append({"id": "convert-neg", "proc": "ConvertCoin", "args": [-5.5, 100, 1], "read": {}})

# ---------------------------------------------------------------------------------- exp
EXP_READ = {"table": "%c.txtCalcEXPTable.Text"}
cls_pick = [None, 1, 2, 5, 8, 9, 15, 9001, 9002, 9003, 99]
race_pick = [None, 1, 2, 8, 12, 13, 9001, 9002, 9003, 99]


def combo(n, li=None):
    if n is None:
        return {"ListIndex": 0, "ItemData": [0, 0]}
    return {"ListIndex": 1 if li is None else li, "ItemData": [0, n, n + 7]}


for c in cls_pick:
    for r in race_pick:
        cases.append({"id": "exp-%s-%s" % (c, r), "steps": [{"call": "CalcExp"}], "read": EXP_READ,
                      "forms": {"frmMain": {"cmbClass": combo(c), "cmbRace": combo(r), "txtCalcEXPTable": {"Text": "stale"}}}})
cases.append({"id": "exp-li2", "steps": [{"call": "CalcExp"}], "read": EXP_READ,
              "forms": {"frmMain": {"cmbClass": combo(3, 2), "cmbRace": combo(4, 2)}}})
cases.append({"id": "exp-neg", "steps": [{"call": "CalcExp"}], "read": EXP_READ,
              "forms": {"frmMain": {"cmbClass": {"ListIndex": -1}, "cmbRace": {"ListIndex": -1}}}})

# ---------------------------------------------------------------------------------- scenario
FRM = [
    {"file": "frmSwingCalc.frm", "procs": ["CalcSwings"]},
    {"file": "frmBSCalc.frm", "decls": ["bDontRefresh"], "procs": ["CalcBS"]},
    {"file": "frmCoinConvert.frm", "enums": ["eCoins"], "decls": ["eCurrentCoin"], "procs": ["CalcCoin", "ConvertCoin"]},
    {"file": "frmExpCalc.frm", "procs": ["CalcExp"]},
]
scn = {
    "family": "calculators",
    "description": "Calculator cores (FAM-27): frmSwingCalc CalcSwings, frmBSCalc CalcBS, frmCoinConvert CalcCoin and "
                   "ConvertCoin, frmExpCalc CalcExp. Stock path only (bGreaterMUD false).",
    "notes": [
        "The four procedures are cut out of their .frm files (scenario key frm, a list of forms). Their controls all "
        "resolve to the frmMain shim, so a case sets the calculator's controls under forms.frmMain; combo boxes hold "
        "the selected row in ListIndex and ItemData, radio buttons are Value true/false in an $array.",
        "Outputs are the label and text box contents the procedure leaves behind; a control it did not write reads as "
        "the empty string. A VB error that escapes (CalcSwings has no handler) is the case's error field; one that MME "
        "handles is in events.",
        "The 'table' case carries the Items, Classes and Races rows used (the Rust port never reads SQLite). 'sn-*' and "
        "'pe-*' cases record GetSpellName and PullSpellEQ for the weapons' cast abilities (the port's CastText).",
        "In swingfilt cases chkGlobalFilter is 1, so CalcSwings runs MME's PopulateCharacterProfile over the panel "
        "values in lblInvenCharStat/txtCharStats Tags; the Rust test builds the profile from the same values.",
        "Rows numbered 9001 and up are invented (db_extra) to reach branches no stock row has."],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Classes": EXTRA_CLASSES, "Races": EXTRA_RACES},
    "tolerance": {"default": 0},
    "globals": {"bGreaterMUD": False, "gAvgLevelMaxAllStats": 262.3},
    "forms": {"frmMain": {
        "lblDatVer": {"Caption": "v1.11p"},
        "lblEU": arr(10), "txtSwing": arr(10), "lblSwingDamage": arr(2), "txtTrueAVG": arr(8), "optSpeed": arr(4),
        "optCoinTop": arr(5), "optCoinBottom": arr(5), "txtCoin": arr(2),
        "lblInvenCharStat": arr(47), "txtCharStats": arr(6), "txtGlobalLevel": arr(1), "cmbGlobalClass": arr(1),
        "cmbGlobalRace": arr(1), "optMonsterFilter": arr(2), "txtMonsterLairFilter": arr(9), "txtCharManaRegen": arr(1)}},
    "cases": cases, "samples": samples,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d explicit cases, %d samples" % (len(cases), sum(s["count"] for s in samples)), file=sys.stderr)

"""Writes fixtures/scenarios/hit_calc.json (FAM-26, the Hit Calculator window):
    python3 -I tools/oracle/gen/hit_calc.py fixtures/scenarios/hit_calc.json

DoHitCalc, SetHitCalcVals, GetMonsterData, GetVileWardValue and ModifyHitCalcValues are procedures of
frmHitCalc; the scenario's `frm` key cuts them (and the form's module variables) out of the .frm file and
the form's controls resolve to the frmMain shim. The first case, "table", carries the Items, Classes and
Monsters rows the cases look up, in compact form. Bulk cases are seeded samples (splitmix64 in the oracle),
so the scenario stays small. Deterministic."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
monsters = {r["Number"]: dict(r) for r in con.execute("select * from Monsters order by Number")}


def iab(row, n=20):
    return [(row["Abil-%d" % i], row["AbilVal-%d" % i]) for i in range(n)]


def mab(row):
    return [(row["Abil-%d" % i], row["AbilVal-%d" % i]) for i in range(10)]


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
    item(9001, "tst both", abil=[(22, 7), (116, 5)]),
    item(9002, "tst negatives", abil=[(22, -6), (116, -4)]),
    item(9003, "tst accy only", abil=[(22, 12)]),
    item(9004, "tst bs only", abil=[(116, 9)]),
    item(9005, "tst twice", abil=[(22, 3), (22, 4), (116, 2), (116, 8)]),
    item(9006, "tst strreq big", StrReq=40000, abil=[(116, 3)]),
    item(9007, "tst big values", abil=[(22, 30000), (116, 30000)]),
    item(9008, "tst last slot", abil=[(0, 0)] * 19 + [(116, 6)]),
    item(9009, "tst strreq", StrReq=75, abil=[(22, 1)]),
    item(9010, "tst plain", ItemType=0, abil=[]),
]
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          **{"Encum": 0, "Accy": 0}, **r}


def mon(n, name, ac=0, bsd=0, align=2, abil=(), att=()):
    r = {"Number": n, "Name": name, "ArmourClass": ac, "BSDefense": bsd, "Align": align}
    for i, (a, v) in enumerate(abil):
        r["Abil-%d" % i] = a
        r["AbilVal-%d" % i] = v
    for i, (t, acc, pct) in enumerate(att):
        r["AttType-%d" % i] = t
        r["AttAcc-%d" % i] = acc
        r["Att%%-%d" % i] = pct
    return r


EXTRA_MONSTERS = [
    mon(9001, "tst dodge zero", ac=40, bsd=20, abil=[(34, 0)], att=[(1, 30, 100)]),
    mon(9002, "tst dodge twice", ac=60, abil=[(34, 15), (34, 25)], att=[(1, 50, 100)]),
    mon(9003, "tst see hidden", ac=10, abil=[(57, 0)], align=1, att=[(3, 70, 50)]),
    mon(9004, "tst dodge neg then sh", ac=0, abil=[(34, -4), (57, 0), (34, 9)], align=5),
    mon(9005, "tst attacks mixed", ac=25, align=6,
        att=[(2, 99, 100), (1, 10, 0), (3, 45, 20), (4, 200, 100), (1, 33, 100)]),
    mon(9006, "tst attack best last", ac=5, align=0, att=[(1, 5, 10), (1, 15, 10), (1, 25, 10), (3, 35, 10), (3, 60, 10)]),
    mon(9007, "tst big", ac=32000, bsd=32000, align=3, abil=[(34, 32000)], att=[(1, 32000, 100)]),
    mon(9008, "tst neg", ac=-5, bsd=-9, align=4, att=[(1, -7, 100)]),
    mon(9009, "tst last abil", abil=[(0, 0)] * 9 + [(34, 44)], align=7),
    mon(9010, "tst last att", att=[(0, 0, 0)] * 4 + [(1, 77, 100)], align=-1),
]
for r in EXTRA_MONSTERS:
    monsters[r["Number"]] = {**{k: 0 for k in monsters[1]}, **{"Name": ""}, **r}

# ---------------------------------------------------------------------------------- pools
real_mon = sorted(n for n in monsters if n < 9000)
dodgers = [n for n in real_mon if any(a == 34 and v > 0 for a, v in mab(monsters[n]))]
hiders = [n for n in real_mon if any(a == 57 for a, v in mab(monsters[n]))]
bsdef = [n for n in real_mon if monsters[n]["BSDefense"]]
by_align = {a: [n for n in real_mon if monsters[n]["Align"] == a] for a in range(7)}
no_att = [n for n in real_mon if not any(0 < monsters[n]["AttType-%d" % i] <= 3 for i in range(5))]
odd_att = [n for n in real_mon if any(monsters[n]["AttType-%d" % i] > 3 for i in range(5))]
by_ac = sorted(real_mon, key=lambda n: (monsters[n]["ArmourClass"], n))
by_acc = sorted(real_mon, key=lambda n: (max(monsters[n]["AttAcc-%d" % i] for i in range(5)), n))
mon_pick = sorted(set(dodgers[::31][:5] + hiders[::37][:4] + bsdef[::97][:5]
                      + [x for a in range(7) for x in by_align[a][:2]] + no_att[:2] + odd_att[:2]
                      + by_ac[::150][:8] + by_ac[-3:] + by_acc[-3:] + [real_mon[0], real_mon[-1]]))
mon_pick = mon_pick[:40]
mon_all = mon_pick + [n for n in monsters if n >= 9000]
MON_ODD = [99999, 0, -3]

weapons = [n for n, r in items.items() if r["ItemType"] == 1 and n < 9000]
with22 = [n for n in weapons if any(a == 22 and v != 0 for a, v in iab(items[n]))]
with116 = [n for n in weapons if any(a == 116 and v != 0 for a, v in iab(items[n]))]
both = [n for n in with22 if n in with116]
strreq = sorted(weapons, key=lambda n: (-items[n]["StrReq"], n))
bs_pool = sorted(set(with22[::5][:6] + with116[::9][:8] + both[:4] + strreq[:3] + weapons[::97][:4]
                     + [n for n in items if 9001 <= n <= 9010]))
BS_ODD = [99999, -2]
table_class = sorted(classes)
stealth_classes = [n for n in classes if any(classes[n]["Abil-%d" % i] == 103 for i in range(10))]


def flat(pairs):
    f = [x for pair in pairs for x in pair]
    while f and f[-1] == 0:
        f.pop()
    return f


MON_FIELDS = ["Number", "ArmourClass", "BSDefense", "Align", "Abilities-0..9 flat", "AttType/AttAcc/Att% x5 flat"]
cases = [{"id": "table",
          "items": [[n, items[n]["StrReq"], flat(iab(items[n]))] for n in bs_pool],
          "item_fields": ["Number", "StrReq", "AbilitiesFlat"],
          "classes": [[n, [classes[n]["Abil-%d" % i] for i in range(10)]] for n in table_class],
          "class_fields": ["Number", "Abil-0..9"],
          "monsters": [[n, monsters[n]["ArmourClass"], monsters[n]["BSDefense"], monsters[n]["Align"], flat(mab(monsters[n])),
                        [x for i in range(5) for x in (monsters[n]["AttType-%d" % i], monsters[n]["AttAcc-%d" % i],
                                                      monsters[n]["Att%%-%d" % i])]]
                       for n in mon_all],
          "monster_fields": MON_FIELDS}]


def arr(n, **kw):
    return {"$array": {str(i): dict(kw) for i in range(n)}}


def opts(n, on):
    """optHitCalcType / optAttacker / optDefender: radio buttons, exactly one (or none) selected."""
    return {"$array": {str(i): {"Value": (i == on)} for i in range(n)}}


# ---------------------------------------------------------------------------------- reads
FORM_READ = {
    "txt": "%c.txtHitCalc.Text[0..5]", "en": "%c.txtHitCalc.Enabled[3..5]",
    "shadow": "%c.chkShadow.Value", "shadow_en": "%c.chkShadow.Enabled",
    "hidden": "%c.chkSeeHidden.Value", "hidden_en": "%c.chkSeeHidden.Enabled",
    "evil": "%c.cmbEvil.ListIndex", "evil_en": "%c.cmbEvil.Enabled", "mlist_en": "%c.cmbMonsterList.Enabled",
    "bs": "%c.lblBSPercep.Caption", "main": "%c.lblMain.Caption", "left": "%c.lblSubLeft.Caption",
    "right": "%c.lblSubRight.Caption", "bold": "%c.lblSubRight.FontBold",
}
STATE_NAMES = ["nDefenderAC", "nDefenderDodge", "nDefenderPerception", "nDefenderProtEvil", "nDefenderVileWard",
               "nDefenderEvilness", "bDefenderShadow", "nCharPerception"]
MON_NAMES = ["nMonsterAccy", "nMonsterAC", "nMonsterDodge", "nMonsterBSdef", "bMonsterEvil", "bMonsterSeeHidden"]
READ = {**FORM_READ, **{n: "%g." + n for n in STATE_NAMES}}
PICK_READ = {**READ, **{n: "%g." + n for n in MON_NAMES}}
GRID_READ = {k: FORM_READ[k] for k in ("main", "right", "bold")}

samples, grids = [], []

# ---------------------------------------------------------------------------------- GetVileWardValue
for vw in (-5, 0, 1, 2, 3, 10, 99, 101, 5000):
    for ev in (-1, 0, 1, 2, 3, 7):
        cases.append({"id": "vw-%d-%d" % (vw, ev), "proc": "GetVileWardValue", "args": [vw, ev]})

# ---------------------------------------------------------------------------------- GetMonsterData
MON_READ = {n: "%g." + n for n in MON_NAMES}
for n in mon_all + MON_ODD:
    for ver in (1.83, 1.82):
        if ver == 1.82 and n not in mon_pick[::5] and n not in (9001, 9007, 99999):
            continue
        cases.append({"id": "mon-%d-%s" % (n, ver), "proc": "GetMonsterData", "args": [n], "read": MON_READ,
                      "globals": {"nNMRVer": ver}})

# ---------------------------------------------------------------------------------- ModifyHitCalcValues
for idx in range(12):
    for txt in ("5", "", "abc", "-3", "2.7", "9999", "1e10", "-0.4", "123456789012"):
        cases.append({"id": "mod-%d-%s" % (idx, txt or "empty"),
                      "steps": [{"call": "ModifyHitCalcValues", "args": [idx]}],
                      "read": {"txt": "%c.txtHitCalc.Text[0..5]"},
                      "forms": {"frmMain": {"txtHitCalc": {"$array": {str(i): {"Text": txt} for i in range(6)}}}}})

# ---------------------------------------------------------------------------------- DoHitCalc
NUMS = ["0", "1", "5", "8", "9", "20", "25", "40", "50", "60", "75", "100", "140", "200", "500", "1000", "9999", "10000",
        "99999", "-5", "-999", "-1000", "2.5", "", "abc", "1e10", "3000000000"]
SANE = ["0", "10", "25", "40", "60", "75", "100", "140", "200", "350"]
TY = [opts(2, 0)] * 3 + [opts(2, 1)] * 3 + [opts(2, -1)]
AT = [opts(3, 0), opts(3, 1), opts(3, 2), opts(3, 2), opts(3, -1)]
DF = [opts(4, 0), opts(4, 1), opts(4, 2), opts(4, 3), opts(4, 3), opts(4, -1)]
BOOL = [True, False]
CLS = [1, 2, 5, 9, 15, 0, 32767, 32768, -40000, 70000]
ST = {n: {"choice": [0, 0, 0, 5, 40, 120, -3]} for n in STATE_NAMES}
for n in ("bDefenderShadow",):
    ST[n] = {"choice": [False, True]}
ST["nDefenderEvilness"] = {"choice": [0, 0, 1, 2, 3]}
ST["nCharPerception"] = {"choice": [0, 20, 80, 150, 10000]}


def txt_array(enabled=True):
    d = {"0": {"Text": "$t0"}, "1": {"Text": "$t1"}, "2": {"Text": "$t2"}}
    for i in (3, 4, 5):
        d[str(i)] = {"Text": "$t%d" % i, "Enabled": "$e%d" % i} if enabled else {"Text": "$t%d" % i}
    return {"$array": d}


dh_draw = {"ty": {"choice": TY}, "df": {"choice": DF}, "cli": {"choice": [0, 0, 0, 0, -1]}, "cls": {"choice": CLS},
           "s": {"choice": [0, 1]}, "se": {"choice": BOOL}, "h": {"choice": [0, 1]}, "he": {"choice": BOOL},
           "e3": {"choice": BOOL}, "e4": {"choice": BOOL}, "e5": {"choice": BOOL}, "evil": {"choice": [0, 1, 2]},
           "ee": {"choice": BOOL}}
for i in range(6):
    dh_draw["t%d" % i] = {"choice": NUMS}
dh_draw.update(ST)
dh_case = {"steps": [{"call": "DoHitCalc"}], "read": READ,
           "forms": {"frmMain": {"txtHitCalc": txt_array(), "optHitCalcType": "$ty", "optDefender": "$df",
                                 "chkShadow": {"Value": "$s", "Enabled": "$se"},
                                 "chkSeeHidden": {"Value": "$h", "Enabled": "$he"},
                                 "cmbEvil": {"ListIndex": "$evil", "Enabled": "$ee"},
                                 "lblMain": {"Caption": "old"},
                                 "cmbGlobalClass": {"$array": {"0": {"ListIndex": "$cli", "ItemData": ["$cls"],
                                                                     "StrictItemData": True}}}}},
           "globals": {n: "$" + n for n in STATE_NAMES}}
samples.append({"name": "dohit", "seed": 4201, "count": 80, "draw": dh_draw, "case": dh_case})
sane = dict(dh_draw)
for i in range(6):
    sane["t%d" % i] = {"choice": SANE}
sane.update({"cli": {"choice": [0]}, "cls": {"choice": [1, 2, 5, 9, 15]}, "e3": {"choice": [True, True, False]},
             "e5": {"choice": [True, True, False]}})
samples.append({"name": "dohitok", "seed": 4202, "count": 40, "draw": sane, "case": dh_case})
# grid: accuracy x AC x dodge, normal and backstab, against a manual defender
for kind, ty in (("n", opts(2, 0)), ("b", opts(2, 1))):
    grids.append({"name": "dhg" + kind,
                  "axes": {"acc": ["1", "9", "50", "140", "9999", "10000"],
                           "ac": ["0", "100", "400", "9999", "-8"],
                           "dg": ["0", "15", "-30"]},
                  "case": {"steps": [{"call": "DoHitCalc"}], "read": GRID_READ,
                           "forms": {"frmMain": {"txtHitCalc": {"$array": {
                               "0": {"Text": "$acc"}, "1": {"Text": "$ac"}, "2": {"Text": "$dg"},
                               "3": {"Text": "10", "Enabled": True}, "4": {"Text": "0", "Enabled": False},
                               "5": {"Text": "35", "Enabled": True}}},
                               "optHitCalcType": ty, "optDefender": opts(4, 3),
                               "chkShadow": {"Value": 1, "Enabled": True}, "chkSeeHidden": {"Value": 0, "Enabled": True},
                               "cmbGlobalClass": {"$array": {"0": {"ListIndex": 0, "ItemData": [1]}}}}}}})

# ---------------------------------------------------------------------------------- SetHitCalcVals
TAGS = ["0", "30", "75.5", "120", "260", "-10", "", "abc", "99999", "40000", "32767.5", "32768", "8", "2.5"]
ACTAG = ["0", "10", "45", "60.7", "150", "-20", "9999", "3000000000", "", "35"]
STLTAG = ["0", "30", "80", "150", "-20", "40000", "32767.5", "32768", "-32768.5", "64", "100"]
LVL = ["0", "1", "30", "100", "255", "", "40000", "abc", "12.5"]
ADJ = [0, 0, 5, -7, 20, 100, -100]
SH = [0, 0, 10, 10, 3]
WN = bs_pool + [0, 0, 99999]
sv_draw = {"ty": {"choice": TY}, "at": {"choice": AT}, "df": {"choice": DF}, "cli": {"choice": [0, 0, 0, 0, 0, -1]},
           "cls": {"choice": [1, 2, 5, 6, 7, 8, 9, 10, 15, 0, 32768]},
           "s": {"choice": [0, 1]}, "se": {"choice": BOOL}, "h": {"choice": [0, 1]}, "he": {"choice": BOOL},
           "e3": {"choice": BOOL}, "e4": {"choice": BOOL}, "e5": {"choice": BOOL}, "evil": {"choice": [0, 1, 2]},
           "ee": {"choice": BOOL}, "me": {"choice": BOOL},
           "acc": {"choice": TAGS}, "ac": {"choice": ACTAG}, "dg": {"choice": TAGS}, "pe": {"choice": TAGS},
           "stl": {"choice": STLTAG}, "bsa": {"choice": STLTAG}, "agi": {"choice": STLTAG}, "str": {"choice": STLTAG},
           "lvl": {"choice": LVL}, "shac": {"choice": SH},
           "aab": {"choice": ADJ + [40000, -40000]}, "aot": {"choice": ADJ}, "ait": {"choice": ADJ},
           "wn": {"choice": WN}, "wa": {"choice": ADJ + [40000, -33000]}, "wb": {"choice": ADJ + [40000, -33000]},
           "atype": {"choice": [0, 1, 2, 3, 4, 4, 5, 6, 6, 7, 7]}, "adj": {"choice": [0, -10, -3, 5, 32767]},
           "ma": {"choice": [0, 1, 2, 3, 4]}, "bsb": {"choice": BOOL}, "bsw": {"choice": WN},
           "ar": {"choice": [[], [], [], [True, False], [False, True], [True, True]]}}
for i in range(6):
    sv_draw["t%d" % i] = {"choice": SANE + ["", "abc", "-5", "99999"]}
sv_draw.update(ST)
for n in MON_NAMES:
    sv_draw[n] = {"choice": {"bMonsterEvil": BOOL, "bMonsterSeeHidden": BOOL}.get(n, [0, 0, 10, 25, 60, 100, -4])}
sv_case = {"steps": [{"call": "SetHitCalcVals", "args": "$ar"}], "read": READ,
           "forms": {"frmMain": {
               "txtHitCalc": txt_array(),
               "optHitCalcType": "$ty", "optAttacker": "$at", "optDefender": "$df",
               "chkShadow": {"Value": "$s", "Enabled": "$se"}, "chkSeeHidden": {"Value": "$h", "Enabled": "$he"},
               "cmbEvil": {"ListIndex": "$evil", "Enabled": "$ee"}, "cmbMonsterList": {"Enabled": "$me"},
               "lblMain": {"Caption": "oldmain"}, "lblSubLeft": {"Caption": "oldleft"},
               "lblSubRight": {"Caption": "oldright"}, "lblBSPercep": {"Caption": "oldbs"},
               "lblInvenCharStat": {"$array": {"2": {"Tag": "$ac"}, "8": {"Tag": "$dg"}, "10": {"Tag": "$acc"},
                                               "13": {"Tag": "$bsa"}, "19": {"Tag": "$stl"}, "20": {"Tag": "$pe"}}},
               "txtCharStats": {"$array": {"0": {"Tag": "$str"}, "3": {"Tag": "$agi"}}},
               "txtGlobalLevel": {"$array": {"0": {"Text": "$lvl"}}},
               "cmbGlobalClass": {"$array": {"0": {"ListIndex": "$cli", "ItemData": ["$cls"], "StrictItemData": True}}}}},
           "globals": {**{n: "$" + n for n in STATE_NAMES + MON_NAMES},
                       "nGlobalCharShadowAC": "$shac", "nGlobalCharAccyAbils": "$aab", "nGlobalCharAccyOther": "$aot",
                       "nGlobalCharAccyItems": "$ait", "nGlobalCharWeaponNumber": ["$wn", 0],
                       "nGlobalCharWeaponAccy": ["$wa", 0], "nGlobalCharWeaponBSaccy": ["$wb", 0],
                       "nGlobalAttackTypeMME": "$atype", "nGlobalAttackAccyAdj": "$adj", "nGlobalAttackMA": "$ma",
                       "bGlobalAttackBackstab": "$bsb", "nGlobalAttackBackstabWeapon": "$bsw"}}
samples.append({"name": "setv", "seed": 4101, "count": 80, "draw": sv_draw, "case": sv_case})
# sensible values, so most draws get through to the end
ok = dict(sv_draw)
ok.update({"acc": {"choice": ["40", "75.5", "120", "200"]}, "ac": {"choice": ["0", "25", "60", "150"]},
           "dg": {"choice": ["0", "20", "60"]}, "pe": {"choice": ["0", "15", "40"]},
           "stl": {"choice": ["0", "30", "80", "150"]}, "bsa": {"choice": ["0", "4", "-6", "12"]},
           "agi": {"choice": ["40", "90", "150"]}, "str": {"choice": ["40", "90", "150"]},
           "lvl": {"choice": ["1", "30", "100"]}, "cli": {"choice": [0]}, "cls": {"choice": [1, 2, 6, 7, 8, 9, 15]},
           "aab": {"choice": ADJ}, "wa": {"choice": ADJ}, "wb": {"choice": ADJ}, "ar": {"choice": [[]]},
           "ty": {"choice": [opts(2, 0), opts(2, 1)]}, "at": {"choice": [opts(3, 0), opts(3, 1), opts(3, 2)]},
           "df": {"choice": [opts(4, 0), opts(4, 1), opts(4, 2), opts(4, 3)]}})
samples.append({"name": "setok", "seed": 4102, "count": 35, "draw": ok, "case": sv_case})

# backstab with the current character attacking: the CalculateBackstabAccuracy arguments and their conversions
bs = dict(ok)
bs.update({"ty": {"choice": [opts(2, 1)]}, "at": {"choice": [opts(3, 0)]},
           "stl": {"choice": STLTAG}, "bsa": {"choice": STLTAG}, "agi": {"choice": STLTAG}, "str": {"choice": STLTAG},
           "lvl": {"choice": LVL}, "cli": {"choice": [0, 0, 0, -1]}, "aab": {"choice": ADJ + [40000, -40000, 2000000000]},
           "aot": {"choice": ADJ + [2000000000]}, "wa": {"choice": ADJ + [40000, -33000]},
           "wb": {"choice": ADJ + [40000, -33000]}, "wn": {"choice": WN}, "bsb": {"choice": BOOL}, "bsw": {"choice": WN}})
samples.append({"name": "setbs", "seed": 4104, "count": 50, "draw": bs, "case": sv_case})

# normal attack by the character: bash, smash, kick and jumpkick give up accuracy
ac_ = dict(ok)
ac_.update({"ty": {"choice": [opts(2, 0)]}, "at": {"choice": [opts(3, 0), opts(3, 0), opts(3, 1)]},
            "atype": {"choice": [1, 4, 4, 4, 6, 7, 2]}, "ma": {"choice": [1, 2, 3, 2, 3]},
            "adj": {"choice": [-10, -3, 0, 5, -25, 32767]}})
samples.append({"name": "setacc", "seed": 4105, "count": 30, "draw": ac_, "case": sv_case})
bo = dict(bs)
bo.update({"stl": {"choice": ["0", "30", "80", "150", "-20"]}, "bsa": {"choice": ["0", "4", "-6", "12", "7"]},
           "agi": {"choice": ["40", "90", "150", "255"]}, "str": {"choice": ["40", "90", "150"]},
           "lvl": {"choice": ["1", "30", "100"]}, "cli": {"choice": [0]}, "cls": {"choice": [1, 2, 6, 7, 8, 9, 10, 15]},
           "aab": {"choice": ADJ}, "aot": {"choice": ADJ}, "wa": {"choice": ADJ}, "wb": {"choice": ADJ},
           "bsb": {"choice": [True, True, False]}})
samples.append({"name": "setbsok", "seed": 4106, "count": 30, "draw": bo, "case": sv_case})

# a monster picked in the drop-down: GetMonsterData, then SetHitCalcVals(False, True)
pk_case = {"steps": [{"call": "GetMonsterData", "args": ["$mn"], "as": "found"},
                     {"call": "SetHitCalcVals", "args": [False, True]}],
           "read": PICK_READ,
           "forms": {"frmMain": {
               "txtHitCalc": txt_array(), "optHitCalcType": "$ty", "optAttacker": "$at", "optDefender": "$df",
               "chkShadow": {"Value": "$s", "Enabled": "$se"}, "chkSeeHidden": {"Value": "$h", "Enabled": "$he"},
               "cmbEvil": {"ListIndex": "$evil", "Enabled": "$ee"}, "cmbMonsterList": {"Enabled": "$me"},
               "lblInvenCharStat": {"$array": {"10": {"Tag": "$acc"}}},
               "cmbGlobalClass": {"$array": {"0": {"ListIndex": 0, "ItemData": [1]}}}}},
           "globals": {**{n: "$" + n for n in STATE_NAMES}, "nGlobalAttackTypeMME": 1}}
pk = dict(ok)
pk["mn"] = {"choice": mon_all}
pk["ty"] = {"choice": [opts(2, 0), opts(2, 1)]}
pk["at"] = {"choice": [opts(3, 0), opts(3, 1), opts(3, 2)]}
pk["df"] = {"choice": [opts(4, 0), opts(4, 1), opts(4, 1), opts(4, 2), opts(4, 3)]}
samples.append({"name": "pick", "seed": 4103, "count": 35, "draw": pk, "case": pk_case})

# ---------------------------------------------------------------------------------- scenario
FRM = [{"file": "frmHitCalc.frm",
        "decls": MON_NAMES + STATE_NAMES + ["bDontRefresh"],
        "procs": ["DoHitCalc", "SetHitCalcVals", "GetMonsterData", "GetVileWardValue", "ModifyHitCalcValues"]}]
scn = {
    "family": "hit_calc",
    "description": "Hit calculator core (FAM-26): frmHitCalc DoHitCalc, SetHitCalcVals, GetMonsterData, "
                   "GetVileWardValue, ModifyHitCalcValues. Stock path only (bGreaterMUD false).",
    "notes": [],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Monsters": EXTRA_MONSTERS},
    "tolerance": {"default": 0},
    "globals": {"bGreaterMUD": False, "nNMRVer": 1.83},
    "forms": {"frmMain": {
        "txtHitCalc": arr(6), "optHitCalcType": arr(2), "optAttacker": arr(3), "optDefender": arr(4),
        "lblInvenCharStat": arr(47), "txtCharStats": arr(6), "txtGlobalLevel": arr(1), "cmbGlobalClass": arr(1)}},
    "cases": cases, "samples": samples, "grids": grids,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d explicit cases, %d samples" % (len(cases), sum(s["count"] for s in samples)), file=sys.stderr)

"""Writes fixtures/scenarios/stats.json: python3 -I tools/oracle/gen/stats.py fixtures/scenarios/stats.json
Reads the stock SQLite for the Races/Classes rows that cases carry as inputs. Deterministic."""
import json, sqlite3, glob, os, sys
out = sys.argv[1]
p = glob.glob(os.path.expanduser('~/.cache/touchstone/mdb/eba20f06*.sqlite'))[0]
c = sqlite3.connect(p); c.row_factory = sqlite3.Row

def abil(r):
    return [[r['Abil-%d' % i], r['AbilVal-%d' % i]] for i in range(10)]

races = {}
for r in c.execute("select * from Races order by Number"):
    races[r['Number']] = {"number": r['Number'], "hp_per_lvl": r['HPPerLVL'], "base_cp": r['BaseCP'],
        "m_str": r['mSTR'], "m_int": r['mINT'], "m_wil": r['mWIL'], "m_agl": r['mAGL'], "m_hea": r['mHEA'], "m_chm": r['mCHM'],
        "x_str": r['xSTR'], "x_int": r['xINT'], "x_wil": r['xWIL'], "x_agl": r['xAGL'], "x_hea": r['xHEA'], "x_chm": r['xCHM'],
        "abilities": abil(r)}
classes = {}
for r in c.execute("select * from Classes order by Number"):
    classes[r['Number']] = {"number": r['Number'], "min_hits": r['MinHits'], "max_hits": r['MaxHits'],
        "magery_type": r['MageryType'], "magery_lvl": r['MageryLVL'], "armour_type": r['ArmourType'],
        "combat_lvl": r['CombatLVL'], "abilities": abil(r)}

cases = []
def add(id_, **kw):
    d = {"id": id_}; d.update(kw); cases.append(d)

# --- recordset lookups
race_fns = {"GetRaceHPBonus": "race", "GetRaceCP": "race", "GetRaceStealth": "race"}
class_fns = ["GetClassMaxHP", "GetClassMinHP", "GetClassMageryLVL", "GetClassMagery", "GetClassArmourType", "GetClassCombat", "GetClassStealth"]
rnums = [0] + list(range(1, 14)) + [14, 99, -1, 5000]
cnums = [0] + list(range(1, 16)) + [16, 99, -1, 5000]
for fn in race_fns:
    for n in rnums:
        if fn == "GetRaceStealth" and n == 0:
            continue
        add("%s-%d" % (fn, n), proc=fn, args=[n], race=races.get(n))
for fn in class_fns:
    for n in cnums:
        if fn == "GetClassStealth" and n == 0:
            continue
        add("%s-%d" % (fn, n), proc=fn, args=[n], **{"class": classes.get(n)})
# stealth selected from the global race/class combo boxes
def combo(ids, idx):
    return {"$array": {"0": {"ItemData": ids, "List": ["x%d" % i for i in ids], "ListIndex": idx}}}
for chk in (0, 1):
    for idx in [-1, 0, 3, 4, 6, 8, 11, 12, 13, 20]:
        ids = list(range(1, 14))
        sel = ids[idx] if 0 <= idx < len(ids) else 0
        add("GetRaceStealth-sel-%d-%d" % (chk, idx), proc="GetRaceStealth", args=[],
            forms={"frmMain": {"chkGlobalFilter": {"Value": chk}, "cmbGlobalRace": combo(ids, idx)}},
            race=races.get(sel if chk else 0), selected={"chk": chk, "item_data": ids, "list_index": idx})
        ids = list(range(1, 16))
        sel = ids[idx] if 0 <= idx < len(ids) else 0
        add("GetClassStealth-sel-%d-%d" % (chk, idx), proc="GetClassStealth", args=[],
            forms={"frmMain": {"chkGlobalFilter": {"Value": chk}, "cmbGlobalClass": combo(ids, idx)}},
            **{"class": classes.get(sel if chk else 0), "selected": {"chk": chk, "item_data": ids, "list_index": idx}})
# empty combos with the filter on, and a non-zero argument that wins over the combo
add("GetRaceStealth-empty", proc="GetRaceStealth", args=[], forms={"frmMain": {"chkGlobalFilter": {"Value": 1}}},
    race=None, selected={"chk": 1, "item_data": [], "list_index": -1})
add("GetClassStealth-empty", proc="GetClassStealth", args=[], forms={"frmMain": {"chkGlobalFilter": {"Value": 1}}},
    **{"class": None, "selected": {"chk": 1, "item_data": [], "list_index": -1}})
add("GetRaceStealth-argwins", proc="GetRaceStealth", args=[4],
    forms={"frmMain": {"chkGlobalFilter": {"Value": 1}, "cmbGlobalRace": combo(list(range(1, 14)), 0)}}, race=races[4])
add("GetClassStealth-argwins", proc="GetClassStealth", args=[8],
    forms={"frmMain": {"chkGlobalFilter": {"Value": 1}, "cmbGlobalClass": combo(list(range(1, 16)), 0)}}, **{"class": classes[8]})

# --- whole-table walk
add("ComputeAvgLevelMaxStats", steps=[{"call": "ComputeAvgLevelMaxStats"}],
    read={"all": "%g.gAvgLevelMaxAllStats", "single": "%g.gAvgLevelMaxSingleStat"},
    races=list(races.values()))

# --- creation points
for d in [-5, 0, 1, 5, 9, 10, 11, 19, 20, 25, 49, 50, 89, 90, 91, 99, 100, 101, 105, 110, 150, 200, 500, 1000, 5000,
          100000, 2000000, 214748300, 214748400, 300000000, 2147483647]:
    add("CP_CostForRaise-%d" % d, proc="CP_CostForRaise", args=[d])
for base in [-50, 0, 1, 100, 250]:
    for need in [-5, 0, 1, 99, 100, 101, 150, 500, 1000, 2000, 10000, 50000, 500000, 1000000, 2000000]:
        add("LevelNeededForCP-%d-%d" % (base, need), proc="LevelNeededForCP", args=[base, need])
for lv in [-5, 0, 1, 2, 3, 9, 10, 11, 12, 20, 21, 50, 99, 100, 101, 255, 1000, 2000]:
    add("CalcCPLevel-%d" % lv, proc="CalcCPLevel", args=[lv])

# --- CalcEncum
for ver in ["v1.11i", "V1.11I", "v1.11p", "", "x1.11i "]:
    for s in [-32768, -5, 0, 1, 50, 100, 101, 255, 682, 683, 32767]:
        for b in [0, 10, 100, -5, 32767]:
            add("CalcEncum-%s-%d-%d" % (ver or "empty", s, b), forms={"frmMain": {"lblDatVer": {"Caption": ver}}},
                proc="CalcEncum", args=[s, b])
    add("CalcEncum-noarg-%s" % (ver or "empty"), forms={"frmMain": {"lblDatVer": {"Caption": ver}}}, proc="CalcEncum", args=[90])

# --- magery names
for n in [-32768, -1, 0, 1, 2, 3, 4, 5, 6, 99, 32767]:
    add("GetMageryEnum-%d-opt" % n, proc="GetMageryEnum", args=[n])
    for lv in [0, 1, 3, -2, 32767]:
        add("GetMageryEnum-%d-%d" % (n, lv), proc="GetMageryEnum", args=[n, lv])

# --- movement
for e in [0, 1, 50, 65, 66, 67, 100, 200, -1]:
    for q in [0, 1, 5, -1]:
        for s in [0, 1, 3, -2]:
            add("CalcMovementSpeed-%d-%d-%d" % (e, q, s), proc="CalcMovementSpeed", args=[e, q, s])
add("CalcMovementSpeed-noargs", proc="CalcMovementSpeed", args=[])
add("CalcMovementSpeed-one", proc="CalcMovementSpeed", args=[70])
add("CalcMovementSpeed-two", proc="CalcMovementSpeed", args=[70, 1])


import random, itertools
rng = random.Random(20260605)
def many(name, proc, argsets):
    for i, a in enumerate(argsets):
        add("%s-%04d" % (name, i), proc=proc, args=list(a))
def rint(lo, hi): return rng.randint(lo, hi)
L_MIN, L_MAX = -2147483648, 2147483647

# CalcRestingRate
many("CalcRestingRate", "CalcRestingRate",
     [(l, h, p, r) for l in [1, 10, 20, 50, 100, 255, -20, -25] for h in [0, 1, 40, 50, 100, 255, -5]
      for p in [0, 50, -100] for r in [False, True]])
many("CalcRestingRate-opt", "CalcRestingRate", [(30, 60), (30, 60, 20)])
many("CalcRestingRate-edge", "CalcRestingRate",
     [(2147483647, 1, 0, False), (2147483627, 2, 0, False), (100, 2147483647, 0, False), (50000, 50000, 0, False),
      (60000, 60000, 0, True), (100, 100, 2147483647, False), (100, 200, 2147483547, True),
      (1000000, 1000000, 100, True), (10, 1000000000, 0, True), (-2147483648, 5, 0, False), (5, -2147483648, 0, False)])
many("CalcRestingRate-s", "CalcRestingRate", [(rint(-30, 400), rint(-10, 300), rint(-150, 300), rng.choice([False, True])) for _ in range(200)])
many("CalcRestingRate-w", "CalcRestingRate", [(rint(L_MIN, L_MAX), rint(-100000, 100000), rint(L_MIN, L_MAX), rng.choice([False, True])) for _ in range(100)])

# CalcManaRegen
many("CalcManaRegen", "CalcManaRegen",
     [(l, 63, 71, 40, ml, t, mp, md) for t in [0, 1, 2, 3, 4, 5, 6, -1] for l in [1, 20, 100] for ml in [0, 2, 5]
      for mp in [0, 50, -100] for md in [False, True]])
many("CalcManaRegen-opt", "CalcManaRegen", [(30, 80, 90, 50, 3, 1), (30, 80, 90, 50, 3, 2, 25)])
many("CalcManaRegen-s", "CalcManaRegen",
     [(rint(0, 300), rint(-5, 300), rint(-5, 300), rint(-5, 300), rint(-1, 7), rint(-1, 6), rint(-150, 250), rng.choice([False, True])) for _ in range(300)])
many("CalcManaRegen-w", "CalcManaRegen",
     [(rint(0, 2147483000), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(0, L_MAX), rint(0, 100000), rint(1, 5),
       rint(L_MIN, 2147483547), rng.choice([False, True])) for _ in range(150)])
many("CalcManaRegen-edge", "CalcManaRegen",
     [(50, i, w, 50, 3, t, mp) for t, i, w, mp in [(3, 2147483647, 1, 0), (3, 2147483647, 2147483647, 0), (5, 0, 0, 2147483547),
       (5, 0, 0, 2147483548), (5, 0, 0, -100), (5, 0, 0, -250), (5, 0, 0, 49), (5, 0, 0, 51), (3, -3, 0, 0), (3, 3, 0, 0),
       (3, -1, 0, 0), (3, 1, 0, 0), (3, 2, 1, 0)]])

# CalcMR, CalcMaxHP, CalcMaxMana, CalcSpellCasting
many("CalcMR-s", "CalcMR", [(rint(-20, 300), rint(-20, 300), rint(-100, 100)) for _ in range(150)])
many("CalcMR-w", "CalcMR", [(rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX)) for _ in range(60)])
many("CalcMR-edge", "CalcMR", [(50, 60), (1, 1), (0, 0), (3, 1), (2, 0), (-3, 0), (-1, -1), (2147483647, 0), (0, 715827883), (0, 715827882), (-2147483648, 0), (2147483647, -715827883)])
many("CalcMaxHP-s", "CalcMaxHP", [(rint(0, 3000), rint(-5, 300), rint(-10, 300), rint(-5, 10)) for _ in range(200)])
many("CalcMaxHP-w", "CalcMaxHP", [(rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX)) for _ in range(80)])
many("CalcMaxHP-g", "CalcMaxHP", [(r, l, h, m) for l in [1, 2, 10, 50] for h in [30, 50, 51, 80, 125] for m in [-1, 0, 1] for r in [4, 40]])
many("CalcMaxHP-edge", "CalcMaxHP", [(0, 1, -1, 0), (0, 3, -1, 0), (0, 1, 49, 0), (0, 15, 49, 0), (0, 16, 51, 0), (0, 17, 49, 0), (2147483647, 1, 50, 0), (0, 2147483647, 100, 0), (0, 43000, 100, 0)])
many("CalcMaxMana", "CalcMaxMana", [(l, m) for l in [-1, 0, 1, 10, 50, 255, 100000] for m in [-1, 0, 1, 2, 3, 100000]] +
     [(2147483647, 2), (1073741823, 1), (1073741820, 1), (1073741821, 1), (-1073741824, 1), (-1073741825, 1)])
many("CalcSpellCasting", "CalcSpellCasting",
     [(l, s[0], s[1], s[2], ml, t) for t in [0, 1, 2, 3, 4, 5, 6, -1] for l in [0, 1, 20, 100] for ml in [0, 1, 3]
      for s in [(0, 0, 0), (63, 71, 40), (100, 100, 100), (255, 3, 77), (-9, 10, -4)]])
many("CalcSpellCasting-w", "CalcSpellCasting",
     [(rint(0, 300), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(0, 100000), rint(1, 5)) for _ in range(80)])

# CalcPicklocks
many("CalcPicklocks", "CalcPicklocks", [(l, a, i) for l in [-3, 0, 1, 14, 15, 16, 17, 30, 31, 100, 255] for a in [0, 40, 100, -10] for i in [0, 55, 255]])
many("CalcPicklocks-opt", "CalcPicklocks", [(20, 50, 60, 70)])
many("CalcPicklocks-w", "CalcPicklocks", [(rint(L_MIN, L_MAX), rint(L_MIN, L_MAX), rint(L_MIN, L_MAX)) for _ in range(40)] +
     [(2147483647, 0, 0), (-2147483648, 0, 0), (100, 2147483647, 1), (100, 2147483647, 0), (214748364, 0, 0), (200000000, 0, 0), (20, 2147483647, 0)])

# GetEncumPercents
many("GetEncumPercents", "GetEncumPercents", [(n,) for n in [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 20, 33, 50, 99, 100, 101, 333, 480, 1000, 4800, 7777, 12345, 99999, 100000, 1000000, 2147483647, -1, -7, -100, -2147483648]])
many("GetEncumPercents-s", "GetEncumPercents", [(rint(1, 20000),) for _ in range(120)])

# CalcEncumbrancePercent, AdjustSpeedForSlowness
def cur(v): return {"t": "cur", "v": v}
many("CalcEncumbrancePercent", "CalcEncumbrancePercent",
     [(cur(a), cur(b)) for a in [-5, 0, 1, 33, 50, 100, 480, 999998, 999999, 1000000, 99999.9999, 0.5] for b in [-5, 0, 1, 3, 100, 480, 999999, 1000000, 5000000]] +
     [(cur(999998.9999), cur(999999)), (cur(999999.9999), cur(999999)), (cur(7), cur(9)), (cur(2), cur(3)), (cur(50.5), cur(101)), (cur(33.3333), cur(100))])
many("CalcEncumbrancePercent-s", "CalcEncumbrancePercent", [(cur(rint(0, 6000)), cur(rint(1, 6000))) for _ in range(150)])
many("AdjustSpeedForSlowness", "AdjustSpeedForSlowness", [(cur(v),) for v in [0, 1, 2, 3, 100, 150, 233, -5, -1, 1.5, 2.5, 0.0001, -0.0001, 33.3333, 100000000000, 300000000000000]])

# CalculateStealth: level, agl, int, cha, bClass, bRace, text, plus, encum
def stealth(l, a, i, ch, bc, br, txt="", plus=None, enc=None):
    args = [l, a, i, ch, bc, br, {"ref": "string", "value": txt}]
    if plus is not None: args.append(plus)
    if enc is not None: args.append(enc)
    return args
n = 0
for l in [-5, 0, 1, 15, 16, 17, 30, 100, 500, 16000, 16400, 32767]:
    for st in [(0, 0, 0), (50, 50, 50), (100, 80, 60), (255, 255, 255), (-10, -10, -10), (32767, 32767, 32767), (7, 15, 5)]:
        for bc, br in [(False, False), (True, False), (False, True), (True, True)]:
            add("CalculateStealth-%04d" % n, steps=[{"call": "CalculateStealth", "args": stealth(l, *st, bc, br), "as": "result"}]); n += 1
for l, st, bc, br, txt, plus, enc in [
    (20, (50, 60, 70), True, True, "pre", 5, 0), (20, (50, 60, 70), False, True, "pre", -20, 50), (20, (50, 60, 70), False, False, "pre", 3, 3),
    (20, (50, 60, 70), True, False, "", 32000, 0), (40, (50, 60, 70), True, True, "  ", 0, 100), (40, (50, 60, 70), True, True, " a", -32768, 100),
    (10, (50, 60, 70), True, False, "x", None, None), (10, (50, 60, 70), True, False, "x", 7, None), (32767, (0, 0, 0), True, False, "", 0, 0),
    (16399, (0, 0, 0), True, False, "", 0, 0), (16400, (0, 0, 0), True, False, "", 0, 0), (32000, (0, 0, 0), True, True, "", 0, 0)]:
    add("CalculateStealth-x-%04d" % n, steps=[{"call": "CalculateStealth", "args": stealth(l, *st, bc, br, txt, plus, enc), "as": "result"}]); n += 1
# optional text omitted entirely
add("CalculateStealth-notext", steps=[{"call": "CalculateStealth", "args": [20, 50, 60, 70, True, True], "as": "result"}])

many("RoundUpToNearest5", "RoundUpToNearest5", [(v,) for v in list(range(-12, 13)) + [99, 100, 101, 2147483640, 2147483641, 2147483645, 2147483646, 2147483647, -2147483648, -2147483647, -2147483643, -2147483644]])
many("Get_Enc_Ratio", "Get_Enc_Ratio",
     [(e, a) + ((b,) if b is not None else ()) for e in [0, -5, 1, 3, 7, 100, 4800, 12345, 2147483647] for a in [0, 1, 10, 333, 1000, -5] for b in [None, 0, 5, 100, -1000]] +
     [(100, 2147483647, 1), (100, 2147483647, 0), (100, -2147483648, -1), (100, 2147483647, -1), (7, 1, 2), (3, 1, 1), (8, 1, 0), (16, 1, 0), (32, 5, 0), (64, 1, 0), (3, 2147483647, 0)])
many("Get_Enc_Ratio-s", "Get_Enc_Ratio", [(rint(-10, 12000), rint(0, 6000), rint(0, 6000)) for _ in range(150)])

scn = {
  "family": "stats",
  "database": True,
  "description": "Stats, vitals, creation points and movement (FAM-02): HP/mana/MR/picklocks/stealth, regen rates, encumbrance and movement, CP costs, and the Races/Classes row lookups. Stock path only (bGreaterMUD false).",
  "notes": [
    "Cases that read a Races or Classes row carry it as an extra 'race'/'class' input (the row the lookup lands on, or null), written from the stock database, because the Rust port never reads SQLite.",
    "ComputeAvgLevelMaxStats walks every race; the case carries all of them in 'races'.",
    "CP_CostForRaise, LevelNeededForCP and the Races/Classes getters are called directly; their Private/Public status does not matter to the oracle."
  ],
  "tolerance": {"default": 0},
  "globals": {"bGreaterMUD": False},
  "cases": cases,
}
with open(out, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(',', ':'))
    f.write("\n")
print(len(cases))

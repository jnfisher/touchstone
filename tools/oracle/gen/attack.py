"""Writes fixtures/scenarios/attack.json (FAM-07, swings, energy and weapon damage):
    python3 -I tools/oracle/gen/attack.py fixtures/scenarios/attack.json

Reads the stock SQLite for the rows cases carry as inputs: the first case, "table", holds the
weapons (and a few other items), classes and races CalculateAttack looks up, in compact form.
CalculateAttack builds a weapon's cast-spell text with PullSpellEQ and GetSpellName, which other
cards port; the scenario calls those directly for every (spell, bonus) the attack cases need, and
the Rust test hands the recorded answers back ("pe-*" and "sn-*" cases). Bulk cases are seeded
samples (splitmix64 in the oracle), so the scenario stays small. Deterministic."""
import json, sqlite3, glob, os, sys

out = sys.argv[1]
p = glob.glob(os.path.expanduser('~/.cache/touchstone/mdb/eba20f06*.sqlite'))[0]
c = sqlite3.connect(p); c.row_factory = sqlite3.Row
MISSING = {"t": "missing"}

STAT_ABILS = {7, 11, 13, 14, 15, 19, 34, 35, 36, 37, 38, 39, 40, 41, 42, 101, 102}
items = {}
for r in c.execute("select * from Items order by Number"):
    ab = [(r['Abil-%d' % i], r['AbilVal-%d' % i]) for i in range(20)]
    items[r['Number']] = {"n": r['Number'], "name": r['Name'] or "", "encum": r['Encum'], "accy": r['Accy'],
        "str": r['StrReq'], "speed": r['Speed'], "min": r['Min'], "max": r['Max'], "wt": r['WeaponType'],
        "type": r['ItemType'], "ab": ab}
weapons = [n for n, r in items.items() if r["type"] == 1]
others = [n for n, r in items.items() if r["type"] != 1][::97][:12]
stat_w = [n for n in weapons if any(a in STAT_ABILS for a, v in items[n]["ab"])]
cast_w = [n for n in weapons if any(a == 43 for a, v in items[n]["ab"])]
twohand = [n for n in weapons if items[n]["wt"] in (1, 3)]
plain = [n for n in weapons if n not in stat_w and n not in cast_w]
table_nums = sorted(set(weapons + others))

def flat(ab):
    f = [x for pair in ab for x in pair]
    while f and f[-1] == 0:
        f.pop()
    return f

classes = []
for r in c.execute("select * from Classes order by Number"):
    classes.append([r['Number'], r['CombatLVL'], [r['Abil-%d' % i] for i in range(10)]])
races = []
for r in c.execute("select * from Races order by Number"):
    races.append([r['Number'], [r['Abil-%d' % i] for i in range(10)]])

cases = [{"id": "table",
          "items": [[i["n"], i["name"], i["encum"], i["accy"], i["str"], i["speed"], i["min"], i["max"], i["wt"], flat(i["ab"])]
                    for i in (items[n] for n in table_nums)],
          "item_fields": ["Number", "Name", "Encum", "Accy", "StrReq", "Speed", "Min", "Max", "WeaponType", "AbilitiesFlat"],
          "classes": classes, "races": races}]
grids, samples = [], []

def case(cid, **kw):
    d = {"id": cid}; d.update(kw); cases.append(d)

BONUSES = [0, 25]
cast_sel = cast_w[::2]
stat_w = [w for w in stat_w if w not in cast_w or w in cast_sel]
twohand = [w for w in twohand if w not in cast_w or w in cast_sel]
spells = sorted({v for n in cast_sel for a, v in items[n]["ab"] if a == 43})
for s in spells:
    case("sn-%d" % s, proc="GetSpellName", args=[s, False])
    for b in BONUSES:
        case("pe-%d-%d" % (s, b), proc="PullSpellEQ",
             args=[True, 0, s, MISSING, MISSING, MISSING, True, MISSING, MISSING, MISSING, MISSING, b])

# ---- CalculateAttack
PROFILE_BASE = {"bIsLoadedCharacter": False}
def prof(**f):
    d = dict(PROFILE_BASE); d.update(f)
    return {"udt": "tCharacterProfile", "fields": d}
def atk(profile, typ=5, wpn=0, slow=False, adj=100, ac=0, dr=0, dodge=0, casts="", force=False, sdmg=-1.0, sacc=-1.0, bsd=0):
    return [profile, typ, wpn, slow, adj, ac, dr, dodge, {"ref": "string", "value": casts}, force, sdmg, sacc, bsd]
def attack_case(cid, args, globals_=None, forms=None):
    d = {"id": cid, "proc": "CalculateAttack", "args": args}
    if globals_: d["globals"] = globals_
    if forms: d["forms"] = forms
    case(cid, **{k: v for k, v in d.items() if k != "id"})

G_NAMES = ["Number", "Encum", "Accy", "Crit", "STR", "AGI", "MaxDmg", "BSaccy", "BSmindmg", "BSmaxdmg", "PunchSkill",
           "PunchAccy", "PunchDmg", "KickSkill", "KickAccy", "KickDmg", "JkSkill", "JkAccy", "JkDmg", "Stealth"]
def globals_from(prefix_env):
    return {"nGlobalCharWeapon" + n: "$" + prefix_env + n for n in G_NAMES}

def profile_template(loaded):
    return {"udt": "tCharacterProfile", "fields": {
        "bIsLoadedCharacter": loaded, "nLevel": "$lv", "nClass": "$cls", "nRace": "$race", "nCombat": "$comb",
        "nSTR": "$str", "nAGI": "$agi", "nINT": "$int", "nCHA": "$cha", "nCrit": "$crit", "nPlusMaxDamage": "$pmax",
        "nPlusMinDamage": "$pmin", "nPlusBSaccy": "$pbsa", "nPlusBSmindmg": "$pbsmin", "nPlusBSmaxdmg": "$pbsmax",
        "nMAPlusSkill": "$maskill", "nMAPlusAccy": "$maaccy", "nMAPlusDmg": "$madmg", "nStealth": "$stealth",
        "bClassStealth": "$cst", "bRaceStealth": "$rst", "nAccuracy": "$acc", "nEncumPCT": "$epct",
        "nEncumCurrent": "$ecur", "nEncumMax": "$emax", "nSpellDmgBonus": "$sdb"}}

def profile_draws():
    return {"lv": {"choice": [0, 1, 5, 12, 20, 30, 45, 60, 100, 150, 255]},
        "cls": {"int": [0, 15]}, "race": {"int": [0, 13]}, "comb": {"choice": [0, 1, 2, 3, 4, 5, 6, 8]},
        "str": {"int": [10, 255]}, "agi": {"int": [10, 255]}, "int": {"int": [10, 200]}, "cha": {"int": [10, 200]},
        "crit": {"choice": [0, 0, 5, 12, 25, 40, 41, 55, 80, 130, -10]},
        "pmax": {"int": [-3, 40]}, "pmin": {"int": [-3, 20]}, "pbsa": {"int": [-5, 30]},
        "pbsmin": {"int": [-5, 20]}, "pbsmax": {"int": [-5, 40]},
        "maskill": {"n": 3, "choice": [0, 0, 1, 3, 8, 15, 25, -2]},
        "maaccy": {"n": 3, "int": [-10, 40]}, "madmg": {"n": 3, "int": [-5, 30]},
        "stealth": {"choice": [0, 0, 30, 80, 150, 255]}, "cst": {"choice": [True, False]}, "rst": {"choice": [True, False]},
        "acc": {"choice": [0.0, 0.0, 8.0, 40.5, 90.0, 150.0, 260.0, 400.0]},
        "epct": {"int": [0, 100]}, "ecur": {"int": [0, 6000]}, "emax": {"choice": [0, 20, 48, 1000, 4800, 6000, 9000]},
        "sdb": {"choice": [0, 0, 25, 25]}}

def attack_draws(weapon_pool):
    return {"typ": {"choice": [1, 2, 3, 4, 5, 5, 5, 6, 7, 0, -1]}, "wpn": {"choice": weapon_pool},
        "slow": {"choice": [False, False, True]}, "adj": {"choice": [100, 100, 100, 50, 150, 70, 130, 0]},
        "ac": {"choice": [0, 0, 10, 40, 80, 150]}, "dr": {"choice": [0, 0, 5, 20, 60]},
        "dodge": {"choice": [0, 0, 10, 40, 90, -20, -50, -100, -150]}, "force": {"choice": [False, False, True]},
        "bsd": {"choice": [0, 0, 10, 40]}}

def attack_args(loaded):
    return [profile_template(loaded), "$typ", "$wpn", "$slow", "$adj", "$ac", "$dr", "$dodge",
            {"ref": "string", "value": ""}, "$force", -1.0, -1.0, "$bsd"]

all_pool = [w for w in weapons if w not in cast_w or w in cast_sel] + [0, 0, 0, -2, -3, -4, -5, -1, 99999] + others[:6]
# not loaded: any attack on any weapon
d = profile_draws(); d.update(attack_draws(all_pool))
samples.append({"name": "nl", "seed": 101, "count": 140, "draw": d,
                "case": {"proc": "CalculateAttack", "args": attack_args(False),
                         "forms": {"frmMain": {"lblDatVer": {"Caption": "v1.11p"}}}}})
# loaded: gear swap against the equipped globals
d = profile_draws(); d.update(attack_draws(stat_w * 3 + twohand[:20] + plain[:40] + [0, 0]))
for pre in ("a", "b"):
    pass
d["acc"] = {"choice": [0.0, 8.0, 40.5, 90.0, 150.0, 260.0]}
for n in G_NAMES:
    lo, hi = {"Number": (0, 0), "Encum": (0, 300), "Accy": (-10, 40), "Crit": (-5, 15), "STR": (-5, 15), "AGI": (-5, 15),
              "MaxDmg": (-5, 20), "BSaccy": (-5, 20), "BSmindmg": (-5, 20), "BSmaxdmg": (-5, 20), "Stealth": (-10, 30)}.get(n, (-5, 15))
    if n == "Number":
        d["g" + n] = {"n": 2, "choice": stat_w + twohand[:30] + plain[:30] + [0, 0, 0]}
    else:
        d["g" + n] = {"n": 2, "int": [lo, hi]}
d["qnd"] = {"choice": [0, 0, 3, 10, 20]}
d["aabil"] = {"int": [-20, 60]}; d["aoth"] = {"int": [-20, 60]}; d["aitem"] = {"int": [-20, 60]}
d["dv"] = {"choice": ["v1.11p", "v1.11i", "V1.11I"]}
g = globals_from("g"); g.update({"nGlobalCharQnDbonus": "$qnd", "nGlobalCharAccyAbils": "$aabil",
                                 "nGlobalCharAccyOther": "$aoth", "nGlobalCharAccyItems": "$aitem"})
samples.append({"name": "ld", "seed": 202, "count": 100, "draw": d,
                "case": {"proc": "CalculateAttack", "args": attack_args(True), "globals": g,
                         "forms": {"frmMain": {"lblDatVer": {"Caption": "$dv"}, "lblInvenCharStat": {"$array": {"10": {"Tag": "55.5"}}}}}}})

# weapons that cast spells, every one, with spell damage bonus and negative dodge quirks
for i, n in enumerate(cast_sel):
    bonus = BONUSES[i % 2]
    typ = [5, 5, 4, 6, 5, 7][i % 6]
    loaded = i % 3 == 0
    dodge = [0, 30, -40, 0, -80][i % 5]
    ac = [0, 40, 40, 90, 20][i % 5]
    pr = prof(bIsLoadedCharacter=loaded, nLevel=[0, 20, 45, 120][i % 4], nCombat=4, nSTR=120, nAGI=130, nINT=60, nCHA=60,
              nCrit=[0, 10, 30, 50][i % 4], nSpellDmgBonus=bonus, nAccuracy=90.5, nEncumPCT=20, nEncumCurrent=1500, nEncumMax=5000,
              nClass=[0, 3, 9, 12][i % 4])
    g = {}
    if loaded:
        g = {"nGlobalCharWeaponNumber": [stat_w[i % len(stat_w)], 0]}
    attack_case("cast-%d" % n, atk(pr, typ, n, dodge=dodge, ac=ac, dr=[0, 3][i % 2]), g or None)

# caller-supplied spell text: each layout the matcher knows, durations, multiple spells, no match
CAST_TEXTS = [
    "[fire burns(979), Damage 5 to 15, 100%]",
    "[fire burns(979), Damage 5 to 15, 25%]|[ice freezes(978), Damage(-MR) 4 to 9, 40%]",
    "[drain(1), DrainLife 2 to 8, AffectsLivingOnly, 25%]",
    "[rip(201), Damage 2 to 4 for 8 rounds, 5%]",
    "[rip(201), Damage 2 to 4, AffectsLivingOnly for 10 rounds, 50%]",
    "[x(2), Damage 1 to 3 for 5 rounds, 100%]|[y(3), Damage 3 to 9, 20%]",
    "[a(4), Damage 1 to 4, EndCast [b(5), Damage 2 to 6, EndCast [c(6), Damage 3 to 9]], 25%]",
    "[a(4), Damage 1 to 4, EndCast [b(5), Damage 2 to 6 for 4 rounds, EndCast [c(6), Damage 3 to 9]], 25%]",
    "[a(4), Damage 1 to 4, EndCast [b(5), Damage 2 to 6, EndCast [c(6), Damage 3 to 9, EndCast [d(7), Damage 4 to 8, EndCast [e(8), Damage 1 to 2, EndCast [f(9), Damage 5 to 5]]]]]], 60%]",
    "[a(4), Damage 1 to 4, EndCast [b(5), HoldPerson +100]], 25%]",
    "[big(7), [{p(8), Damage 5 to 15} OR {q(9), Damage 5 to 15} OR {r(10), Damage 2 to 4}], 100%]",
    "[big(7), EndCast [{p(8), Damage 5 to 15} OR {q(9), Damage 7 to 9 for 3 rounds} OR {r(10), DrainLife 2 to 4}]], 50%]",
    "[big(7), EndCast [{p(8), Damage 5 to 15} OR {q(9), Damage 5 to 15} OR {r(10), Damage 2 to 4} OR {s(11), Damage 1 to 2} OR {t(12), Damage 3 to 4} OR {u(13), Damage 9 to 10}]], 100%]",
    "[big(7), EndCast [{p(8), Damage 5 to 15} or {q(9), Damage 5 to 15}]], 25%]",
    "[spear(72), Damage 0+(2*lvl) to 0+(3*lvl), 20% EndCast [slam(1169), HoldPerson +100]]|[dark(443), Illu -999, 100%]",
    "[nothing(3), Accuracy +5 for 10 rounds, 50%]",
    "no spells here",
    "[bad(1), Damage 5 to 15]",
    "[z(5), Damage -3 to -1, 50%]",
    "[z(5), Damage 10 to 40, 33%]|[y(6), Damage 1 to 2, 33%]|[w(7), Damage 20 to 30, 100%]",
    "[[x, Damage 1 to 2 for 3 rounds, EndCast [y, Damage 4 to 6 for 2 rounds]], 80%]",
]
for i, t in enumerate(CAST_TEXTS):
    for j, (bonus, typ, loaded) in enumerate([(0, 5, False), (30, 5, False), (0, 4, True)]):
        w = [stat_w[0], plain[3], twohand[5], plain[10]][j]
        pr = prof(bIsLoadedCharacter=loaded, nLevel=40, nCombat=4, nSTR=130, nAGI=100, nSpellDmgBonus=bonus, nAccuracy=80.0,
                  nEncumPCT=10, nEncumCurrent=900, nEncumMax=4800, nCrit=5)
        attack_case("text-%02d-%d" % (i, j), atk(pr, typ, w, ac=30, dodge=[0, 20, 0, -30][j], casts=t), {"nGlobalCharWeaponNumber": [w, 0]} if loaded else None)

# manual damage and accuracy
for dm in [0.0, 12.5, 13.5, 250.0, 9999999.0, 0.4]:
    for vs in [(0, 0, 0), (40, 3, 20), (80, 10, -40)]:
        for loaded, sacc in [(False, -1.0), (True, -1.0), (False, 55.5), (True, 3.0)]:
            attack_case("manual-%s-%s-%d-%s" % (dm, "_".join(map(str, vs)), loaded, sacc),
                        atk(prof(bIsLoadedCharacter=loaded, nLevel=30), 5, 0, ac=vs[0], dr=vs[1], dodge=vs[2], sdmg=dm, sacc=sacc),
                        None, {"frmMain": {"lblInvenCharStat": {"$array": {"10": {"Tag": "77"}}}}} if loaded else None)
attack_case("manual-overflow", atk(prof(), 5, 0, sdmg=3.0e9))
# weapon 0 / proxy / maxed character grids
for typ in [0, 1, 2, 3, 4, 5, 6, 7, 8]:
    for wpn in [0, -2, -5, -1]:
        for lv, comb in [(0, 0), (30, 3)]:
            for slow in (False, True):
                attack_case("base-%d-%d-%d-%d" % (typ, wpn, lv, slow),
                            atk(prof(nLevel=lv, nCombat=comb, nSTR=100, nAGI=90, nMAPlusSkill=[10, 6, 4], nMAPlusAccy=[5, -3, 8],
                                     nMAPlusDmg=[2, 1, -1], nEncumMax=3000, nCrit=8, nPlusBSaccy=4, nPlusMaxDamage=2, nStealth=60,
                                     bClassStealth=bool(slow), nAccuracy=70.0), typ, wpn, slow, ac=25, dr=2, dodge=15))
for avg in [0.0, 262.3, 300.7]:
    attack_case("maxlevel-%s" % avg, atk(prof(), 5, 0, ac=10, dodge=5, force=True), {"gAvgLevelMaxAllStats": avg})
    attack_case("maxlevel-mk-%s" % avg, atk(prof(), 1, 0, ac=10), {"gAvgLevelMaxAllStats": avg})
    attack_case("maxlevel-bs-%s" % avg, atk(prof(), 4, stat_w[1], ac=10, bsd=5), {"gAvgLevelMaxAllStats": avg})
# every gear ability that feeds the attack, with each attack type, against an empty or a different equipped hand
for ab in sorted(STAT_ABILS):
    w = next((n for n in stat_w if any(a == ab for a, v in items[n]["ab"])), None)
    if w is None:
        continue
    for typ in (1, 2, 3, 4, 5):
        other = plain[ab % 7] if typ % 2 else 0
        pr = prof(bIsLoadedCharacter=True, nLevel=35, nCombat=3, nSTR=90, nAGI=80, nINT=50, nCHA=50, nAccuracy=70.0, nStealth=40,
                  nMAPlusSkill=[5, 5, 5], nEncumPCT=30, nEncumCurrent=2000, nEncumMax=4800, nCrit=10)
        attack_case("gear-%d-%d" % (ab, typ), atk(pr, typ, w, ac=20, dodge=10),
                    {"nGlobalCharWeaponNumber": [other, 0], "nGlobalCharWeaponEncum": [30, 0], "nGlobalCharWeaponPunchSkill": [2, 0],
                     "nGlobalCharWeaponKickSkill": [2, 0], "nGlobalCharWeaponJkSkill": [2, 0]})
for crit in (200, 218, 250, 32000):
    attack_case("crit-%d" % crit, atk(prof(nLevel=50, nCombat=3, nSTR=100, nAGI=90, nCrit=crit, nAccuracy=80.0), 5, plain[1], ac=10))
# negative dodge past -100 pushes the blended hit chance under the minimum
for ac in (5, 40, 150):
    for dodge in (-101, -150, -300, -1000):
        for typ in (5, 4):
            attack_case("negdodge-%d-%d-%d" % (ac, dodge, typ),
                        atk(prof(nLevel=30, nCombat=3, nSTR=100, nAGI=90, nAccuracy=60.0, nStealth=80, bClassStealth=True), typ, plain[4], ac=ac, dodge=dodge))
# the 6-swing cap and extreme speeds
for w in [plain[0], twohand[3]]:
    for lv, agi, comb in [(255, 255, 8), (100, 200, 6), (5, 20, 0), (1, 1, 0), (30, 255, 10)]:
        for adj in (100, 25, 400):
            attack_case("swings-%d-%d-%d-%d" % (w, lv, agi, adj),
                        atk(prof(nLevel=lv, nCombat=comb, nSTR=100, nAGI=agi, nAccuracy=100.0, nEncumMax=4000), 5, w, adj=adj, ac=10))
attack_case("overflow-level", atk(prof(nLevel=40000), 5, plain[0]))
attack_case("energy-zero", atk(prof(nLevel=1, nCombat=-2, nAGI=-150, nSTR=10), 5, plain[0]))

# ---- helper procedures
def helper(name, proc, argsets):
    for i, a in enumerate(argsets):
        case("%s-%03d" % (name, i), proc=proc, args=a)
def cur(v): return {"t": "cur", "v": v}
ens = []
for comb in [0, 3, 8]:
    for lv in [1, 20, 100, 255]:
        for spd in [1000, 1150, 2300, 5000]:
            for agl in [20, 90, 255]:
                ens.append([cur(comb), cur(lv), cur(spd), cur(agl)])
helper("CalcEnergyUsed-base", "CalcEnergyUsed", ens)
ext = []
for strn, item in [(0, 0), (50, 80), (90, 120), (120, 100), (-5, 30)]:
    for enc in [-1, 0, 33, 67, 100]:
        for adj, bs in [(0, False), (100, False), (50, False), (150, True), (70, False)]:
            ext.append([cur(3), cur(40), cur(2300), cur(120), cur(strn), enc, cur(item), cur(adj), bs])
helper("CalcEnergyUsed-opt", "CalcEnergyUsed", ext)
helper("CalcEnergyUsed-edge", "CalcEnergyUsed", [[cur(-2), cur(0), cur(1000), cur(-150)], [cur(0), cur(0), cur(0), cur(0)],
    [cur(3), cur(1), cur(1), cur(1)], [cur(-2), cur(5), cur(1000), cur(50)]])
helper("CalcEnergyUsedWithEncum", "CalcEnergyUsedWithEncum",
       [[cur(3), cur(lv), cur(s), cur(a), cur(st), cur(e), cur(it)] for lv in [10, 60] for s in [1150, 3000] for a in [60, 200]
        for st, it in [(100, 50), (40, 90)] for e in [0, 33, 50, 100]] + [[cur(3), cur(10), cur(1000), cur(100), cur(10), cur(5), MISSING]])
helper("AdjustEnergyUsedWithSpeed", "AdjustEnergyUsedWithSpeed", [[cur(e), cur(s)] for e in [0, 1, 99, 150, 777, 2300] for s in [0, 50, 100, 133, 250]])
helper("AdjustEnergyUsedWithEncum", "AdjustEnergyUsedWithEncum", [[cur(e), cur(s)] for e in [0, 1, 99, 150, 777, 2300] for s in [0, 1, 33, 50, 67, 100]])
helper("CalcQuickAndDeadlyBonus", "CalcQuickAndDeadlyBonus",
       [[cur(a), cur(e), en] for a in [20, 50, 60, 100, 255] for e in [0, 50, 150, 199, 200, 250] for en in [0, 32, 33, 66, 67, 100]])
helper("CalcBSDamage", "CalcBSDamage",
       [[lv, st, dm, bm, cs] for lv in [1, 20, 100, 255] for st in [0, 55, 255] for dm in [0, 14, 80] for bm in [-5, 0, 30] for cs in (True, False)]
       + [[16384, 0, 1, 0, True], [20000, 0, 20000, 0, False], [32700, 0, 1, 0, True], [32767, 255, 1, 0, False], [-5, -50, -3, -9, False]])
helper("CalcTrueAverage", "CalcTrueAverage",
       [[sw, hp, ha, cp, ca, ep, ea] for sw in [0, 1.5, 4.9, 5, 7] for hp, ha in [(0, 0), (60, 25), (99, 130)] for cp, ca in [(0, 0), (15, 80)]
        for ep, ea in [(0, 0), (25, 12)]] + [[-1, 50, 10, 5, 20, 10, 5], [3, 33.3, 17, 7.7, 41, 12.5, 9]])

# GetQuickAndDeadlyBonus reads the main window
for i, (n, filt, cls, strn, agi, ec, em, lv) in enumerate(
        [(stat_w[0], 1, 3, "100", "120", "500", "4800", "30"), (plain[2], 1, 9, "20", "90", "100", "1000", "50"),
         (plain[2], 0, 9, "200", "90", "100", "1000", "50"), (plain[5], 1, -1, "200", "90", "100", "1000", "50"),
         (twohand[1], 1, 12, "255", "255", "9000", "9000", "255"), (0, 1, 3, "100", "100", "0", "100", "10"),
         (99999, 1, 3, "100", "100", "0", "100", "10"), (plain[7], 1, 3, "", "", "", "", ""), (plain[8], 1, 5, "180", "200", "0", "0", "100"),
         (plain[9], 1, 5, "180", "40", "3000", "6000", "1")] + [(plain[10 + k], 1, 1 + k % 15, str(60 + 10 * k), str(50 + 12 * k), str(300 * k), "4800", str(5 + 9 * k)) for k in range(30)]):
    ids = list(range(1, 16))
    cb = {"ItemData": ids, "List": ["x"] * 15, "ListIndex": (cls - 1) if cls > 0 else -1}
    case("GetQuickAndDeadlyBonus-%02d" % i, proc="GetQuickAndDeadlyBonus", args=[n],
         forms={"frmMain": {"chkGlobalFilter": {"Value": filt}, "cmbGlobalClass": {"$array": {"0": cb}},
                            "txtCharStats": {"$array": {"0": {"Tag": strn}, "3": {"Tag": agi}}},
                            "lblInvenCharStat": {"$array": {"0": {"Caption": ec}, "1": {"Caption": em}}},
                            "txtGlobalLevel": {"$array": {"0": {"Text": lv}}}}})

scn = {"family": "attack", "database": True,
       "description": "Swings, energy and weapon damage (FAM-07): CalculateAttack for every attack type, weapon, proxy weapon, martial art and gear swap; energy, Quick and Deadly, backstab damage and true average. Stock path only (bGreaterMUD false).",
       "notes": ["The 'table' case carries the Items, Classes and Races rows CalculateAttack looks up (the Rust port never reads SQLite).",
                 "'sn-*' and 'pe-*' cases record GetSpellName and PullSpellEQ (another card's) for the weapons' cast abilities; the Rust test supplies them as the port's CastText.",
                 "ByRef sCasts comes back in outputs.result.refs[0]."],
       "tolerance": {"default": 0}, "globals": {"bGreaterMUD": False},
       "cases": cases, "samples": samples}
with open(out, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(',', ':')); f.write("\n")
print(len(cases), "explicit cases")

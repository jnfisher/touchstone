"""Writes fixtures/scenarios/accuracy.json: python3 -I tools/oracle/gen/accuracy.py fixtures/scenarios/accuracy.json
Reads the stock SQLite for the Classes rows that cases carry as inputs. Deterministic (splitmix64 draws)."""
import json, sqlite3, glob, os, sys
out = sys.argv[1]
p = glob.glob(os.path.expanduser('~/.cache/touchstone/mdb/eba20f06*.sqlite'))[0]
c = sqlite3.connect(p); c.row_factory = sqlite3.Row

# Only the columns the ported procedures read: GetClassCombat (CombatLVL) and GetClassArmourType.
classes = {}
for r in c.execute("select * from Classes order by Number"):
    classes[r['Number']] = {"number": r['Number'], "combat_lvl": r['CombatLVL'], "armour_type": r['ArmourType']}

cases = []
def add(id_, **kw):
    d = {"id": id_}; d.update(kw); cases.append(d)

M64 = (1 << 64) - 1
class Rng:
    def __init__(self, seed): self.s = seed & M64
    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)
    def int(self, lo, hi): return lo + self.next() % (hi - lo + 1)
    def choice(self, xs): return xs[self.next() % len(xs)]
rng = Rng(20260606)
I16 = (-32768, 32767); I32 = (-2147483648, 2147483647)

def cls_input(n):
    return {"class": classes.get(n)} if n > 0 else {}

# ---- CalculateAccuracy(nClass, nLevel, nSTR, nAGI, nINT, nCHA, nAccyWorn, nPlusAccy, nEncumPCT, ref text, attackType)
def accy(id_, cl, lv, st, ag, worn, plus, enc, text="", it=50, ch=50, at=0):
    add(id_, steps=[{"call": "CalculateAccuracy", "args": [cl, lv, st, ag, it, ch, worn, plus, enc, {"ref": "string", "value": text}, at], "as": "result"}],
        **cls_input(cl))
levels = [-3, 0, 1, 2, 4, 9, 15, 16, 24, 25, 50, 100, 255, 5000, 16383, 16384, 32767]
for cl in list(range(0, 17)) + [99, -1]:
    for lv in levels:
        accy("Accuracy-cl-%d-%d" % (cl, lv), cl, lv, 60, 70, 5, 0, 20)
encs = [-32768, -100, -11, -1, 0, 1, 9, 10, 11, 20, 29, 30, 32, 33, 34, 32767]
for enc in encs:
    for worn in [-5, 0, 1, 32767]:
        for plus in [-3, 0, 32767]:
            accy("Accuracy-enc-%d-%d-%d" % (enc, worn, plus), 4, 20, 0, 0, worn, plus, enc)
stats = [-32768, -5, 0, 1, 47, 49, 50, 52, 53, 56, 100, 32767]
for st in stats:
    for ag in stats:
        accy("Accuracy-sa-%d-%d" % (st, ag), 0, 0, st, ag, 3, 0, 50)
for t in ["", "x", "  ", "a\r\nb"]:
    for enc in [0, 10, 50]:
        for worn in [0, 4]:
            accy("Accuracy-text-%s-%d-%d" % (len(t), enc, worn), 3, 9, 80, 30, worn, 2, enc, text=t)
add("Accuracy-noargs", steps=[{"call": "CalculateAccuracy", "args": [], "as": "result"}])
for i in range(250):
    cl = rng.choice([0, 0, 1, 2, 3, 4, 5, 8, 12, 15, 16, 99]); lv = rng.int(-5, 300)
    accy("Accuracy-s-%04d" % i, cl, lv, rng.int(-10, 300), rng.int(-10, 300), rng.int(-20, 200), rng.int(-50, 500),
         rng.choice([0, rng.int(-5, 120), rng.int(0, 40)]), it=rng.int(0, 255), ch=rng.int(0, 255), at=rng.int(0, 8))
for i in range(80):
    accy("Accuracy-w-%04d" % i, rng.choice([0, 1, 4, 15, 99, rng.int(*I16)]), rng.choice([rng.int(*I16), rng.int(0, 32767)]), rng.int(*I16), rng.int(*I16),
         rng.int(*I16), rng.int(*I16), rng.int(*I16))

# ---- CalculateBackstabAccuracy(stealth, agility, plusBS, bClassStealth, plusNormal, level, str, strReq)
def bsa(id_, *a): add(id_, proc="CalculateBackstabAccuracy", args=list(a))
vals = [-32768, -101, -100, -3, -2, -1, 0, 1, 2, 3, 49, 50, 99, 100, 255, 16383, 16384, 32766, 32767]
for s in vals[::3]:
    for a in vals[1::3] + [32767]:
        for cs in (False, True):
            bsa("Backstab-sa-%d-%d-%d" % (s, a, cs), s, a, 5, cs, 0)
for pb in vals[::2]:
    for pn in [-32768, 0, 7, 32767]:
        for cs in (False, True):
            bsa("Backstab-p-%d-%d-%d" % (pb, pn, cs), 60, 70, pb, cs, pn)
bsa("Backstab-optional", 60, 70, 4, True, 3, 20, 55, 60)
for i in range(200):
    bsa("Backstab-s-%04d" % i, rng.int(-20, 400), rng.int(-20, 400), rng.int(-100, 300), rng.choice([False, True]), rng.int(-50, 300),
        rng.int(0, 100), rng.int(0, 100), rng.int(0, 100))

# ---- CalcDodgeVSAccuracy(rawDodge, accy, class)
raws = [-2147483648, -1, 0, 1, 2, 5, 9, 10, 50, 95, 100, 500, 1000, 9999, 10000, 50000, 2147483647]
accs = [-2147483648, -5, 0, 7, 8, 9, 15, 16, 17, 24, 25, 40, 100, 800, 1000, 9999, 10000, 2147483647]
for r in raws:
    for a in accs:
        add("DodgeVsAccy-%d-%d" % (r, a), proc="CalcDodgeVSAccuracy", args=[r, a])
for cl in [0, 1, 4, 99]:
    add("DodgeVsAccy-class-%d" % cl, proc="CalcDodgeVSAccuracy", args=[40, 100, cl])
for i in range(250):
    add("DodgeVsAccy-s-%04d" % i, proc="CalcDodgeVSAccuracy", args=[rng.int(-10, 600), rng.int(-10, 1500), rng.int(0, 16)])

# ---- CalcDodge(level, agility, charm, plusDodge, curEncum, maxEncum)
def dodge(id_, *a): add(id_, proc="CalcDodge", args=list(a))
for lv in [-32768, -6, 0, 5, 100, 32767]:
    for ag in [-32768, -5, 47, 49, 50, 52, 53, 100, 32767]:
        for ch in [-32768, -32718, 0, 45, 55, 32767]:
            dodge("Dodge-lac-%d-%d-%d" % (lv, ag, ch), lv, ag, ch, 0.0)
for pl in [-40000.0, -33.5, -2.5, -0.5, 0.0, 0.4999, 0.5, 1.5, 2.5, 3.5, 100.25, 32767.0, 32767.5, 1e300, -1e300]:
    for base in [(0, 50, 50), (20, 80, 70), (32767, 50, 50)]:
        dodge("Dodge-plus-%s-%s" % (pl, base), base[0], base[1], base[2], pl)
for cur in [-100.0, 0.0, 10.0, 32.0, 33.0, 480.0, 1e9, 1e300]:
    for mx in [-1.0, 0.0, 1e-300, 100.0, 480.0, 1e300]:
        dodge("Dodge-enc-%s-%s" % (cur, mx), 30, 60, 55, 4.0, cur, mx)
for cur in [0.0, 3.0, 9.9, 10.0, 19.9, 20.0, 29.9, 30.0, 32.9]:
    dodge("Dodge-encbands-%s" % cur, 30, 60, 55, 2.5, cur, 100.0)
dodge("Dodge-enc-ovf-hi", 32760, 50, 50, 0.0, 0.0, 100.0)
dodge("Dodge-enc-ovf-lo", -32760, 50, 50, 0.0, -1000.0, 100.0)
dodge("Dodge-noargs")
dodge("Dodge-lvl-only", 50)
dodge("Dodge-four", 50, 80, 60, 3.0)
dodge("Dodge-five", 50, 80, 60, 3.0, 10.0)
for i in range(200):
    dodge("Dodge-s-%04d" % i, rng.int(-50, 300), rng.int(-20, 300), rng.int(-20, 300), rng.int(-300, 600) / rng.choice([1, 2, 4, 10]),
          rng.int(0, 500) / 1.0, rng.choice([-1.0, 0.0, 100.0, rng.int(1, 600) / 1.0]))

# ---- caps, minimum, class helpers
for cl in [-32768, -1, 0, 1, 4, 8, 15, 99, 32767]:
    for soft in [False, True]:
        add("GetDodgeCap-%d-%d" % (cl, soft), proc="GetDodgeCap", args=[cl, soft])
    add("GetHitMin-%d" % cl, proc="GetHitMin", args=[cl])
add("GetDodgeCap-noargs", proc="GetDodgeCap", args=[])
add("GetDodgeCap-class-only", proc="GetDodgeCap", args=[3])
add("GetHitMin-noargs", proc="GetHitMin", args=[])
add("GetHitCap", proc="GetHitCap", args=[])
add("GetSpellHitCap", proc="GetSpellHitCap", args=[])
for cl in list(range(-1, 17)) + [99, 5000]:
    add("IsLightArmour-%d" % cl, proc="GMUD_IsLightArmourClass", args=[cl], **cls_input(cl))
for v in [-1e10, -123.456, -4.0, -1.0, -0.25, 0.0, 0.25, 1.0, 3.0, 4.0, 10.0, 37.5, 100.0, 12345.678, 1e10, 1e100, 1e300, 1.7e308]:
    for s in [-4.0, 0.0, 1e-300, 0.5, 1.0, 4.0, 7.0, 100.0, 1e300]:
        add("DimReturns-%s-%s" % (v, s), proc="GMUD_DiminishingReturns", args=[v, s])

# ---- CalculateAttackDefense(accy, ac, dodge, bsDef, protEv, protGd, perception, vileWard, eEvil,
#                            bShadow, bSeeHidden, bBackstab, bVSplayer, class)
def ad(id_, accy, ac, dodge=0, bsd=0, pev=0, pgd=0, per=0, vile=0, evil=0, sh=False, see=False, bs=False, vp=False, cl=0):
    add(id_, proc="CalculateAttackDefense", args=[accy, ac, dodge, bsd, pev, pgd, per, vile, evil, sh, see, bs, vp, cl])
A = [-2147483648, -46341, -46340, -5, 0, 1, 8, 9, 13, 50, 140, 187, 1000, 9999, 10000, 2147483647]
AC = [-2147483648, -1, 0, 1, 10, 22, 50, 100, 500, 1000, 9999, 10000, 2147483647]
for a in A:
    for ac in AC:
        ad("AD-n-%d-%d" % (a, ac), a, ac)
D = [-2147483648, -999, -1, 0, 1, 9, 10, 50, 96, 500, 9999, 10000, 2147483647]
for a in [-5, 0, 8, 9, 17, 100, 153, 9999]:
    for d in D:
        for bs in (False, True):
            ad("AD-d-%d-%d-%d" % (a, d, bs), a, 40, dodge=d, bs=bs)
for cl in [0, 4, 99, 32767, 32768, -32768, -32769, 100000, 2147483647]:
    ad("AD-class-%d" % cl, 200, 60, dodge=30, cl=cl)
    ad("AD-class-bs-%d" % cl, 200, 60, dodge=30, bs=True, cl=cl)
# secondary defenses on a normal attack, and what backstab does with them
for pev in [-2147483648, -50, 0, 20, 10000, 2147483647]:
    for pgd in [-2147483648, 0, 20, 2147483647]:
        for sh in (False, True):
            for ac in [0, 30]:
                ad("AD-prot-%d-%d-%d-%d" % (pev, pgd, sh, ac), 120, ac, pev=pev, pgd=pgd, sh=sh)
for vile in [-2147483648, -10, 0, 7, 30, 9999, 10000, 2147483647]:
    for sh in (False, True):
        for bs in (False, True):
            for ac in [-5, 0, 25]:
                ad("AD-vile-%d-%d-%d-%d" % (vile, sh, bs, ac), 150, ac, pev=5, pgd=3, vile=vile, sh=sh, bs=bs)
# backstab: defense from AC, BS defense and perception
for bs_def in [-2147483648, 0, 15, 10000, 2147483647]:
    for per in [-2147483648, -30, 40, 10000, 2147483647]:
        for ac in [-20, 3, 60, 2147483647]:
            for sh, vp in [(False, False), (True, True), (False, True)]:
                ad("AD-bs-%d-%d-%d-%d-%d" % (bs_def, per, ac, sh, vp), 140, ac, dodge=20, bsd=bs_def, per=per, bs=True, vp=vp, sh=sh, see=(per & 1) == 0)
for ac in [0, 4, 7, 100, 200, 400, 10000]:
    for see in (False, True):
        for vp in (False, True):
            for a in [50, 200, 140, 400]:
                ad("AD-bsf-%d-%d-%d-%d" % (ac, see, vp, a), a, ac, dodge=60, bsd=12, per=50, bs=True, vp=vp, see=see)
# eEvil does nothing on stock
for ev in [-300, -201, -1, 0, 39, 40, 119, 120, 500, 501, 2147483647]:
    for bs in (False, True):
        ad("AD-evil-%d-%d" % (ev, bs), 180, 70, dodge=25, bsd=5, pev=3, pgd=2, per=30, vile=40, evil=ev, bs=bs, vp=True)
ad("AD-defaults-only", 100, 50)
add("AD-short-args", proc="CalculateAttackDefense", args=[100, 50, 20])
add("AD-seven-args", proc="CalculateAttackDefense", args=[100, 50, 20, 5, 6, 7, 8])
for i in range(350):
    wide = i % 5 == 4
    g = (lambda lo, hi: rng.int(*I32)) if wide else (lambda lo, hi: rng.int(lo, hi))
    ad("AD-s-%04d" % i, g(-50, 1200), g(-50, 600), g(-100, 700), g(-50, 400), g(-30, 200), g(-30, 200), g(-30, 400), g(-30, 200),
       rng.int(-300, 600), rng.choice([False, True]), rng.choice([False, True]), rng.choice([False, True]), rng.choice([False, True]),
       rng.choice([0, 1, 4, 8, 15]))

# ---- DodgeMaxAccForPercent(rawDodge, targetPct, class)
R = [-5, 0, 1, 2, 5, 9, 10, 11, 15, 50, 95, 96, 200, 1000, 9999, 10000, 2147483647]
T = [-5, 0, 1, 2, 5, 17, 50, 94, 95, 96, 2147483647]
for r in R:
    for t in T:
        add("MaxAcc-%d-%d" % (r, t), proc="DodgeMaxAccForPercent", args=[r, t])
for cl in [0, 4, 99]:
    add("MaxAcc-class-%d" % cl, proc="DodgeMaxAccForPercent", args=[60, 40, cl])
add("MaxAcc-two", proc="DodgeMaxAccForPercent", args=[60, 40])
for i in range(250):
    add("MaxAcc-s-%04d" % i, proc="DodgeMaxAccForPercent", args=[rng.int(1, 400), rng.int(1, 100), rng.choice([0, 3])])
# the nudge loops: sweep a dense small range
for r in range(1, 31):
    for t in [1, 13, 95]:
        add("MaxAcc-dense-%d-%d" % (r, t), proc="DodgeMaxAccForPercent", args=[r, t])

scn = {
  "family": "accuracy",
  "database": True,
  "description": "Accuracy, hit chance and dodge (FAM-06): CalculateAccuracy, CalculateBackstabAccuracy, CalculateAttackDefense, CalcDodgeVSAccuracy, CalcDodge, DodgeMaxAccForPercent, the hit/dodge caps and the GMUD helpers. Stock path only (bGreaterMUD false).",
  "notes": [
    "Cases that read a Classes row carry it as an extra 'class' input (number, combat_lvl, armour_type), or null when the lookup fails, written from the stock database because the Rust port never reads SQLite.",
    "Optional arguments are omitted in a few cases to pin MME's defaults; arguments only GreaterMUD reads (eEvil, intellect, charm, attack type) are passed to show they have no effect on stock.",
    "GetDodgeCap, GetHitMin and GMUD_IsLightArmourClass are called directly with arbitrary class numbers."
  ],
  "tolerance": {"default": 0},
  "globals": {"bGreaterMUD": False},
  "cases": cases,
}
with open(out, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(',', ':'))
    f.write("\n")
print(len(cases))

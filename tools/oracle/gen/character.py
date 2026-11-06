"""Writes fixtures/scenarios/character.json (FAM-05, modMain half: character profile and attack config).

    python3 -I tools/oracle/gen/character.py [DB.sqlite]

Procedures: PopulateCharacterProfile, SetCurrentAttackTypeConfig, SetCharDefenseDescription,
RefreshCombatHealingValues, GetCurrentAttackName, bCharIsEvilAligned, GetCharActiveVileWard,
ClearLearnedSpells, LearnSpell, UnLearnSpell, LearnOrUnlearnSpell.

Deterministic (splitmix-style counter, no clock). The case "table" carries the Spells, Items and
Classes rows the other cases touch (only the fields the Rust port reads), so the Rust test needs no
database. Every case sets the whole of frmMain (a case replaces the form), so each lists the controls
it cares about; a control left out reads as the VB default ("" / 0 / ListIndex -1).
"""
import json
import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DEFAULT_DB = os.path.expanduser(
    "~/.cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite")

SPELL_FIELDS = ["Number", "Name", "Short", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff",
                "TypeOfResists", "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc",
                "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc", "Learnable", "LearnedFromLen", "CastedByLen",
                "Classes"]
ITEM_FIELDS = ["Number", "Name", "ItemType", "WeaponType", "StrReq"]

HEAL_SPELLS = [13, 17, 27, 89, 121, 123, 138, 747, 831, 1039, 1059, 19, 313]
ATTACK_SPELLS = [1, 285, 301, 340, 706, 413, 426, 207, 17]
BAD_SPELL = 99999
CLASSES = [0, 1, 4, 6, 7, 8, 12, 14, 15, 99]       # 99: no such row; 0: the "none" class
RACES = [1, 2, 3, 5, 9, 13]
M64 = 2**64 - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def pick(self, xs):
        return xs[self.next() % len(xs)]

    def int(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)

    def chance(self, n, d):
        return self.next() % d < n


def load(db):
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    spells, items, classes = {}, {}, []
    for r in c.execute("select * from Spells order by Number"):
        ab = []
        for i in range(10):
            a, v = r["Abil-%d" % i], r["AbilVal-%d" % i]
            ab += [a, v]
        spells[r["Number"]] = [r["Number"], r["Name"] or "", r["Short"] or "", r["ReqLevel"], r["EnergyCost"],
                               r["ManaCost"], r["MinBase"], r["MaxBase"], r["Diff"], r["TypeOfResists"],
                               r["Targets"], r["Dur"], r["AttType"], r["Magery"], r["MageryLVL"], r["Cap"],
                               r["MaxIncLVLs"], r["MaxInc"], r["MinIncLVLs"], r["MinInc"], r["DurIncLVLs"],
                               r["DurInc"], r["Learnable"], len(r["Learned From"] or ""),
                               len(r["Casted By"] or ""), r["Classes"] or "", ab]
    for r in c.execute("select * from Items order by Number"):
        ab = []
        for i in range(20):
            a, v = r["Abil-%d" % i], r["AbilVal-%d" % i]
            if a in (22, 116):          # the only abilities PopulateCharacterProfile asks for
                ab += [i, a, v]
        items[r["Number"]] = [r["Number"], r["Name"] or "", r["ItemType"], r["WeaponType"], r["StrReq"], ab]
    for r in c.execute("select * from Classes order by Number"):
        ab = []
        for i in range(10):
            ab += [r["Abil-%d" % i], r["AbilVal-%d" % i]]
        classes.append([r["Number"], r["CombatLVL"], ab])
    return spells, items, classes


# ----------------------------------------------------------------------------- form building

def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def frm(p):
    """The frmMain controls the procedures read, from a compact parameter dict."""
    f = {}
    if "filter" in p:
        f["chkGlobalFilter"] = {"Value": p["filter"]}
    if "lair" in p:
        f["optMonsterFilter"] = arr({1: {"Value": bool(p["lair"])}})
    if p.get("lairtxt"):
        f["txtMonsterLairFilter"] = arr({k: {"Text": v} for k, v in p["lairtxt"].items()})
    if "mdmg" in p:
        f["txtMonsterDamage"] = {"Text": p["mdmg"]}
    if "level" in p:
        f["txtGlobalLevel"] = arr({0: {"Text": p["level"]}})
    if "cls" in p:
        if p["cls"] is None:
            f["cmbGlobalClass"] = arr({0: {"ItemData": [1, 2], "ListIndex": -1, "StrictItemData": True}})
        else:
            f["cmbGlobalClass"] = arr({0: {"ItemData": [p["cls"]], "ListIndex": 0}})
    if "race" in p:
        if p["race"] is None:
            f["cmbGlobalRace"] = arr({0: {"ItemData": [1, 2], "ListIndex": -1, "StrictItemData": True}})
        else:
            f["cmbGlobalRace"] = arr({0: {"ItemData": [p["race"]], "ListIndex": 0}})
    if "align" in p:
        f["cmbGlobalAlignment"] = {"ListIndex": p["align"]}
    stat = {}
    if "enc" in p:
        stat[0] = {"Caption": p["enc"][0]}
        stat[1] = {"Caption": p["enc"][1]}
    for k, v in (p.get("tag") or {}).items():
        stat.setdefault(k, {})["Tag"] = v
    if stat:
        f["lblInvenCharStat"] = arr(stat)
    if "cs19" in p:
        f["lblInvenStats"] = arr({19: {"Tag": p["cs19"]}})
    if p.get("cstat"):
        f["txtCharStats"] = arr({k: {"Tag": v} for k, v in p["cstat"].items()})
    for key, ctl, prop in (("hp", "lblCharMaxHP", "Tag"), ("rest", "lblCharRestRate", "Tag"),
                           ("med", "txtCharManaRegen", "Tag"), ("mana", "lblCharMaxMana", "Tag"),
                           ("mrate", "lblCharManaRate", "Tag"), ("sc", "lblCharSC", "Tag"),
                           ("bless", "lblCharBless", "Caption"), ("mr", "txtCharMR", "Text"),
                           ("anti", "chkCharAntiMagic", "Value")):
        if key in p:
            f[ctl] = {prop: p[key]}
    return {"frmMain": f}


def prof(**kw):
    return {"udt": "tCharacterProfile", "fields": kw, "out": True}


def s(n):
    return str(n)


def main():
    spells, items, classes = load(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB)
    cases = []
    rng = Rng(1105)
    used_items = set()

    def add(cid, **kw):
        c = {"id": cid}
        c.update(kw)
        cases.append(c)

    # ---- item pools
    weapons116 = [n for n, r in items.items() if r[2] == 1 and any(r[5][k + 1] == 116 for k in range(0, len(r[5]), 3))]
    pos116 = [n for n in weapons116 if any(items[n][5][k + 2] > 0 for k in range(0, len(items[n][5]), 3) if items[n][5][k + 1] == 116)]
    neg116 = [n for n in weapons116 if any(items[n][5][k + 2] < 0 for k in range(0, len(items[n][5]), 3) if items[n][5][k + 1] == 116)]
    zero116 = [n for n in weapons116 if any(items[n][5][k + 2] == 0 for k in range(0, len(items[n][5]), 3) if items[n][5][k + 1] == 116)]
    twohand = [n for n, r in items.items() if r[2] == 1 and r[3] in (1, 3)]
    onehand = [n for n, r in items.items() if r[2] == 1 and r[3] in (0, 2)]
    nonweapon = [n for n, r in items.items() if r[2] != 1][:3]
    WPN = sorted(set(pos116[:3] + neg116[:3] + zero116[:2] + twohand[:3] + onehand[:2] + nonweapon[:2]))
    WPN_EXTRA = [0, 99999]
    used_items.update(WPN)

    # ---- tag pools
    def tag(lo, hi, frac=False):
        v = rng.int(lo, hi)
        if frac and rng.chance(1, 6):
            return s(v) + rng.pick([".5", ".25", ".75", ".4", ".6"])
        return s(v)

    def rand_stats(big=False):
        """lblInvenCharStat(i).Tag for the indexes PopulateCharacterProfile reads."""
        t = {}
        t[8] = tag(0, 150)
        t[7] = tag(0, 60)
        t[11] = tag(0, 120, True)
        t[30] = tag(0, 90)
        t[13] = tag(-20, 80)
        t[14] = tag(0, 60)
        t[15] = tag(0, 90)
        t[19] = tag(0, 120)
        t[33] = tag(-20, 100)
        t[10] = tag(-40, 400)
        t[12] = tag(0, 400)
        t[31] = rng.pick(["0", "0", "5", "12", "30"])
        for i in range(34, 43):
            t[i] = tag(0, 90)
        return t

    def rand_p(**over):
        lvl = rng.int(1, 255)
        p = {
            "filter": 1, "lair": False, "level": s(lvl),
            "cls": rng.pick(CLASSES), "race": rng.pick(RACES), "align": rng.int(-1, 5),
            "enc": [s(rng.int(0, 400)), s(rng.pick([0, 1, 50, 100, 150, 300, 600, 1000]))],
            "tag": rand_stats(),
            "cstat": {0: tag(10, 120), 1: tag(10, 120), 3: tag(10, 120), 5: tag(10, 120)},
            "hp": tag(10, 900), "rest": tag(0, 90, True), "med": tag(0, 40, True), "mana": tag(0, 700),
            "mrate": tag(0, 40, True), "sc": tag(0, 260), "bless": rng.pick(["0", "6", "12", "30", "45"]),
            "cs19": rng.pick(["0", "1", "2", "3"]),
        }
        p.update(over)
        return p

    def rand_g(**over):
        g = {
            "nNMRVer": rng.pick([1.7, 1.82, 1.83, 1.84]),
            "bGlobalAttackUseMeditate": rng.chance(1, 3),
            "nGlobalAttackHealValue": rng.pick([0, 0, 30, 999999, 12345]),
            "nGlobalAttackHealCost": rng.pick([0.0, 0.0, 1.5, 8.25, 40.0]),
            "nGlobalAttackTypeMME": rng.int(0, 7),
            "nGlobalAttackSpellNum": rng.pick(ATTACK_SPELLS + [0]),
            "bGlobalAttackBackstab": rng.chance(1, 3),
            "nGlobalAttackBackstabWeapon": rng.pick([0, 0] + WPN),
            "nGlobalCharWeaponNumber": [rng.pick([0] + WPN), rng.pick([0, 0] + WPN)],
            "nGlobalCharWeaponBSaccy": [rng.int(-5, 20), rng.int(-5, 20)],
            "nGlobalCharWeaponAccy": [rng.int(-5, 25), rng.int(-5, 25)],
            "nGlobalCharAccyAbils": rng.int(-10, 30),
            "nGlobalCharAccyOther": rng.int(-10, 30),
            "nGlobalCharAccyItems": rng.int(-10, 60),
            "nGlobalCharHitMagicNonWeapon": rng.pick([0, 0, 25, 180]),
            "gAvgLevelMaxAllStats": rng.pick([0.0, 0.0, 120.5, 300.25]),
        }
        g.update(over)
        return g

    def populate(cid, p, g, force_use=False, no_party=False, mud=0, wpn=0, no_char=False, fields=None):
        add(cid, forms=frm(p), globals=g, proc="PopulateCharacterProfile",
            args=[prof(**(fields or {})), force_use, no_party, mud, wpn, no_char])

    # ============================================================ PopulateCharacterProfile
    # character branch, every non-surprise attack type
    for i in range(30):
        p = rand_p()
        g = rand_g()
        mud = rng.pick([0, 1, 2, 3, 5, 6, 7])
        populate("pop-char-%d" % i, p, g, rng.chance(1, 5), True, mud)
    # filter off but forced
    for i in range(6):
        populate("pop-force-%d" % i, rand_p(filter=0), rand_g(), True, True, rng.pick([0, 1, 4]), rng.pick([0, -1] + WPN))
    # surprise (backstab) accuracy, every weapon-choice rule
    for i in range(45):
        p = rand_p(cls=rng.pick([1, 6, 7, 8, 14, 12, 0, 99]))
        g = rand_g(nGlobalAttackTypeMME=rng.pick([1, 2, 4, 5]))
        w = rng.pick(WPN + WPN_EXTRA + [-1, -1, 0, 0, 0])
        populate("pop-bs-%d" % i, p, g, rng.chance(1, 4), True, 4, w)
    # no class / race selected (ItemData(-1) is run-time error 381): the profile stops half filled
    for i, (cls, race) in enumerate([(None, 1), (3, None), (None, None)]):
        populate("pop-nosel-%d" % i, rand_p(cls=cls, race=race), rand_g(), False, True, 0)
    # party
    for i in range(16):
        lt = {0: rng.pick(["2", "3", "6", "9", "1", "0", "x4"]), 1: tag(0, 80), 2: tag(0, 50), 3: tag(0, 90),
              4: tag(0, 90), 5: rng.pick(["0", "-5", "40", "120", "300"]), 6: "1", 7: tag(0, 30), 8: tag(0, 300)}
        p = rand_p(filter=rng.pick([0, 1]), lair=True, lairtxt=lt, mdmg=rng.pick(["", "0", "55", "400", "-8", "99999999"]))
        g = rand_g()
        populate("pop-party-%d" % i, p, g, rng.chance(1, 6), rng.chance(1, 5), rng.pick([0, 1, 2, 3, 4, 5]),
                 rng.pick([0] + WPN[:3]), rng.chance(1, 8),
                 fields={"nParty": rng.pick([0, 0, 3, 8, -2])} if rng.chance(1, 3) else None)
    # no character, no party
    for i in range(18):
        p = rand_p(filter=rng.pick([0, 0, 1]), lair=rng.chance(1, 4),
                   lairtxt={0: rng.pick(["1", "0", "2"])}, mdmg=rng.pick(["", "70", "-3", "10000000", "2.5"]))
        g = rand_g(nNMRVer=rng.pick([1.6, 1.82, 1.83, 1.84, 1.9]),
                   nGlobalAttackTypeMME=rng.pick([0, 1, 2, 3, 4, 5, 6, 7]),
                   nGlobalAttackSpellNum=rng.pick(ATTACK_SPELLS + [0, BAD_SPELL]))
        populate("pop-none-%d" % i, p, g, False, True, rng.pick([0, 1, 2, 3, 4, 5]),
                 rng.pick([0, -1]), rng.chance(1, 3))
    # edges: integer conversion, clamps, overflow
    edge = [
        ("pop-edge-round", dict(tag={11: "40.5", 30: "41.5", 7: "-0.5", 8: "1.5", 13: "2.5", 14: "3.5", 15: "-2.5", 19: "19.5", 33: "7.5"}), {}),
        ("pop-edge-ovf-maxdmg", dict(tag={11: "40000"}), {}),
        ("pop-edge-ovf-stat", dict(cstat={0: "33000"}), {}),
        ("pop-edge-hp-low", dict(hp="0.3", rest="-4"), {}),
        ("pop-edge-mana-neg", dict(mana="-20", mrate="-3", sc="-5"), {}),
        ("pop-edge-mana-big", dict(mana="20000000", mrate="30000000", bless="90000000"), {"nGlobalAttackHealCost": 9000000.0}),
        ("pop-edge-acc-neg", dict(tag={10: "-50"}), {}),
        ("pop-edge-acc-big", dict(tag={10: "20000000"}), {}),
        ("pop-edge-enc-over", dict(enc=["900", "300"]), {}),
        ("pop-edge-enc-neg", dict(enc=["-50", "300"]), {}),
        ("pop-edge-enc-zero-max", dict(enc=["50", "0"]), {}),
        ("pop-edge-walk", dict(enc=["200", "300"], tag={31: "10"}), {}),
        ("pop-edge-walk-light", dict(enc=["10", "300"], tag={31: "10"}), {}),
        ("pop-edge-quick-ovf", dict(enc=["200", "300"], tag={31: "3000000000"}), {}),
        ("pop-edge-thresh-neg", dict(), {"nGlobalAttackHealValue": -5}),
        ("pop-edge-thresh-big", dict(), {"nGlobalAttackHealValue": 20000000}),
        ("pop-edge-spellcost-energy", dict(), {"nGlobalAttackTypeMME": 2, "nGlobalAttackSpellNum": 285}),
        ("pop-edge-spellcost-500", dict(), {"nGlobalAttackTypeMME": 3, "nGlobalAttackSpellNum": 301}),
        ("pop-edge-spellcost-none", dict(), {"nGlobalAttackTypeMME": 2, "nGlobalAttackSpellNum": BAD_SPELL}),
        ("pop-edge-spellcost-melee", dict(), {"nGlobalAttackTypeMME": 1, "nGlobalAttackSpellNum": 285}),
        ("pop-edge-meditate", dict(med="17.5"), {"bGlobalAttackUseMeditate": True}),
        ("pop-edge-nomeditate", dict(med="17.5"), {"bGlobalAttackUseMeditate": False}),
    ]
    for cid, pov, gov in edge:
        p = rand_p(cls=7, race=3, filter=1, **pov)
        g = rand_g(nGlobalAttackTypeMME=gov.get("nGlobalAttackTypeMME", 1), **{k: v for k, v in gov.items() if k != "nGlobalAttackTypeMME"})
        populate(cid, p, g, False, True, 0)
    # surprise edges
    sp = rand_p(cls=7, race=3)
    gb = rand_g(nGlobalCharWeaponNumber=[pos116[0], twohand[0]], bGlobalAttackBackstab=False)
    for cid, w, gov in [
        ("pop-bs-edge-same", pos116[0], {}),
        ("pop-bs-edge-2h-offhand", twohand[0], {}),
        ("pop-bs-edge-2h-nooffhand", twohand[0], {"nGlobalCharWeaponNumber": [pos116[0], 0]}),
        ("pop-bs-edge-punch", -1, {}),
        ("pop-bs-edge-cur", 0, {"bGlobalAttackBackstab": False}),
        ("pop-bs-edge-bsweapon", 0, {"bGlobalAttackBackstab": True, "nGlobalAttackBackstabWeapon": neg116[0]}),
        ("pop-bs-edge-bsweapon0", 0, {"bGlobalAttackBackstab": True, "nGlobalAttackBackstabWeapon": 0}),
        ("pop-bs-edge-missing", 99999, {}),
        ("pop-bs-edge-ovf", pos116[1], {"nGlobalCharAccyAbils": 40000, "nGlobalCharAccyOther": 0}),
        ("pop-bs-edge-ovf2", pos116[1], {"nGlobalCharWeaponBSaccy": [-40000, 0], "nGlobalCharWeaponNumber": [neg116[0], 0]}),
        ("pop-bs-edge-nostealth", pos116[1], {}),
    ]:
        g = dict(gb)
        g.update(gov)
        pp = dict(sp)
        if cid.endswith("nostealth"):
            pp = rand_p(cls=1, race=3)
        populate(cid, pp, g, False, True, 4, w)

    # ============================================================ SetCurrentAttackTypeConfig
    ma_g = {}
    for nm in ("Punch", "Kick", "Jk"):
        ma_g["nGlobalCharWeapon%sSkill" % nm] = [rng.int(0, 30), rng.int(0, 30)]
        ma_g["nGlobalCharWeapon%sAccy" % nm] = [rng.int(0, 30), rng.int(0, 30)]
        ma_g["nGlobalCharWeapon%sDmg" % nm] = [rng.int(0, 30), rng.int(0, 30)]
    for i in range(40):
        t = rng.int(0, 7)
        filt = rng.pick([0, 1, 1])
        p = rand_p(filter=filt)
        g = {
            "nGlobalAttackTypeMME": t,
            "nGlobalAttackSpellNum": rng.pick(ATTACK_SPELLS),
            "nGlobalAttackSpellLVL": rng.pick([0, 5, 30]),
            "bGlobalAttackUseMeditate": rng.chance(1, 3),
            "nGlobalAttackManualP": rng.pick([0, 20, 150]),
            "nGlobalAttackManualM": rng.pick([0, 15, 300]),
            "nGlobalAttackMA": rng.pick([1, 2, 3, 0]),
            "bGlobalAttackBackstab": rng.chance(1, 3),
            "nGlobalAttackBackstabWeapon": rng.pick([0] + WPN[:4]),
            "nGlobalCharWeaponNumber": [rng.pick([0] + WPN), rng.pick([0, 0] + WPN)],
            "nGlobalCharQnDbonus": rng.pick([0, 5, 10]),
            "sGlobalAttackConfig": rng.pick(["", "stale", "0"]),
            "sCharDamageVsMonsterConfig": "marker",
        }
        g.update(ma_g)
        add("cfg-%d" % i, forms=frm(p), globals=g, proc="SetCurrentAttackTypeConfig", args=[],
            read={"cfg": "%g.sGlobalAttackConfig", "cdv": "%g.sCharDamageVsMonsterConfig"})
    # unchanged config leaves the cache key alone
    for i, (t, filt) in enumerate([(0, 0), (4, 0)]):
        pass
    add("cfg-same-0", forms=frm({"filter": 0}), globals={"nGlobalAttackTypeMME": 0, "sGlobalAttackConfig": "0", "sCharDamageVsMonsterConfig": "marker"},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig", "cdv": "%g.sCharDamageVsMonsterConfig"})
    add("cfg-same-5", forms=frm({"filter": 0}), globals={"nGlobalAttackTypeMME": 5, "nGlobalAttackManualP": 7, "nGlobalAttackManualM": -2,
                                                         "sGlobalAttackConfig": "5_7_-2", "sCharDamageVsMonsterConfig": "marker"},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig", "cdv": "%g.sCharDamageVsMonsterConfig"})
    # class stealth flag, weapons and rounding in the key
    add("cfg-edge-stealth", forms=frm(rand_p(filter=1, cs19="2", tag={7: "20.5", 11: "9", 30: "1", 10: "77", 13: "3", 14: "1", 15: "4", 19: "30"})),
        globals={"nGlobalAttackTypeMME": 1, "nGlobalCharWeaponNumber": [364, 0], "nGlobalCharQnDbonus": 5},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig"})
    add("cfg-edge-ovf", forms=frm({"filter": 0}),
        globals={"nGlobalAttackTypeMME": 4, "nGlobalAttackMA": 1, "nGlobalCharWeaponPunchSkill": [2000000000, 2000000000],
                 "sGlobalAttackConfig": "keep", "sCharDamageVsMonsterConfig": "marker"},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig", "cdv": "%g.sCharDamageVsMonsterConfig"})
    add("cfg-edge-nosel", forms=frm({"filter": 1, "level": "10", "cls": None}),
        globals={"nGlobalAttackTypeMME": 1, "sGlobalAttackConfig": "keep", "sCharDamageVsMonsterConfig": "marker"},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig", "cdv": "%g.sCharDamageVsMonsterConfig"})
    add("cfg-edge-encum", forms=frm({"filter": 1, "level": "10", "cls": 4, "enc": ["333", "1000"]}),
        globals={"nGlobalAttackTypeMME": 2, "nGlobalAttackSpellNum": 1},
        proc="SetCurrentAttackTypeConfig", args=[], read={"cfg": "%g.sGlobalAttackConfig"})

    # ============================================================ SetCharDefenseDescription
    def def_tags(**kw):
        t = {2: tag(0, 300), 3: tag(0, 300), 8: tag(0, 120), 25: tag(-20, 80), 26: tag(-20, 80), 27: tag(-20, 80),
             28: tag(-20, 80), 29: tag(-20, 80)}
        t.update(kw)
        return t
    for i in range(10):
        p = {"filter": rng.pick([0, 1]), "level": s(rng.int(1, 200)), "cls": rng.pick([1, 4, 7, 12]),
             "enc": [s(rng.int(0, 400)), s(rng.pick([50, 300, 1000]))], "tag": def_tags(),
             "mr": rng.pick(["", "0", "55", "x", "12.5", " 7"]), "anti": rng.pick([0, 1])}
        g = {"sGlobalCharDefenseDescription": rng.pick(["", "old"]), "bDontPromptCalcCharMonsterDamage": True}
        add("def-%d" % i, forms=frm(p), globals=g, proc="SetCharDefenseDescription", args=[],
            read={"desc": "%g.sGlobalCharDefenseDescription", "dont": "%g.bDontPromptCalcCharMonsterDamage"})
    add("def-nosel", forms=frm({"filter": 1, "level": "5", "cls": None, "tag": def_tags(), "mr": "9"}),
        globals={"sGlobalCharDefenseDescription": "old", "bDontPromptCalcCharMonsterDamage": True},
        proc="SetCharDefenseDescription", args=[],
        read={"desc": "%g.sGlobalCharDefenseDescription", "dont": "%g.bDontPromptCalcCharMonsterDamage"})
    # unchanged description keeps the prompt flag: compute the key by hand for a simple state
    add("def-same", forms=frm({"filter": 0, "cls": 4, "enc": ["0", "1000"], "tag": {2: "10", 3: "5", 8: "20", 25: "1", 26: "2", 27: "3", 28: "4", 29: "5"}, "mr": "30", "anti": 1}),
        globals={"sGlobalCharDefenseDescription": "0_10_5_30_20_1_4_3_1_5_2_0", "bDontPromptCalcCharMonsterDamage": True},
        proc="SetCharDefenseDescription", args=[],
        read={"desc": "%g.sGlobalCharDefenseDescription", "dont": "%g.bDontPromptCalcCharMonsterDamage"})

    # ============================================================ RefreshCombatHealingValues / GetCurrentAttackName
    heal_reads = {"value": "%g.nGlobalAttackHealValue", "cost": "%g.nGlobalAttackHealCost",
                  "lvl": "%g.nGlobalAttackHealSpellLVL", "rounds": "%g.nGlobalAttackHealRounds"}

    def heal_g(**over):
        g = {
            "nGlobalAttackHealType": rng.pick([0, 1, 2, 3, 3, 2, 4, 5]),
            "nGlobalAttackHealSpellNum": rng.pick(HEAL_SPELLS + [0, BAD_SPELL]),
            "nGlobalAttackHealSpellLVL": rng.pick([-5, 0, 5, 20, 40, 12000]),
            "nGlobalAttackHealRounds": rng.pick([-1, 0, 1, 2, 3, 7, 50, 51, 100]),
            "nGlobalAttackHealManual": rng.pick([0, 500, -4, 2000000]),
            "nGlobalAttackHealValue": 777, "nGlobalAttackHealCost": 3.5,
            "nNMRVer": rng.pick([1.7, 1.83, 1.84]),
            "bGlobalAttackUseMeditate": rng.chance(1, 3),
            "gAvgLevelMaxAllStats": 0.0,
        }
        g.update(over)
        return g

    for i in range(32):
        filt = rng.pick([0, 1, 1])
        lair = rng.chance(1, 4)
        p = rand_p(filter=filt, lair=lair, lairtxt={0: rng.pick(["0", "1", "2", "5"])},
                   mdmg=rng.pick(["", "35", "-9", "3000000000"]), level=s(rng.pick([1, 8, 15, 30, 60, 255])))
        p["tag"][33] = rng.pick(["0", "25", "60", "-10", "200"])
        add("heal-%d" % i, forms=frm(p), globals=heal_g(), proc="RefreshCombatHealingValues", args=[], read=heal_reads)
    # a spell with a duration, rounds below / above it
    for i, (sp, rounds, lvl) in enumerate([(138, 1, 12), (138, 30, 12), (138, 60, 12), (138, 100, 12), (747, 50, 20), (747, 5, 0), (831, 61, 5), (138, 2, 40)]):
        p = rand_p(filter=1, lair=False, level="30")
        add("heal-dur-%d" % i, forms=frm(p),
            globals=heal_g(nGlobalAttackHealType=rng.pick([2, 3]), nGlobalAttackHealSpellNum=sp, nGlobalAttackHealRounds=rounds,
                           nGlobalAttackHealSpellLVL=lvl),
            proc="RefreshCombatHealingValues", args=[], read=heal_reads)
    # party mode returns early with the monster damage as the heal value
    for i, (t0, md) in enumerate([("3", "45"), ("2", "0"), ("1", "99"), ("4", "2.6")]):
        p = rand_p(filter=1, lair=True, lairtxt={0: t0}, mdmg=md)
        add("heal-party-%d" % i, forms=frm(p), globals=heal_g(nGlobalAttackHealType=2, nGlobalAttackHealSpellNum=17),
            proc="RefreshCombatHealingValues", args=[], read=heal_reads)
    add("heal-cost-clamp", forms=frm(rand_p(filter=1, level="40")),
        globals=heal_g(nGlobalAttackHealType=3, nGlobalAttackHealSpellNum=1039, nGlobalAttackHealRounds=1, nGlobalAttackHealSpellLVL=99),
        proc="RefreshCombatHealingValues", args=[], read=heal_reads)
    add("heal-cost-small", forms=frm(rand_p(filter=1, level="40")),
        globals=heal_g(nGlobalAttackHealType=2, nGlobalAttackHealSpellNum=13, nGlobalAttackHealRounds=50, nGlobalAttackHealSpellLVL=99),
        proc="RefreshCombatHealingValues", args=[], read=heal_reads)
    add("heal-nosel", forms=frm({"filter": 1, "level": "20", "cls": None, "race": 1}),
        globals=heal_g(nGlobalAttackHealType=2, nGlobalAttackHealSpellNum=17, nGlobalAttackHealRounds=2),
        proc="RefreshCombatHealingValues", args=[], read=heal_reads)

    name_reads = dict(heal_reads)
    for i in range(36):
        t = rng.int(0, 7)
        filt = rng.pick([0, 1])
        lair = rng.chance(2, 3)
        p = rand_p(filter=filt, lair=lair, lairtxt={0: rng.pick(["0", "1", "3"])}, mdmg=rng.pick(["", "40"]),
                   level=s(rng.pick([5, 20, 40, 70])))
        g = heal_g(
            nGlobalAttackTypeMME=t, nGlobalAttackSpellNum=rng.pick(ATTACK_SPELLS + [0, BAD_SPELL]),
            nGlobalAttackSpellLVL=rng.pick([0, 9, 25]), nGlobalAttackMA=rng.pick([0, 1, 2, 3]),
            nGlobalAttackManualP=rng.pick([0, 8, -4]), nGlobalAttackManualM=rng.pick([0, 9, -4]),
            bGlobalAttackBackstab=rng.chance(1, 3), nGlobalAttackBackstabWeapon=rng.pick([0] + WPN[:5]),
            bHideRecordNumbers=rng.chance(1, 2), bGlobal_cephRecoveryOnly=rng.chance(1, 5))
        g["nEquippedItem"] = [0] * 16 + [rng.pick([0, 0] + WPN)] + [0] * 3
        add("name-%d" % i, forms=frm(p), globals=g, proc="GetCurrentAttackName", args=[rng.chance(1, 3)], read=name_reads)

    # ============================================================ alignment and ward
    for i, (filt, al) in enumerate([(1, 3), (1, 2), (1, 0), (1, 4), (1, -1), (0, 3), (0, 2)]):
        add("evil-%d" % i, forms=frm({"filter": filt, "align": al}), proc="bCharIsEvilAligned", args=[])
        add("ward-%d" % i, forms=frm({"filter": filt, "align": al}), globals={"nGlobalCharVileWard": 35 + i},
            proc="GetCharActiveVileWard", args=[])

    # ============================================================ learned spells
    def learned(n, cls):
        a = [0] * 100
        for k in range(n):
            a[k] = 10 + 7 * k
        return a

    for i in range(6):
        add("learn-clear-%d" % i, forms=frm({"filter": 1, "cls": 4}),
            globals={"nLearnedSpells": learned(rng.int(0, 100), 4), "nLearnedSpellClass": rng.pick([0, 4, 9])},
            proc="ClearLearnedSpells", args=[], read={"spells": "%g.nLearnedSpells", "cls": "%g.nLearnedSpellClass"})
    ls_reads = {"spells": "%g.nLearnedSpells", "cls": "%g.nLearnedSpellClass"}
    for i in range(7):
        n = rng.pick([0, 3, 40, 99, 100])
        cls = rng.pick([1, 4, 12, 0])
        add("learn-add-%d" % i, forms=frm({"filter": rng.pick([0, 1]), "cls": cls}),
            globals={"nLearnedSpells": learned(n, cls), "nLearnedSpellClass": rng.pick([0, cls, 7])},
            proc="LearnSpell", args=[{"ref": "long", "value": rng.pick([13, 27, 5, 1, 0, 32767])}], read=ls_reads)
    add("learn-add-nosel", forms=frm({"filter": 1, "cls": None}),
        globals={"nLearnedSpells": learned(3, 0), "nLearnedSpellClass": 4},
        proc="LearnSpell", args=[{"ref": "long", "value": 27}], read=ls_reads)
    for i in range(5):
        n = rng.pick([3, 10, 60])
        add("learn-del-%d" % i, forms=frm({"filter": 1, "cls": 4}),
            globals={"nLearnedSpells": learned(n, 4), "nLearnedSpellClass": 4},
            proc="UnLearnSpell", args=[{"ref": "long", "value": rng.pick([10, 17, 24, 59, 13])}], read=ls_reads)
    for i, sp in enumerate([10, 17, 13, 0, 31, 5]):
        add("learn-toggle-%d" % i, forms=frm({"filter": 1, "cls": rng.pick([4, 12])}),
            globals={"nLearnedSpells": learned(rng.pick([2, 8, 100]), 4), "nLearnedSpellClass": rng.pick([0, 4])},
            proc="LearnOrUnlearnSpell", args=[{"ref": "long", "value": sp}], read=ls_reads)

    # ============================================================ table
    need_spells = set(HEAL_SPELLS + ATTACK_SPELLS)
    table = {
        "id": "table",
        "spell_fields": SPELL_FIELDS + ["AbilitiesFlat"],
        "item_fields": ITEM_FIELDS + ["AbilityTriples(slot,ability,value)"],
        "class_fields": ["Number", "CombatLVL", "AbilitiesFlat"],
        "spells": [spells[n] for n in sorted(need_spells) if n in spells],
        "items": [items[n] for n in sorted(used_items) if n in items],
        "classes": classes,
    }
    cases.insert(0, table)

    scn = {
        "family": "character",
        "database": True,
        "description": "Character profile and attack config (FAM-05 modMain half, stock): PopulateCharacterProfile, SetCurrentAttackTypeConfig, SetCharDefenseDescription, RefreshCombatHealingValues, GetCurrentAttackName, bCharIsEvilAligned, GetCharActiveVileWard and the learned-spell list.",
        "notes": [
            "Generated by tools/oracle/gen/character.py. The case 'table' carries the Spells, Items and Classes rows the other cases touch (fields named in the case); the Rust test needs no database.",
            "frmMain is replaced whole by each case's 'forms'; a control not listed reads as the VB default. Control .Tag/.Caption values are the text CalcCharacterStats and the Char-tab refreshers leave there; the Rust test reads them with Val().",
            "Class and race combos are one-item lists with ListIndex 0; StrictItemData with ListIndex -1 makes ItemData(-1) raise error 381 as VB6 does (nothing selected).",
            "PopulateCharacterProfile's ByRef tCharacterProfile argument is 'out': its final fields are in outputs['result.refs'][0] (lower-cased Type member names).",
            "No stock item has ability 22 (Accuracy) in its slots, so the ItemHasAbility(w, 22) adjustment is always 'not found' in these cases.",
            "RefreshLearnedSpellColors (UI) is a recorded no-op; the nChar*VsMonster arrays are allocated so ClearSavedDamageVsMonster runs (sCharDamageVsMonsterConfig then holds the previous sGlobalAttackConfig, as in MME).",
        ],
        "tolerance": {"default": 0},
        "globals": {
            "nCharDamageVsMonster": [0], "nCharFirstRoundDamageVsMonster": [0], "nCharMinRoundDamageVsMonster": [0],
            "nCharSurpriseDamageVsMonster": [0], "nCharSurpriseMinDamageVsMonster": [0],
            "nCharSurpriseChanceVsMonster": [0],
        },
        "cases": cases,
    }
    out = os.path.join(ROOT, "fixtures", "scenarios", "character.json")
    with open(out, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("wrote %s: %d cases, %d bytes" % (out, len(cases), os.path.getsize(out)))


if __name__ == "__main__":
    main()

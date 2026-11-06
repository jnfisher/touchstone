#!/usr/bin/env python3
"""Writes fixtures/scenarios/filters_sm.json (FAM-23, spells and monsters; card P3-30).

    python3 -I tools/oracle/gen/filters_sm.py [DB.sqlite]

frmMain FilterSpells and FilterMonsters, cut out of the form (tools/oracle/frm.py) with the helpers they call. The lists they
fill are recording ListViews read before the sort that follows: one row per AddSpell2LV / AddMonster2LV call, in the order MME
makes them. A spell row is its number. A monster row is its number with a tag holding the optional arguments of the call that
differ from AddMonster2LV's defaults (d damage out, e exp/hr, r time recovering, s surprise damage, m surprise min, c surprise
chance, f first round, n min round, x does-not-match) and the profile's HP (h) and HP regen (g) when the filter changed them.
In the cases that set `bRealAdd` the call goes on to the real AddMonster2LV and the row is the whole row.

The Monsters table is a stratified subset of the stock one (scenario key `db_only`; the full Summoned By text is 2 MB): the
subjects, plus every mob of the lairs their text names, so the lair averages are computed from complete lairs. Characters are
the pipeline scenario's (RefreshAll runs first); cases without one set the global filter's controls by hand. Reachability is
scripted (a monster whose number is a multiple of 7 is out of reach below level 30), because modReachability is not loaded.
Deterministic: sha256-seeded choices only.
"""
import copy
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys
from decimal import ROUND_HALF_EVEN, Decimal

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "filters_sm.json")
PIPE = json.load(open(os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")))
PAT = re.compile(r"\[([\d\-]+)\]\[(\d+)\]Group\(lair\): (\d+)/(\d+)", re.A)
N_SUBJECTS = int(os.environ.get("FSM_SUBJECTS", 150))


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def rnd(x, n):
    q = Decimal(1).scaleb(-n)
    return float(Decimal(repr(x)).quantize(q, rounding=ROUND_HALF_EVEN))


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)
    c.row_factory = sqlite3.Row
    lairs = {r["GroupIndex"]: dict(r) for r in c.execute("select * from Lairs order by GroupIndex")}
    mons = {r["Number"]: dict(r) for r in c.execute("select * from Monsters order by Number")}
    spells = {r["Number"]: dict(r) for r in c.execute("select * from Spells order by Number")}
    classes = {r["Number"]: dict(r) for r in c.execute("select * from Classes order by Number")}
    top = max(mons)

    def abil(m):
        return {m["Abil-%d" % i]: m["AbilVal-%d" % i] for i in range(10) if m["Abil-%d" % i]}

    def sb(m):
        return m["Summoned By"] or ""

    def entries(m):
        return [(g, int(r)) for g, r, _, _ in PAT.findall(sb(m))]

    def mobs_of(g):
        L = lairs.get(g)
        return [int(x) for x in L["MobList"].split(",") if x.strip().isdigit() and int(x) in mons] if L else []

    def attack_spells(m):
        out = set()
        for i in range(5):
            if m["AttType-%d" % i] == 2 and m["AttAcc-%d" % i]:
                out.add(m["AttAcc-%d" % i])
            if m["AttHitSpell-%d" % i]:
                out.add(m["AttHitSpell-%d" % i])
            if m["MidSpell-%d" % i]:
                out.add(m["MidSpell-%d" % i])
        if m["DeathSpell"]:
            out.add(m["DeathSpell"])
        return out

    # a monster is usable when it, and every mob of the lairs it names, has short text and few lair entries and so on
    # transitively: the subset must hold whole lairs (MME's GetDamageOutput moves the cursor to the first record when a
    # mob is missing, which loops FilterMonsters forever)
    elig = {n for n, m in mons.items() if len(sb(m)) < 700 and len(entries(m)) <= 3}
    changed = True
    while changed:
        changed = False
        for n in sorted(elig):
            if any(x not in elig for g, _ in entries(mons[n]) for x in mobs_of(g)):
                elig.discard(n)
                changed = True
    named = [n for n, m in mons.items() if n in elig and m["Name"] and not m["Name"].startswith("sdf")]
    S = {
        "plain": [n for n in named if not sb(mons[n]) and mons[n]["RegenTime"] == 0],
        "regen": [n for n in named if mons[n]["RegenTime"] > 0 and "(lair)" not in sb(mons[n])],
        "room": [n for n in named if "Room" in sb(mons[n]) and "(lair)" not in sb(mons[n])],
        "lair1": [n for n in named if len(entries(mons[n])) == 1 and mons[n]["RegenTime"] == 0],
        "lair2": [n for n in named if len(entries(mons[n])) == 2 and mons[n]["RegenTime"] == 0],
        "lair3": [n for n in named if 3 <= len(entries(mons[n])) <= 6 and mons[n]["RegenTime"] == 0],
        "lair_regen": [n for n in named if len(entries(mons[n])) >= 1 and mons[n]["RegenTime"] > 0],
        "magic": [n for n in named if abil(mons[n]).get(28, 0) > 0],
        "dodge": [n for n in named if abil(mons[n]).get(34, 0) > 0],
        "antimagic": [n for n in named if 51 in abil(mons[n])],
        "undead": [n for n in named if mons[n]["Undead"] == 1],
        "bsdef": [n for n in named if mons[n]["BSDefense"] > 0],
        "follow": [n for n in named if mons[n]["Follow%"] > 0],
        "limit": [n for n in named if mons[n]["GameLimit"] > 0],
        "lairexp": [n for n in named if mons[n]["AvgLairExp"] > 0],
        "coins": [n for n in named if mons[n]["R"] + mons[n]["P"] + mons[n]["G"] + mons[n]["S"] + mons[n]["C"] > 0],
        "runic": [n for n in named if mons[n]["R"] > 0 or mons[n]["P"] > 0],
        "align": [n for n in named if mons[n]["Align"] in (0, 1, 5, 6)],
        "spellatk": [n for n in named if attack_spells(mons[n])],
        "exp0": [n for n in named if mons[n]["EXP"] == 0],
        "expmulti": [n for n in named if mons[n]["ExpMulti"] != 1.0 and mons[n]["EXP"] > 0],
        "notingame": [n for n, m in mons.items() if not m["In Game"] and m["Name"] and len(sb(m)) < 700],
    }
    must = [n for n in (1086, 1087, 597) if n in mons]
    subjects = set(must)
    for k, n in (("plain", 14), ("regen", 14), ("room", 10), ("lair1", 12), ("lair2", 8), ("lair3", 4), ("lair_regen", 6),
                 ("magic", 8), ("dodge", 6), ("antimagic", 3), ("undead", 5), ("bsdef", 4), ("follow", 5), ("limit", 5),
                 ("lairexp", 5), ("coins", 5), ("runic", 4), ("align", 8), ("spellatk", 10), ("exp0", 3), ("expmulti", 5),
                 ("notingame", 3)):
        subjects.update(sorted(S[k], key=lambda x: H(k, x))[:n])
    subjects = set(sorted(subjects, key=lambda x: H("all", x))[:N_SUBJECTS]) | set(must)
    table_nums = set(subjects)
    for n in subjects:
        for g, _ in entries(mons[n]):
            table_nums.update(mobs_of(g))
    table_nums = sorted(table_nums)
    top = max(table_nums)
    lair_subjects = sorted(n for n in table_nums if mons[n]["In Game"] and (entries(mons[n]) or mons[n]["RegenTime"] > 0))

    sp_nums = set()
    for n in table_nums:
        sp_nums |= attack_spells(mons[n])
    sp_nums = {s for s in sp_nums if s in spells}
    more = True
    while more:
        more = False
        for s in list(sp_nums):
            e = abil(spells[s]).get(151, 0)
            if e and e in spells and e not in sp_nums:
                sp_nums.add(e)
                more = True

    # the spells: a cover of every value the filters look at, the pipeline's own, and the ones monsters cast
    pipe_sp = {r[0] for r in PIPE["data"]["spells"]}
    ABIL_PICK = (1, 2, 4, 6, 8, 13, 22, 23, 24, 29, 43, 54, 58, 68, 97, 98, 112, 110, 111, 113)

    def sp_features(s):
        a = abil(s)
        return ([("m", s["Magery"]), ("ml", s["MageryLVL"]), ("at", s["AttType"]), ("t", s["Targets"]), ("lr", s["Learnable"] > 0),
                 ("cl", bool(s["Classes"]) and s["Classes"] != "(*)"), ("star", s["Classes"] == "(*)"),
                 ("lf", len(s["Learned From"] or "") > 1), ("cb", len(s["Casted By"] or "") > 1),
                 ("mk", (s["Magery"], s["Learnable"] > 0, s["MageryLVL"]))] + [("ab", x) for x in ABIL_PICK if x in a])

    picked = set(pipe_sp) | set(sp_nums)
    seen = {}
    for n in sorted(spells, key=lambda x: H("spell", x)):
        for ft in sp_features(spells[n]):
            if seen.get(ft, 0) < 6:
                picked.add(n)
            seen[ft] = seen.get(ft, 0) + 1
    picked = sorted(n for n in picked if n in spells)
    lair_groups = sorted({g for n in table_nums for g, _ in entries(mons[n])})

    # ------------------------------------------------------------------ data for the Rust test
    def mon_row(n):
        m = mons[n]
        return ([n, m["Name"] or "", m["RegenTime"], m["EXP"], m["ExpMulti"], m["HP"], m["ArmourClass"], m["DamageResist"],
                 m["MagicRes"], m["BSDefense"], m["Undead"], m["AvgLairExp"], m["HPRegen"], m["AvgDmg"], m["DeathSpell"],
                 m["Follow%"], m["Align"], m["GameLimit"], m["R"], m["P"], m["G"], m["S"], m["C"], 1 if m["In Game"] else 0,
                 sb(m), [[i, m["Abil-%d" % i], m["AbilVal-%d" % i]] for i in range(10) if m["Abil-%d" % i]],
                 [[i, m["AttType-%d" % i], m["AttAcc-%d" % i], m["Att%%-%d" % i], m["AttTrue%%-%d" % i], m["AttHitSpell-%d" % i]]
                  for i in range(5) if m["AttType-%d" % i] or m["AttAcc-%d" % i] or m["AttHitSpell-%d" % i]],
                 [[i, m["MidSpell-%d" % i], m["MidSpell%%-%d" % i]] for i in range(5) if m["MidSpell-%d" % i]]])

    LAIR_FIELDS = ["GroupIndex", "MobList", "Mobs", "TotalLairs", "AvgDelay", "AvgWalk", "AvgExp", "AvgDmg", "AvgHP",
                   "AvgAC", "AvgDR", "AvgMR", "AvgDodge"]
    SP_FIELDS = ["Number", "ReqLevel", "Magery", "MageryLVL", "Learnable", "Targets", "AttType", "Dur", "DurIncLVLs", "DurInc"]

    def sp_row(n):
        s = spells[n]
        return ([s[k] for k in SP_FIELDS] + [len(s["Learned From"] or ""), len(s["Casted By"] or ""), s["Classes"] or "",
                                              [[s["Abil-%d" % i], s["AbilVal-%d" % i]] for i in range(10) if s["Abil-%d" % i]]])

    data = {
        "monster_fields": ["Number", "Name", "RegenTime", "EXP", "ExpMulti", "HP", "ArmourClass", "DamageResist", "MagicRes",
                           "BSDefense", "Undead", "AvgLairExp", "HPRegen", "AvgDmg", "DeathSpell", "Follow%", "Align",
                           "GameLimit", "R", "P", "G", "S", "C", "InGame", "SummonedBy", "Abilities(slot,ability,value)",
                           "Attacks(slot,type,acc,pct,truepct,hitspell)", "MidSpells(slot,spell,pct)"],
        "monsters": [mon_row(n) for n in table_nums],
        "lair_fields": LAIR_FIELDS,
        "lairs": [[lairs[g][k] for k in LAIR_FIELDS] for g in lair_groups if g in lairs],
        "spell_fields": SP_FIELDS + ["len(LearnedFrom)", "len(CastedBy)", "Classes", "Abilities(nonzero)"],
        "spells": [sp_row(n) for n in picked],
        "attack_spells": sorted(sp_nums),
    }

    # ------------------------------------------------------------------ globals shared by the monster cases
    def damage_array(salt, lo, hi, miss):
        a = [-1] * (top + 1)
        for n in table_nums:
            if miss == 1 or H(salt, "miss", n) % miss:
                a[n] = rnd(mons[n]["AvgDmg"] * (lo + (H(salt, "f", n) % int((hi - lo) * 10 + 1)) / 10.0) + (H(salt, "a", n) % 3) * 0.5, 1)
        return a

    possy = [0.0] * (top + 1)
    chance = [0.0] * (top + 1)
    for n, m in ((n, mons[n]) for n in table_nums):
        es = entries(m)
        if not es or "(lair)" not in sb(m):
            continue
        total, ch = 0.0, 0.0
        for g, r in es:
            total += r
            nm = len(mobs_of(g))
            if r > 0 and nm > 0:
                ch += rnd(1 - (1 - 1.0 / nm) ** r, 2)
        if total > 0:
            possy[n] = rnd(total / len(es), 1)
        chance[n] = rnd(ch / len(es), 2)

    # ------------------------------------------------------------------ the form code
    FRM = copy.deepcopy(PIPE["frm"])
    FRM["procs"] = FRM["procs"] + ["FilterSpells", "FilterMonsters", "txtMonsterEXP_Change"]
    FRM["stub_procs"] = FRM["stub_procs"] + [
        "LV_RefreshSort", "LV_Sort_ColumnClick", "lvSpells_ItemClick", "lvMonsters_ItemClick",
        "RefreshExtraMonsterFilterButton", "RefreshMonsterColors"]
    FRM["stub_replace"] = ["LoadResPicture", "LockWindowUpdate"]
    FRM["stub_false"] = ["FormIsLoaded"]
    FRM["me_is_form"] = True
    FRM["decls"] = FRM["decls"] + ["bAllowForceMonsterExpSort", "bAllowForceSpellCombatSort", "bMapCancelFind"]
    FRM["subs"] = {
        "FilterSpells": {
            "Call AddSpell2LV(lvSpells, tChar)":
                'Set oLI = lvSpells.ListItems.Add(): oLI.Text = tabSpells.Fields("Number")'},
        "FilterMonsters": {
            "Call AddMonster2LV(lvMonsters, tChar, nDamageOut, nPassEXP, nPassRecovery, _":
                "Call RecMon(nMonsterNum, tChar, nDamageOut, nPassEXP, nPassRecovery, _",
            "optMonsterFilter(0).Value = True": "optMonsterFilter(0).Value = True: optMonsterFilter(1).Value = False",
            "txtMonsterEXP.Text = 0": "txtMonsterEXP.Text = 0: txtMonsterEXP.Tag = 0",
            "If Len(Trim(txtMonsterEXP.Text)) = 0 Then txtMonsterEXP.Text = 0":
                "If Len(Trim(txtMonsterEXP.Text)) = 0 Then txtMonsterEXP.Text = 0: txtMonsterEXP.Tag = 0",
        },
    }
    FRM["subs_all"] = {"FilterMonsters": {
        "If x = vbYes Then Call CalculateMonsterDamageVsCharALL": "If x = vbYes Then nRecalcChar = nRecalcChar + 1",
        "Call CalculateMonsterDamageVsCharALL(True)": "nRecalcParty = nRecalcParty + 1",
    }}
    FRM["extra"] = [
        "Public Enum ListDataType", "ldtstring = 0", "ldtnumber = 1", "ldtdate = 2", "End Enum",
        "Public Const vbResBitmap = 0", "Public Const vbDefaultButton1 = 0",
        "Public nRecalcChar As Long", "Public nRecalcParty As Long", "Public bRealAdd As Boolean",
        "Public nRecHP0 As Double", "Public nRecReg0 As Double",
        "Public Sub RecInit(tChar As tCharacterProfile)", "nRecHP0 = tChar.nHP", "nRecReg0 = tChar.nHPRegen", "End Sub",
        "Public Sub SetAbil(ByVal y As Long, ByVal a As Long, ByVal op As Long, ByVal v As Long)",
        "filter_Monster_nAbilities(y, 0) = a", "filter_Monster_nAbilities(y, 1) = op", "filter_Monster_nAbilities(y, 2) = v",
        "End Sub",
        "Public Sub SetPrompts(ByVal sCfg As String, ByVal sDesc As String, ByVal bDC As Boolean, ByVal bPC As Boolean, ByVal bDP As Boolean)",
        "sMonsterDamageVsCharDefenseConfig = sCfg", "sGlobalCharDefenseDescription = sDesc",
        "bDontPromptCalcCharMonsterDamage = bDC", "bMonsterDamageVsPartyCalculated = bPC", "bDontPromptCalcPartyMonsterDamage = bDP",
        "End Sub",
        "Public Sub optMonsterFilter_Click(Index As Integer)", "Call SetCurrentAttackTypeConfig", "End Sub",
        "Public Sub ReachEnsureComputed()", "End Sub",
        "Public Function ReachIsMonsterOK(ByVal nMonsterNum As Long, ByVal nLevel As Long) As Boolean",
        "ReachIsMonsterOK = Not (nMonsterNum Mod 7 = 0 And nLevel < 30)", "End Function",
        "Public Sub RecMon(ByVal nMonsterNum As Long, tChar As tCharacterProfile, ByVal nDamageOut As Long, "
        "ByVal nPassEXP As Currency, ByVal nPassRecovery As Double, ByVal nSurpriseDamageOut As Long, "
        "ByVal nSurpriseMinDMG As Long, ByVal nSurpriseChance As Integer, ByVal nFirstRoundDMG As Long, "
        "ByVal nMinRoundDMG As Long, ByVal bDoesNotMatchFilter As Boolean)",
        "Dim oLI As ListItem, s As String",
        "If bRealAdd Then",
        "Call AddMonster2LV(lvMonsters, tChar, nDamageOut, nPassEXP, nPassRecovery, nSurpriseDamageOut, nSurpriseMinDMG, "
        "nSurpriseChance, nFirstRoundDMG, nMinRoundDMG, bDoesNotMatchFilter)",
        "Exit Sub", "End If",
        'If nDamageOut <> -9999 Then s = s & "d" & nDamageOut & ";"',
        'If nPassEXP <> -1 Then s = s & "e" & nPassEXP & ";"',
        'If nPassRecovery <> -1 Then s = s & "r" & nPassRecovery & ";"',
        'If nSurpriseDamageOut <> -9999 Then s = s & "s" & nSurpriseDamageOut & ";"',
        'If nSurpriseMinDMG <> 0 Then s = s & "m" & nSurpriseMinDMG & ";"',
        'If nSurpriseChance <> 0 Then s = s & "c" & nSurpriseChance & ";"',
        'If nFirstRoundDMG <> 0 Then s = s & "f" & nFirstRoundDMG & ";"',
        'If nMinRoundDMG <> 0 Then s = s & "n" & nMinRoundDMG & ";"',
        'If bDoesNotMatchFilter Then s = s & "x;"',
        'If tChar.nHP <> nRecHP0 Then s = s & "h" & tChar.nHP & ";"',
        'If tChar.nHPRegen <> nRecReg0 Then s = s & "g" & tChar.nHPRegen & ";"',
        "Set oLI = lvMonsters.ListItems.Add()", "oLI.Text = nMonsterNum", "oLI.Tag = s", "End Sub",
    ]

    # ------------------------------------------------------------------ controls
    PC = {x["id"]: x for x in PIPE["cases"]}
    CLASS_NUMS = sorted(classes)
    MAGERY_ABILS = [0, 1, 2, 3, 4, 5]
    ABIL_DATA = [0, -1] + sorted({a for n in sp_nums | set(spells) for a in abil(spells[n]) if a in
                                  (1, 2, 4, 6, 8, 13, 22, 23, 24, 29, 43, 54, 58, 68, 97, 98, 112, 110, 111, 113)})

    WARRIOR_DELTA = ('chkGlobalFilter', 'txtGlobalLevel', 'cmbGlobalClass')

    def base_forms():
        f = copy.deepcopy(PIPE["forms"]["frmMain"])
        f.update({
            "chkGlobalFilter": {"Value": 0},
            "txtGlobalLevel": arr({0: {"Text": "999"}, 1: {"Text": "999"}}),
            "cmbGlobalClass": arr({0: {"ListIndex": 0, "ItemData": [0] + CLASS_NUMS}}),
            "cmbGlobalAlignment": {"ListIndex": 0},
            "cmbSpellMagery": {"ListIndex": 0},
            "cmbSpellMageryLevel": {"ListIndex": 0},
            "chkSpellOptions": arr({0: {"Value": 0}, 1: {"Value": 0}, 2: {"Value": 0}}),
            "cmbSpellAttackType": {"ListIndex": 0},
            "cmbSpellTarget": {"ListIndex": 0},
            "cmbSpellContainsAbil": {"ListIndex": 0, "ItemData": ABIL_DATA},
            "lvSpells": {"$recordlv": []},
            "lvMonsters": {"$recordlv": []},
            "lvMonsterDetail": {"$recordlv": []},
            "cmdNav": arr({k: {} for k in range(10)}),
            "cmdFilter": arr({k: {} for k in range(10)}),
            "optMonsterFilter": arr({0: {"Value": True}, 1: {"Value": False}}),
            "cmbMonsterRegen": {"ListIndex": 0},
            "txtMonsterRegen": {"Text": "999"},
            "txtMonsterHP": {"Text": "99999"},
            "txtMonsterEXP": {"Text": "0", "Tag": 0},
            "txtMonsterDamage": {"Text": "99999"},
            "chkMonMagic": {"Value": 0},
            "txtMonMagic": {"Text": "999"},
            "txtMonsterDamageOUT": arr({0: {"Text": "99999"}, 1: {"Text": "99999"}}),
            "txtMonsterLairFilter": arr({k: {"Text": t} for k, t in enumerate(["1", "60", "20", "50", "50", "400", "0", "12", "250", "1"])}),
        })
        wf = PIPE["cases"][[x["id"] for x in PIPE["cases"]].index("ladder-c1-l50")]["forms"]["frmMain"]
        for k, v in wf.items():
            if k not in WARRIOR_DELTA:
                f[k] = v
        return f

    cases = []

    def call(proc, *args):
        return {"call": proc, "args": list(args)}

    def spell_case(cid, forms=None, glob_=None, remove=False, msgbox=None):
        fm = dict(forms or {})
        g = {"bOnlyInGame": True}
        g.update(glob_ or {})
        cases.append({"id": cid, "forms": {"frmMain": fm}, "globals": g,
                      "steps": [call("FilterSpells", True)] if remove else [call("FilterSpells")],
                      "read": {"spells": "%lvn.lvSpells", "nav": "%c.cmdNav(2).Caption"}})

    def sp(**kw):
        f = {}
        if "magery" in kw:
            f["cmbSpellMagery"] = {"ListIndex": kw["magery"]}
        if "mlvl" in kw:
            f["cmbSpellMageryLevel"] = {"ListIndex": kw["mlvl"]}
        if "learn" in kw:
            f["chkSpellOptions"] = arr({0: {"Value": 0}, 1: {"Value": kw["learn"]}, 2: {"Value": 0}})
        if "att" in kw:
            f["cmbSpellAttackType"] = {"ListIndex": kw["att"]}
        if "target" in kw:
            f["cmbSpellTarget"] = {"ListIndex": kw["target"]}
        if "abil" in kw:
            f["cmbSpellContainsAbil"] = {"ListIndex": ABIL_DATA.index(kw["abil"]), "ItemData": ABIL_DATA}
        return f

    def gl(cls=0, level=999, align=0, lvl1=None, on=1):
        ds = [0] + CLASS_NUMS
        return {"chkGlobalFilter": {"Value": on}, "cmbGlobalClass": arr({0: {"ListIndex": ds.index(cls), "ItemData": ds}}),
                "txtGlobalLevel": arr({0: {"Text": str(level)}, 1: {"Text": str(level if lvl1 is None else lvl1)}}),
                "cmbGlobalAlignment": {"ListIndex": align}}

    def merged(*p):
        f = {}
        for x in p:
            f.update(x)
        return f

    # spells without a character
    spell_case("sp-default")
    spell_case("sp-default-all", glob_={"bOnlyInGame": False})
    spell_case("sp-remove", sp(magery=2, learn=1), remove=True)
    for m in range(1, 6):
        spell_case("sp-magery-%d" % m, sp(magery=m))
    for lv in range(1, 4):
        spell_case("sp-mlvl-%d" % lv, sp(mlvl=lv))
        spell_case("sp-magery-2-mlvl-%d" % lv, sp(magery=2, mlvl=lv))
    spell_case("sp-learn", sp(learn=1))
    spell_case("sp-learn-magery-5", sp(learn=1, magery=5))
    spell_case("sp-learn-magery-3", sp(learn=1, magery=3))
    spell_case("sp-learn-kai-off", sp(learn=1, magery=5), glob_={"bDisableKaiAutolearn": True})
    for a in range(1, 8):
        spell_case("sp-att-%d" % a, sp(att=a))
    for t in range(1, 6):
        spell_case("sp-target-%d" % t, sp(target=t))
    for k in range(2, len(ABIL_DATA)):
        spell_case("sp-abil-%d" % ABIL_DATA[k], sp(abil=ABIL_DATA[k]))
    spell_case("sp-combo-1", sp(magery=1, mlvl=2, att=5, target=3, learn=1))
    spell_case("sp-combo-2", sp(magery=3, att=5, target=3, abil=ABIL_DATA[3]))
    # the global filter by hand
    combos = [(cls, lv, al) for cls in (1, 3, 7, 12, 15) for lv in (1, 10, 40, 100) for al in (0, 1, 3)]
    for cls, lv, al in combos[::2]:
        spell_case("sp-g-c%d-l%d-a%d" % (cls, lv, al), gl(cls, lv, al))
    for cls, lv, al in combos[1::5]:
        spell_case("sp-g-learn-c%d-l%d-a%d" % (cls, lv, al), merged(gl(cls, lv, al), sp(learn=1)))
    for m in (1, 2, 3, 4, 5):
        for cls in (2, 12, 15):
            spell_case("sp-g-magery-%d-c%d" % (m, cls), merged(gl(cls, 50, 0), sp(magery=m)))
    spell_case("sp-g-anyclass-magery-2", merged(gl(0, 50, 0), sp(magery=2)))
    spell_case("sp-g-level-zero", gl(7, 0, 0, lvl1=0))
    spell_case("sp-g-level1-zero", gl(7, 30, 0, lvl1=0))
    spell_case("sp-g-off-with-values", gl(7, 30, 2, on=0))
    spell_case("sp-g-remove", gl(7, 30, 2), remove=True)
    spell_case("sp-g-v17", merged(gl(12, 50, 0), sp(magery=2)), glob_={"nNMRVer": 1.6})
    spell_case("sp-g-v18", merged(gl(12, 50, 0), sp(magery=2)), glob_={"nNMRVer": 1.75})
    for base in ("ladder-c1-l50", "ladder-c12-l50", "ladder-c15-l50"):
        cf = copy.deepcopy(PC[base]["forms"]["frmMain"])
        cf["txtGlobalLevel"] = arr({0: cf["txtGlobalLevel"]["$array"]["0"], 1: {"Text": "50"}})
        spell_case("sp-char-%s" % base, merged(cf))
        spell_case("sp-char-%s-learn-mage" % base, merged(cf, sp(learn=1, magery=2)))
        spell_case("sp-char-%s-target" % base, merged(cf, sp(target=3, att=5)))

    # ------------------------------------------------------------------ monsters
    CH = {
        "warrior": ("ladder-c1-l50", {"nGlobalAttackTypeMME": 1, "bGlobalAttackBackstab": False}),
        "mage": ("ladder-c12-l50", {"nGlobalAttackTypeMME": 2}),
        "mystic": ("ladder-c15-l50", {}),
    }

    def lair_forms(size=1, anti=0, hp="60", regen="20"):
        lt = {0: str(size), 1: hp, 2: regen, 3: "50", 4: "50", 5: "400", 6: str(anti), 7: "12", 8: "250", 9: "1"}
        return {"optMonsterFilter": arr({0: {"Value": False}, 1: {"Value": True}}),
                "txtMonsterLairFilter": arr({k: {"Text": v} for k, v in lt.items()}),
                "txtMonsterDamageOUT": arr({0: {"Text": "140"}, 1: {"Text": "60"}})}

    def exp_forms(text):
        return {"txtMonsterEXP": {"Text": text, "Tag": 0}}

    def mon(cid, who=None, forms=None, glob_=None, remove=False, extras=None, abil_filters=(), msgbox=None, real=False,
            level=None, version=None, prompts=("", "", True, False, True)):
        """One FilterMonsters case. extras: filter_Monster_* globals; abil_filters: (ability, op, value) up to three."""
        base, gl_ = CH[who or "warrior"]
        g = {} if who in (None, "warrior") else json.loads(json.dumps(PC[base]["globals"]))
        if who not in (None, "warrior"):
            g.update(gl_)
        g.update(glob_ or {})
        if version:
            g["nNMRVer"] = version
        if real:
            g["bRealAdd"] = True
        f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
        if who in (None, "warrior"):
            f = {k: v for k, v in f.items() if k in WARRIOR_DELTA}
        if who is None:
            f.pop("chkGlobalFilter", None)
        if level is not None:
            f["txtGlobalLevel"] = arr({0: {"Text": str(level)}, 1: {"Text": str(level)}})
        f.update(forms or {})
        if extras:
            g.update({"filter_Monster_" + k: v for k, v in extras.items()})
        steps = [call("RefreshAll"), call("SetCurrentAttackTypeConfig"), call("LoadLairInfo")]
        steps.append({"call": "PopulateCharacterProfile", "args": [{"udt": "tCharacterProfile", "fields": {}, "out": True}],
                      "as": "tchar"})
        steps.append({"call": "RecInit", "args": [{"saved": "tchar"}]})
        for y, (a, op, v) in enumerate(abil_filters):
            steps.append(call("SetAbil", y, a, op, v))
        steps.append(call("SetPrompts", *prompts))
        steps.append(call("txtMonsterEXP_Change"))
        steps.append(call("FilterMonsters", True) if remove else call("FilterMonsters"))
        case = {"id": cid, "forms": {"frmMain": f}, "globals": g, "steps": steps,
                "read": {"mons": "%lv.lvMonsters" if real else "%lvn.lvMonsters", "nav": "%c.cmdNav(8).Caption", "recalc": "%g.nRecalcChar",
                         "recalc_party": "%g.nRecalcParty"}}
        if msgbox:
            case["msgbox"] = msgbox
        cases.append(case)

    E = lambda **kw: dict(kw, bExtrasEnabled=True)       # extras on, plus the one setting
    # solo, no character
    mon("m-default", None)
    mon("m-default-all-items", None, glob_={"bOnlyInGame": False})
    mon("m-remove", None, forms={"txtMonsterRegen": {"Text": "5"}, "txtMonsterHP": {"Text": "100"}, "txtMonsterEXP": {"Text": "5K", "Tag": 5000}},
        remove=True)
    mon("m-remove-extras", None, extras=E(nArmourClass=5, bShowAll=True), remove=True)
    mon("m-remove-from-lair", "warrior", forms=lair_forms(1), remove=True)
    for op, t in ((0, "0"), (0, "10"), (0, "60"), (1, "10"), (1, "100"), (1, "0"), (0, "abc"), (2, "5")):
        mon("m-regen-%d-%s" % (op, t), None, forms={"cmbMonsterRegen": {"ListIndex": op}, "txtMonsterRegen": {"Text": t}})
    for t in ("0", "20", "100", "500", "2000"):
        mon("m-hp-%s" % t, None, forms={"txtMonsterHP": {"Text": t}})
    for t in ("0", "5", "40", "150"):
        mon("m-damage-%s" % t, None, forms={"txtMonsterDamage": {"Text": t}})
    for t in ("1", "3", "10"):
        mon("m-magic-%s" % t, None, forms={"chkMonMagic": {"Value": 1}, "txtMonMagic": {"Text": t}})
    mon("m-magic-off-text", None, forms={"txtMonMagic": {"Text": "0"}})
    for t in ("100", "5K", "20K", "2.5M"):
        mon("m-exp-%s" % t, None, forms=exp_forms(t))
    mon("m-blank-boxes", None, forms={"txtMonsterRegen": {"Text": " "}, "txtMonsterHP": {"Text": ""}, "txtMonsterDamage": {"Text": ""},
                                      "txtMonMagic": {"Text": ""}, "txtMonsterEXP": {"Text": "", "Tag": 0}})
    mon("m-exp-multi", None, forms=exp_forms("200"), glob_={"UseExpMulti": True})
    # extras, one at a time
    for k, v in (("nArmourClass", 20), ("nArmourClass", 0), ("nDamageResist", 10), ("nMagicRes", 20), ("nBSDef", 20),
                 ("nGameLimit", 5), ("nAvgLairExp", 5000), ("nAtkAccuracyMaj", 100), ("nAtkAccuracyMax", 150), ("nDodge", 20),
                 ("nNumLairs", 3), ("nNumMobsLTE", 2), ("nNumMobsGTE", 3), ("nNumMobsGTE", 1.5), ("nFollowGTE", 50), ("nFollowLTE", 20),
                 ("bIsUndead", True), ("bIsNonHostile_vEvil", True), ("bIsNonHostile_vNG", True),
                 ("bDropsCash", True), ("bDropsR", True), ("bDropsP", True), ("bDropsG", True), ("bDropsS", True),
                 ("bAtkNoPoison", True), ("bAtkNoConfusion", True), ("bAtkNoFear", True)):
        mon("m-x-%s-%s" % (k, v), None, extras=E(**{k: v}))
    mon("m-x-disabled", None, extras={"bExtrasEnabled": False, "nArmourClass": 5, "bIsUndead": True, "bShowAll": True})
    mon("m-x-reach-10", None, extras=E(bReachableOnly=True), level=10)
    mon("m-x-reach-50", None, extras=E(bReachableOnly=True), level=50)
    mon("m-x-reach-showall", None, extras=E(bReachableOnly=True, bShowAll=True), level=10)
    for a, op, v in ((28, 1, 1), (28, 0, 0), (28, 0, 3), (34, 1, 10), (51, 1, 1), (51, 0, 1), (4, 1, 1), (135, 0, 5)):
        mon("m-abil-%d-%d-%d" % (a, op, v), None, extras=E(), abil_filters=[(a, op, v)])
    mon("m-abil-three", None, extras=E(), abil_filters=[(28, 1, 1), (34, 0, 30), (51, 0, 0)])
    mon("m-abil-showall", None, extras=E(bShowAll=True), abil_filters=[(28, 1, 1)])
    mon("m-showall-ac", None, extras=E(bShowAll=True, nArmourClass=10))
    mon("m-showall-regen-hp", None, extras=E(bShowAll=True), forms={"cmbMonsterRegen": {"ListIndex": 1}, "txtMonsterRegen": {"Text": "10"}})
    mon("m-atleast-lairs", None, extras=E(bAtLeastLairs=True, nAtLeastLairs=2, nNumMobsLTE=4))
    mon("m-atleast-lairs-gte", None, extras=E(bAtLeastLairs=True, nAtLeastLairs=1, nNumMobsGTE=2))
    mon("m-atleast-lairs-v182", None, extras=E(bAtLeastLairs=True, nAtLeastLairs=1, nNumMobsGTE=2), version=1.82)
    mon("m-combo-1", None, extras=E(nArmourClass=40, bDropsG=True, nDodge=30),
        forms={"cmbMonsterRegen": {"ListIndex": 1}, "txtMonsterRegen": {"Text": "1"}, "txtMonsterHP": {"Text": "600"}})
    mon("m-combo-2", None, extras=E(bIsUndead=True, nNumLairs=1, nMagicRes=50), forms={"chkMonMagic": {"Value": 1}, "txtMonMagic": {"Text": "5"}})
    mon("m-v182", None, extras=E(nNumLairs=2, nNumMobsGTE=2, nBSDef=10), version=1.82)
    mon("m-v175", None, extras=E(nNumLairs=2, nNumMobsGTE=2, nNumMobsLTE=5), version=1.75)
    mon("m-v170-exp", None, forms=exp_forms("100"), version=1.7)

    # characters, solo
    for who in ("warrior", "mage", "mystic"):
        mon("m-%s-default" % who, who)
        mon("m-%s-damage" % who, who, forms={"txtMonsterDamage": {"Text": "30"}})
        mon("m-%s-exp-solo" % who, who, forms=exp_forms("2K"))

    # lair mode, party of one
    for who in (None, "warrior", "mage", "mystic"):
        n = who or "nochar"
        mon("l-%s-default" % n, who, forms=lair_forms(1))
        mon("l-%s-hp" % n, who, forms=merged(lair_forms(1), {"txtMonsterHP": {"Text": "150"}}))
        mon("l-%s-damage" % n, who, forms=merged(lair_forms(1), {"txtMonsterDamage": {"Text": "20"}}))
    for who in (None, "mage", "mystic"):
        n = who or "nochar"
        for t in ("1", "10", "100", "20K", "1M"):
            mon("l-%s-exp-%s" % (n, t), who, forms=merged(lair_forms(1), exp_forms(t)))
    mon("l-nochar-exp-showall", None, forms=merged(lair_forms(1), exp_forms("50")), extras=E(bShowAll=True))
    mon("l-mystic-exp-showall", "mystic", forms=merged(lair_forms(1), exp_forms("5K")), extras=E(bShowAll=True))
    mon("l-warrior-exp-extras", "warrior", forms=merged(lair_forms(1), exp_forms("5K")), extras=E(nNumLairs=2, bShowAll=True))
    mon("l-warrior-extras", "warrior", forms=lair_forms(1), extras=E(nNumMobsGTE=2, nNumLairs=2, nGameLimit=10, nAvgLairExp=1000))
    mon("l-warrior-showall-hard", "warrior", forms=lair_forms(1), extras=E(bShowAll=True))
    mon("l-warrior-hard-skips", "warrior", forms=lair_forms(1))
    mon("l-warrior-recovery-only", "warrior", forms=merged(lair_forms(1), exp_forms("5K"), {"txtMonsterDamage": {"Text": "30"}}),
        glob_={"bGlobal_cephRecoveryOnly": True})
    mon("l-warrior-v182", "warrior", forms=merged(lair_forms(1), exp_forms("1K")), version=1.82)
    mon("l-warrior-v182-hp", "warrior", forms=merged(lair_forms(1), {"txtMonsterHP": {"Text": "80"}}), version=1.82)
    # lair mode, party
    for who in ("warrior", "mage"):
        for size, anti in ((2, 0), (4, 2)):
            mon("p-%s-%d" % (who, size), who, forms=lair_forms(size, anti))
            mon("p-%s-%d-exp" % (who, size), who, forms=merged(lair_forms(size, anti), exp_forms("3K")))
    mon("p-nochar-3-exp", None, forms=merged(lair_forms(3, 1), exp_forms("1K")))
    mon("p-warrior-6-damage", "warrior", forms=merged(lair_forms(6, 0), {"txtMonsterDamage": {"Text": "40"}}))
    # questions
    dlg = {"vs_char": ("A", "B", False, False, True), "no_char": ("", "B", False, False, True), "same": ("B", "B", False, False, True),
           "party_done": ("", "", True, True, False), "party_new": ("", "", True, False, False)}
    for name in ("vs_char", "no_char", "same"):
        for ans in (6, 7, 2):
            mon("q-%s-%d" % (name, ans), "warrior", forms=lair_forms(1), prompts=dlg[name], msgbox=[ans])
    mon("q-char-nochar-flag", None, forms=lair_forms(1), prompts=dlg["vs_char"], msgbox=[6])
    mon("q-char-dontprompt", "warrior", forms=lair_forms(1), prompts=("A", "B", True, False, True), msgbox=[6])
    mon("q-solo-no-question", "warrior", prompts=dlg["vs_char"], msgbox=[6])
    for name in ("party_done", "party_new"):
        for ans in (6, 7, 2):
            mon("q-%s-%d" % (name, ans), "mage", forms=lair_forms(3, 1), prompts=dlg[name], msgbox=[ans])
    mon("q-remove-no-question", "warrior", forms=lair_forms(1), prompts=dlg["vs_char"], remove=True, msgbox=[6])
    # the real AddMonster2LV on a short list
    short = {"txtMonsterHP": {"Text": "90"}}
    mon("r-nochar-solo", None, forms=short, real=True)
    mon("r-warrior-solo-damage", "warrior", forms=merged(short, {"txtMonsterDamage": {"Text": "25"}}), real=True)
    mon("r-warrior-lair", "warrior", forms=merged(lair_forms(1), {"txtMonsterHP": {"Text": "70"}}), real=True)
    mon("r-nochar-lair-exp", None, forms=merged(lair_forms(1), {"txtMonsterHP": {"Text": "90"}}, exp_forms("200")), real=True)
    mon("r-mage-party", "mage", forms=merged(lair_forms(3, 1), {"txtMonsterHP": {"Text": "70"}}), real=True)
    mon("r-warrior-showall", "warrior", forms=merged(lair_forms(1), {"txtMonsterHP": {"Text": "40"}}), extras=E(bShowAll=True), real=True)
    mon("r-remove", None, forms=short, remove=True, real=True)

    scn = {
        "family": "filters_sm",
        "description": "List filters, spells and monsters (FAM-23): FilterSpells and FilterMonsters.",
        "notes": [
            "Each case runs frmMain's own FilterSpells / FilterMonsters (cut out by tools/oracle/frm.py) against a subset of the stock "
            "database (db_only). The lists are recording ListViews read before the sort that follows. A spell row is its number; a "
            "monster row is its number and a tag with the arguments that differ from AddMonster2LV's defaults (d,e,r,s,m,c,f,n,x) "
            "and the profile's HP (h) and HP regen (g) when the filter changed them. Cases with bRealAdd go on to the real AddMonster2LV.",
            "Stubbed: LV_RefreshSort / LV_Sort_ColumnClick, the *_ItemClick handlers, RefreshExtraMonsterFilterButton, "
            "RefreshMonsterColors, the picture loads. optMonsterFilter_Click only runs SetCurrentAttackTypeConfig (as "
            "RefreshMonsterCombatGUI does first). ReachIsMonsterOK is scripted (multiples of 7 are out of reach below level 30). "
            "A Yes to the damage questions is counted (recalc, recalc_party), not run.",
        ],
        "database": True,
        "db_only": {"Monsters": table_nums, "Spells": picked, "Lairs": [g for g in lair_groups if g in lairs]},
        "frm": FRM,
        "tolerance": {"default": 0},
        "globals": {
            "nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "UseExpMulti": False, "bOnlyInGame": True,
            **PC["ladder-c1-l50"]["globals"], **CH["warrior"][1],
            "nMonsterDamageVsChar": damage_array("vc", 0.3, 1.6, 1), "nMonsterDamageVsParty": damage_array("vp", 0.2, 2.0, 1),
            "nMonsterDamageVsDefault": damage_array("vd", 0.5, 1.2, 6), "nMonsterPossy": possy, "nMonsterSpawnChance": chance,
            "bGlobal_cephModelA": True, "bGlobal_cephModelB": True, "bGlobal_cephModelC": True, "bGlobal_cephModelD": True,
            "bGlobal_cephRecoveryOnly": False, "bGlobal_cephShowAll": False,
            "nGlobal_cephXP_Knob": 1.0, "nGlobal_cephDMG_Knob": 1.0, "nGlobal_cephMana_Knob": 1.0, "nGlobal_cephMove_Knob": 1.0,
                        "filter_Monster_nArmourClass": 9999, "filter_Monster_nDamageResist": 9999, "filter_Monster_nMagicRes": 9999,
            "filter_Monster_nBSDef": 9999, "filter_Monster_nGameLimit": 9999, "filter_Monster_nAvgLairExp": 0.0,
            "filter_Monster_nAtkAccuracyMaj": 9999, "filter_Monster_nAtkAccuracyMax": 9999, "filter_Monster_nDodge": 9999,
            "filter_Monster_nNumLairs": 0, "filter_Monster_nNumMobsLTE": 9999.0, "filter_Monster_nNumMobsGTE": 0.0,
            "filter_Monster_nFollowGTE": 0, "filter_Monster_nFollowLTE": 9999, "filter_Monster_nAtLeastLairs": 1,
            **{n: [-1] * (top + 1) for n in (
                "nCharDamageVsMonster", "nCharFirstRoundDamageVsMonster", "nCharMinRoundDamageVsMonster",
                "nCharSurpriseDamageVsMonster", "nCharSurpriseMinDamageVsMonster", "nCharSurpriseChanceVsMonster")},
        },
        "forms": {"frmMain": base_forms()},
        "data": data,
        "cases": cases,
    }
    if os.environ.get("FSM_ONLY"):
        scn["cases"] = [x for x in cases if os.environ["FSM_ONLY"] in x["id"]]
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("cases", len(cases), "table", len(table_nums), "lair subjects", len(lair_subjects), "bytes", os.path.getsize(OUT))


if __name__ == "__main__":
    main()

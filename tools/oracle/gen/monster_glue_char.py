"""Writes fixtures/scenarios/monster_glue_char.json (FAM-13, character side):
    python3 -I tools/oracle/gen/monster_glue_char.py fixtures/scenarios/monster_glue_char.json

SetupMonsterAttackSimWithCharStats, CalculateMonsterDamageVsChar (solo, party, mixed party),
GetPreCalculatedMonsterDamage and GetMonsterAvgDmgFromDB read frmMain controls (the character's AC,
DR, MR, dodge, anti-magic, protection from evil and resist tags, the lair filter's party) and the
Monsters, Spells and Items tables.

Characters: the control values of five pipeline cases (fixtures/rules/pipeline.json, whose oracle ran
RefreshAll) are copied here as literals, so this family does not depend on the pipeline's procedures;
the Rust test rebuilds the same characters with refresh_all and checks its adapter against them.
"characters" (top-level, ignored by the oracle) maps each label to its pipeline case id. Synthetic
characters (labels starting with "s") have hand-written controls for the assignment quirks.
"table" (also ignored by the oracle) holds the Monsters, Spells and Items rows the Rust test builds its
World from, as in monster_glue.py. Deterministic: strides only. Needs the converted stock SQLite and the
committed pipeline fixture."""
import glob, json, os, sqlite3, sys

out = sys.argv[1]
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
dbs = sorted(glob.glob(os.path.join(cache, "mdb", "*.sqlite")))
if not dbs:
    sys.exit("no converted database; run tools/mdb-fetch.sh")
c = sqlite3.connect(dbs[0])
c.row_factory = sqlite3.Row

MON = {r["Number"]: r for r in c.execute("select * from Monsters order by Number")}
SPL = {r["Number"]: r for r in c.execute("select * from Spells order by Number")}
ITM = {r["Number"]: r for r in c.execute("select * from Items order by Number")}
ITEM_ABS = (4, 22, 105, 106)
MAXN = max(MON)


def spell_abs(s):
    return [(s["Abil-%d" % i], s["AbilVal-%d" % i]) for i in range(10)]


def item_abs(i):
    return [(i["Abil-%d" % k], i["AbilVal-%d" % k]) for k in range(20)]


def has(abs_, a):
    return any(x == a for x, _ in abs_)


def tags(m):
    t = set()
    for x in range(5):
        ty = m["AttType-%d" % x]
        acc, hs = m["AttAcc-%d" % x], m["AttHitSpell-%d" % x]
        if ty == 2:
            t.add("spellatk")
            s = SPL.get(acc)
            if s is not None:
                ab = spell_abs(s)
                if s["Targets"] == 12:
                    t.add("area17" if has(ab, 17) else "area1" if has(ab, 1) else "areaother")
                if s["Dur"] > 0 or (s["DurIncLVLs"] > 0 and s["DurInc"] > 0):
                    t.add("durspell")
                for a in (1, 17, 8):
                    if has(ab, a):
                        t.add("atkab%d" % a)
                t.add("resist%d" % s["TypeOfResists"])
        if ty in (1, 3):
            t.add("melee")
        if ty in (1, 3) and hs > 0:
            t.add("hitspell")
            s = SPL.get(hs)
            if s is not None:
                ab = spell_abs(s)
                t.add("hit1" if has(ab, 1) else "hit17" if has(ab, 17) else "hitnodmg")
        ms = m["MidSpell-%d" % x]
        if ms > 0:
            t.add("mid")
            s = SPL.get(ms)
            if s is not None:
                for a in (1, 17, 8):
                    if has(spell_abs(s), a):
                        t.add("midab%d" % a)
    names = [m["AttName-%d" % x] for x in range(5) if 0 < m["AttType-%d" % x] < 4]
    if len(names) != len(set(names)):
        t.add("dupname")
    if not names and not any(m["MidSpell-%d" % x] > 0 for x in range(5)):
        t.add("noattack")
    if m["Energy"] != 1000:
        t.add("energy%s" % (m["Energy"] if m["Energy"] in (0, 1, 10, 100) else "other"))
    t.add("align%d" % m["Align"])
    its = [m["Weapon"]] + [m["DropItem-%d" % x] for x in range(10)]
    for it in its:
        i = ITM.get(it) if it else None
        if i is not None and any(has(item_abs(i), a) for a in ITEM_ABS):
            t.add("itembonus" if i["Limit"] == 0 else "itembonuslimited")
    return t


TAGS = {n: tags(m) for n, m in MON.items()}
ALL = sorted(MON)
by_tag = {}
for n in ALL:
    for t in TAGS[n]:
        by_tag.setdefault(t, []).append(n)

# ~80 monsters: up to 2 per tag (rare tags first), spread evenly, then every stride-th to fill
chosen = set()
for t in sorted(by_tag, key=lambda t: (len(by_tag[t]), t)):
    ns = by_tag[t]
    if len(chosen) >= 74:
        break
    step = max(1, len(ns) // 2)
    for n in ns[::step][:2]:
        chosen.add(n)
for n in ALL[::97]:
    if len(chosen) < 80:
        chosen.add(n)
# the cache cases name monsters 1..5 and 300, whose AvgDmg the fallback reads
chosen.update((1, 2, 3, 4, 5, 300, MAXN))
MONS = sorted(chosen)
ATTACKERS = [n for n in MONS if "noattack" not in TAGS[n]]
NOATK = [n for n in MONS if "noattack" in TAGS[n]]
SPELLY = [n for n in ATTACKERS if TAGS[n] & {"spellatk", "mid", "hitspell"}]
MELEE = [n for n in ATTACKERS if n not in SPELLY]
MISSING = [0, -1, 5000]

# ---- characters
pipe = {c_["id"]: c_ for c_ in json.load(open(os.path.join(ROOT, "fixtures", "rules", "pipeline.json")))["cases"]}
CHARS = {
    "caster": "ladder-c12-l50",     # low AC: a negative caption, DR 0.2
    "warrior": "bless-15",          # AC 109, DR 100.5, resists 25 and dodge 121
    "antimagic": "grid-c2-r1-l255",  # anti-magic class, protection from evil, a negative resist
    "highmr": "edge-huge-stats",    # MR 255
    "nothing": "edge-neither",      # no class, nothing but a negative dodge
}


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def forms_of(ctl):
    """frmMain controls from a dict of strings (class is an int)."""
    cls = ctl.get("cls", 0)
    stat = {}
    for i, k in ((2, "ac"), (3, "dr")):
        if k in ctl:
            stat[i] = {"Caption": ctl[k]}
    for i, k in ((20, "pe"), (25, "sto"), (26, "wat"), (27, "fir"), (28, "col"), (29, "lit")):
        if k in ctl:
            stat[i] = {"Tag": ctl[k]}
    f = {
        "lblInvenCharStat": arr(stat),
        "cmbGlobalClass": arr({0: {"ListIndex": 1, "ItemData": {"1": cls}} if cls else {"ListIndex": 0}}),
        "chkGlobalFilter": {"Value": 1},
        "optMonsterFilter": arr({1: {"Value": ctl.get("lair", False)}}),
    }
    if "mr" in ctl:
        f["txtCharMR"] = {"Text": ctl["mr"]}
    if "dodge" in ctl:
        f["lblCharDodge"] = {"Tag": ctl["dodge"]}
    if ctl.get("am") is not None:
        f["chkCharAntiMagic"] = {"Value": ctl["am"]}
    return f


def from_pipeline(pid):
    case = pipe[pid]
    o, inp = case["outputs"], case["inputs"]["forms"]["frmMain"]
    cls = inp["cmbGlobalClass"]["$array"]["0"].get("ItemData", {}).get("1", 0) if inp["cmbGlobalClass"]["$array"]["0"].get("ListIndex", 0) > 0 else 0
    t = o["tags"]
    return {"cls": cls, "ac": o["stat"][2], "dr": o["stat"][3], "mr": o["mr"], "dodge": o["dodge_tag"], "am": o["antimagic"],
            "pe": t[20], "sto": t[25], "wat": t[26], "fir": t[27], "col": t[28], "lit": t[29]}


CTL = {k: from_pipeline(v) for k, v in CHARS.items()}

# synthetic characters: assignment quirks
CTL["s_empty"] = {}   # every control at its default: nothing is set, and the empty resist tag is error 13
CTL["s_round"] = {"ac": "2.5", "dr": "0.5", "mr": "49.5", "dodge": "33.5", "am": 1, "pe": "1.5",
                  "col": "3.5", "fir": "-2.5", "sto": "0.5", "lit": "2.5", "wat": "7", "cls": 4}
CTL["s_neg"] = {"ac": "-5", "dr": "-1.5", "mr": "-3", "dodge": "-9", "am": 0, "pe": "-4",
                "col": "-10", "fir": "-20", "sto": "-1", "lit": "-30", "wat": "-40"}
CTL["s_big"] = {"ac": "3000000000", "dr": "5", "mr": "5", "dodge": "5", "am": 1, "pe": "5",
                "col": "1", "fir": "2", "sto": "3", "lit": "4", "wat": "5"}   # AC overflows Long: error 6
CTL["s_partial"] = {"ac": "40", "dr": "12", "mr": "60", "dodge": "30", "am": 0, "pe": "0", "col": "10", "fir": "20"}  # sto/lit/wat tags empty

# ---- cases
SIM = {"sim": {"class": "clsMonsterAttackSim", "global": "clsMonAtkSim"}}
USER = {k: "sim.%s" % k for k in (
    "nUserAC", "nUserDR", "nUserDodge", "nUserMR", "nUserAntiMagic", "nUserProtEvil", "nUserVileWard", "nUserRFIR", "nUserRCOL",
    "nUserRLIT", "nUserRWAT", "nUserRSTO", "nNumberOfRounds", "bDynamicCalc", "bGreaterMUD", "nDynamicCalcDifference", "HIT_MIN",
    "HIT_CAP", "SPELL_HIT_CAP", "DODGE_SOFTCAP", "DODGE_CAP", "nCombatLogMaxRounds", "bCombatLogMaxRoundOnly", "bMobIsEvil")}
STR = {"ref": "string", "value": ""}

cases, grids = [], []


def char_grid(label, ns, rounds=None):
    g = {"name": "dmg-" + label, "axes": {"n": ns}, "case": {
        "objects": SIM, "forms": {"frmMain": forms_of(CTL[label])},
        "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n"], "as": "r"},
                  {"call": "GetPreCalculatedMonsterDamage", "args": ["$n", STR], "as": "cache"}],
        }}
    if rounds is not None:
        g["case"]["globals"] = {"nGlobalMonsterSimRounds": rounds}
    grids.append(g)


for label in ("caster", "warrior", "antimagic", "highmr"):
    char_grid(label, MONS)
char_grid("nothing", ATTACKERS[::4] + NOATK[:2] + MISSING)
for label in ("s_empty", "s_round", "s_neg", "s_big", "s_partial"):
    char_grid(label, SPELLY[::6][:6] + MELEE[::8][:4])
# rounds: below 100 and above 10000 are clamped; 250 is taken as is
for rounds, tag in ((0, "r0"), (37, "r37"), (250, "r250")):
    grids.append({"name": "rounds-" + tag, "axes": {"n": SPELLY[::9][:5] + MELEE[::12][:3]}, "case": {
        "objects": SIM, "forms": {"frmMain": forms_of(CTL["warrior"])}, "globals": {"nGlobalMonsterSimRounds": rounds},
        "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n"], "as": "r"}],
        "read": {"rnd": "%rnd", "rounds": "sim.nNumberOfRounds"}}})
grids.append({"name": "rounds-max", "axes": {"n": SPELLY[:2]}, "case": {
    "objects": SIM, "forms": {"frmMain": forms_of(CTL["caster"])}, "globals": {"nGlobalMonsterSimRounds": 30000},
    "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n"], "as": "r"}],
    "read": {"rnd": "%rnd", "rounds": "sim.nNumberOfRounds"}}})
# a different seed
grids.append({"name": "seed", "axes": {"n": SPELLY[::7][:6]}, "case": {
    "rnd": 90210, "objects": SIM, "forms": {"frmMain": forms_of(CTL["antimagic"])},
    "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n"], "as": "r"}], "read": {"rnd": "%rnd"}}})

# ---- party: the lair filter's controls (0 size, 1 AC, 2 DR, 3 MR, 4 dodge, 6 anti-magic count)
PARTIES = {
    "solo": ("4", "40", "8", "70", "30", "0"),
    "split": ("5", "35", "5", "60", "25", "2"),
    "equal": ("3", "50", "10", "80", "40", "3"),
    "empty": ("6", "", "", "", "", "1"),
    "round": ("2", "45.5", "2.5", "55.5", "20.5", "1"),
    "surplus": ("3", "20", "4", "40", "10", "5"),
    "overflow": ("40000", "20", "4", "40", "10", "1"),
    "fraction": ("2.5", "30", "6", "65", "15", "1"),
    "single": ("1", "30", "6", "65", "15", "1"),
    "blank": ("", "", "", "", "", ""),
}


PN = SPELLY[1::5][:7] + MELEE[2::9][:5]
for name, vals in PARTIES.items():
    pf = {"txtMonsterLairFilter": arr(
        {0: {"Text": vals[0]}, 1: {"Text": vals[1]}, 2: {"Text": vals[2]}, 3: {"Text": vals[3]}, 4: {"Text": vals[4]},
         6: {"Text": vals[5]}}),
        "optMonsterFilter": arr({1: {"Value": True}}), "chkGlobalFilter": {"Value": 0}}
    grids.append({"name": "party-" + name, "axes": {"n": PN if name not in ("single", "blank", "overflow") else PN[:4]}, "case": {
        "objects": SIM, "forms": {"frmMain": pf},
        "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n", True], "as": "r"},
                  {"call": "GetPreCalculatedMonsterDamage", "args": ["$n", STR], "as": "cache"}],
        }})
# the party path while the character's own (non-party) call ignores the party boxes
grids.append({"name": "party-ignored", "axes": {"n": PN[:4]}, "case": {
    "objects": SIM, "forms": {"frmMain": dict(forms_of(CTL["warrior"]), txtMonsterLairFilter=arr(
        {0: {"Text": "5"}, 1: {"Text": "99"}, 6: {"Text": "2"}}))},
    "steps": [{"call": "CalculateMonsterDamageVsChar", "args": ["$n", False], "as": "r"}],
    }})

# ---- SetupMonsterAttackSimWithCharStats on its own: defaults, every argument, every character
SETUP_ARGS = [[], [100], [250, False], [100, True, True], [100, False, True, 1], [100, False, True, 0], [30, True, False, 1]]
for label in CTL:
    cs = {"objects": SIM, "forms": {"frmMain": forms_of(CTL[label])}}
    grids.append({"name": "setup-" + label, "axes": {"a": SETUP_ARGS}, "case": dict(
        cs, steps=[{"call": "SetupMonsterAttackSimWithCharStats", "args": "$a"}])})
# party values for the setup
for name in ("split", "round", "empty", "blank"):
    vals = PARTIES[name]
    pf = {"txtMonsterLairFilter": arr({0: {"Text": vals[0]}, 1: {"Text": vals[1]}, 2: {"Text": vals[2]}, 3: {"Text": vals[3]},
                                       4: {"Text": vals[4]}, 6: {"Text": vals[5]}})}
    grids.append({"name": "setup-party-" + name, "axes": {"a": [[100, False, True, 0], [100, False, True, 1], [50, True, True, 1]]},
                  "case": {"objects": SIM, "forms": {"frmMain": pf},
                           "steps": [{"call": "SetupMonsterAttackSimWithCharStats", "args": "$a"}]}})

# ---- the simulator is one global: durations and hit-spell names survive between calls
SEQ = [(a, b) for a in SPELLY[::11][:4] for b in MELEE[::17][:3]]
for a, b in SEQ:
    cases.append({"id": "seq-%d-%d" % (a, b), "objects": SIM, "forms": {"frmMain": forms_of(CTL["caster"])},
                  "steps": [{"call": "CalculateMonsterDamageVsChar", "args": [a], "as": "r1"},
                            {"call": "CalculateMonsterDamageVsChar", "args": [b], "as": "r2"}],
                  "read": {"dur": "sim.nAtkDuration[0..4]", "hsn": "sim.sAtkHitSpellName[0..4]",
                           "brd": "sim.nBetweenRoundDuration[0..4]", "rnd": "%rnd"}})

# ---- caches: GetPreCalculatedMonsterDamage reads what CalculateMonsterDamageVsChar stored
# (the arrays are dimensioned and -1 in the family setup)
GOOD = SPELLY[3]
for name, forms, args, steps_before in (
    ("char", forms_of(CTL["warrior"]), [GOOD], True),
    ("char-nofilter", dict(forms_of(CTL["warrior"]), chkGlobalFilter={"Value": 0}), [GOOD], True),
    ("never", forms_of(CTL["warrior"]), [GOOD], False),
):
    st = []
    if steps_before:
        st.append({"call": "CalculateMonsterDamageVsChar", "args": [GOOD]})
    st.append({"call": "GetPreCalculatedMonsterDamage", "args": [GOOD, STR], "as": "g"})
    cases.append({"id": "cache-" + name, "objects": SIM, "forms": {"frmMain": forms}, "steps": st, "read": {"rnd": "%rnd"}})
# party size from the box (lair mode) or the argument, with the party cache filled in first
for lairmode, boxsize, npar in ((True, "3", None), (True, "1", None), (False, "3", None), (False, "3", 4), (True, "3", 1), (True, "70000", None),
                                (True, "", None), (True, "2.5", None)):
    forms = dict(forms_of(CTL["caster"]), optMonsterFilter=arr({1: {"Value": lairmode}}),
                 txtMonsterLairFilter=arr({0: {"Text": boxsize}, 6: {"Text": "1"}}))
    for fill in ("party", "char", "none"):
        st = []
        if fill == "party":
            st.append({"call": "CalculateMonsterDamageVsChar", "args": [GOOD, True]})
        elif fill == "char":
            st.append({"call": "CalculateMonsterDamageVsChar", "args": [GOOD]})
        a = [GOOD, STR] + ([npar] if npar is not None else [])
        st.append({"call": "GetPreCalculatedMonsterDamage", "args": a, "as": "g"})
        cases.append({"id": "cache-lair%d-%s-n%s-%s" % (lairmode, boxsize or "blank", npar, fill), "objects": SIM,
                      "forms": {"frmMain": forms}, "steps": st, "read": {"rnd": "%rnd"}})
# no monster: only the label
for lairmode, boxsize, npar, gf in ((True, "3", None, 1), (False, "", None, 1), (False, "", None, 0), (False, "", 2, 0)):
    forms = {"optMonsterFilter": arr({1: {"Value": lairmode}}), "txtMonsterLairFilter": arr({0: {"Text": boxsize}}), "chkGlobalFilter": {"Value": gf}}
    for n in (0, -3):
        a = [n, STR] + ([npar] if npar is not None else [])
        cases.append({"id": "cache-label-%d-%s-%s-%d-n%d" % (lairmode, boxsize or "blank", npar, gf, n), "read": {}, "forms": {"frmMain": forms},
                      "steps": [{"call": "GetPreCalculatedMonsterDamage", "args": a, "as": "g"}]})
# case arrays are laid over the setup's (the oracle copies a list into the existing array from index 0), so
# these are caches filled in for the first few monsters; the rest of each array stays -1
for gi, glob_ in enumerate(({"nMonsterDamageVsParty": [-1, -1, 5], "nMonsterDamageVsChar": [-1, -1, 7, 8, 9, 10], "nMonsterDamageVsDefault": [-1] * 3},
                            {"nMonsterDamageVsParty": [-1] * 4, "nMonsterDamageVsChar": [-1, -1], "nMonsterDamageVsDefault": [-1] * 4},
                            {"nMonsterDamageVsParty": [-1] * 4, "nMonsterDamageVsChar": [-1] * 4, "nMonsterDamageVsDefault": [-1, 3, 4]},
                            {"nMonsterDamageVsParty": [-1] * 4, "nMonsterDamageVsChar": [-1] * 4, "nMonsterDamageVsDefault": [-1, 3.5, 4]})):
    for n in (1, 2, 5, 300):
        for npar in (1, 3):
            cases.append({"id": "cache-overlay-%d-%d-%d" % (gi, n, npar),
                          "globals": glob_, "read": {}, "forms": {"frmMain": {"chkGlobalFilter": {"Value": 1}}},
                          "steps": [{"call": "GetPreCalculatedMonsterDamage", "args": [n, STR, npar], "as": "g"}]})
# a monster number past the arrays' end is error 9: the handler gives 0 and leaves the label as the caller had it
for n in (MAXN, MAXN + 1, 5000):
    for npar in (1, 3):
        cases.append({"id": "cache-range-%d-%d" % (n, npar), "read": {}, "forms": {"frmMain": {"chkGlobalFilter": {"Value": 1}}},
                      "steps": [{"call": "GetPreCalculatedMonsterDamage", "args": [n, STR, npar], "as": "g"}]})
# cached values in the arrays, all three kinds
FULL = {"nMonsterDamageVsParty": [-1, 12.5, -1, 2, -1, -1], "nMonsterDamageVsChar": [-1, -1, 7.25, 3, -1, -1],
        "nMonsterDamageVsDefault": [-1, 1.5, 2.5, 3.5, 4.5, -1]}
for n in range(0, 6):
    for npar, gf in ((1, 1), (1, 0), (2, 1), (2, 0)):
        cases.append({"id": "cache-full-%d-%d-%d" % (n, npar, gf), "globals": FULL, "read": {},
                      "forms": {"frmMain": {"chkGlobalFilter": {"Value": gf}}},
                      "steps": [{"call": "GetPreCalculatedMonsterDamage", "args": [n, STR, npar], "as": "g"}]})

# ---- GetMonsterAvgDmgFromDB: the stored AvgDmg from 1.8, else a simulation
AVG = MONS[::3] + MISSING
grids.append({"name": "avgdb", "axes": {"n": AVG}, "case": {"objects": SIM, "steps": [
    {"call": "GetMonsterAvgDmgFromDB", "args": ["$n"], "as": "r"}], "read": {"rnd": "%rnd"}}})
for v, rounds, tag in ((1.7, 100, "v170"), (1.75, 0, "v175-0")):
    grids.append({"name": "avgdb-" + tag, "axes": {"n": SPELLY[::8][:6] + MELEE[::13][:3] + MISSING[:1]}, "case": {
        "objects": SIM, "globals": {"nNMRVer": v, "nGlobalMonsterSimRounds": rounds},
        "steps": [{"call": "GetMonsterAvgDmgFromDB", "args": ["$n"], "as": "r"}],
        "read": {"rnd": "%rnd"}}})

# ---- the table the Rust test builds its World from
need_m = set(MONS)
need_s = set()
for n in need_m:
    m = MON[n]
    need_s.add(m["DeathSpell"])
    for x in range(5):
        need_s.update((m["AttAcc-%d" % x] if m["AttType-%d" % x] == 2 else 0, m["AttHitSpell-%d" % x], m["MidSpell-%d" % x]))
need_s.discard(0)
todo = sorted(need_s)
while todo:
    s = SPL.get(todo.pop())
    if s is None:
        continue
    for k, v in spell_abs(s):
        if k == 151 and v > 0 and v not in need_s:
            need_s.add(v)
            todo.append(v)
need_i = set()
for n in need_m:
    m = MON[n]
    need_i.update([m["Weapon"]] + [m["DropItem-%d" % x] for x in range(10)])
need_i.discard(0)


def flat(ab):
    f = [x for pair in ab for x in pair]
    while f and f[-1] == 0:
        f.pop()
    return f


def trim(row):
    while row and row[-1] in (0, [], ""):
        row.pop()
    return row


monsters = []
for n in sorted(need_m):
    m = MON[n]
    atk = [trim([m["AttName-%d" % x] or "", m["AttType-%d" % x], m["AttAcc-%d" % x], m["Att%%-%d" % x],
                 m["AttTrue%%-%d" % x], m["AttMin-%d" % x], m["AttMax-%d" % x], m["AttEnergy-%d" % x],
                 m["AttHitSpell-%d" % x]]) for x in range(5)]
    mid = [trim([m["MidSpell-%d" % x], m["MidSpell%%-%d" % x], m["MidSpellLVL-%d" % x]]) for x in range(5)]
    drops = [[m["DropItem-%d" % x], m["DropItem%%-%d" % x]] for x in range(10) if m["DropItem-%d" % x]]
    monsters.append([n, m["Align"], m["Energy"], m["Weapon"], atk, mid, drops, m["DeathSpell"], m["AvgDmg"]])
spells = []
for n in sorted(need_s):
    s = SPL.get(n)
    if s is None:
        continue
    spells.append([n, s["Name"] or "", s["AttType"], s["TypeOfResists"], s["Targets"], s["Dur"], s["DurInc"],
                   s["DurIncLVLs"], s["MinBase"], s["MaxBase"], s["MinInc"], s["MinIncLVLs"], s["MaxInc"],
                   s["MaxIncLVLs"], s["ReqLevel"], s["Cap"], s["EnergyCost"], flat(spell_abs(s))])
items = []
for n in sorted(need_i):
    i = ITM.get(n)
    if i is None or not any(has(item_abs(i), a) for a in ITEM_ABS):
        continue
    items.append([n, i["Limit"], flat(item_abs(i))])

scn = {
    "family": "monster_glue_char",
    "database": True,
    "description": "FAM-13 character side: SetupMonsterAttackSimWithCharStats, CalculateMonsterDamageVsChar (solo and party), "
                   "GetPreCalculatedMonsterDamage and GetMonsterAvgDmgFromDB (stock engine) over stock monsters.",
    "notes": [
        "Characters: the control values (AC, DR, MR, dodge, anti-magic, protection from evil, resist tags) of pipeline fixture cases, "
        "copied as literals; 'characters' maps each label to its pipeline case id. s_* characters have hand-written controls.",
        "The damage arrays nMonsterDamageVsChar/Party/Default are dimensioned to the highest monster number and -1 in the setup, "
        "as ClearMonsterDamageVsCharALL leaves them; GetPreCalculatedMonsterDamage reads back what CalculateMonsterDamageVsChar stored.",
        "Random mode (bDynamic False) on the oracle's seeded Rnd; the final Rnd state is read so the sequence of rolls is pinned.",
        "table (top-level key, ignored by the oracle) holds the rows the Rust test builds its World from: "
        "monsters [Number, Align, Energy, Weapon, [5 x [AttName, AttType, AttAcc, Att%, AttTrue%, AttMin, AttMax, AttEnergy, AttHitSpell]], "
        "[5 x [MidSpell, MidSpell%, MidSpellLVL]], [[DropItem, DropItem%]..], DeathSpell, AvgDmg]; "
        "spells and items as in monster_glue.json.",
    ],
    "tolerance": {"default": 0},
    # a case's own "read" replaces this default: the simulator's user-side state after the steps, and the Rnd state
    "read": dict(USER, rnd="%rnd"),
    "globals": {"nNMRVer": 1.83, "nGlobalMonsterSimRounds": 100,
                "nMonsterDamageVsChar": [-1] * (MAXN + 1), "nMonsterDamageVsParty": [-1] * (MAXN + 1),
                "nMonsterDamageVsDefault": [-1] * (MAXN + 1)},
    "characters": CHARS,
    "controls": CTL,
    "cases": cases,
    "grids": grids,
    "table": {"monsters": monsters, "spells": spells, "items": items},
}
with open(out, "w", newline="\n") as f:
    json.dump(scn, f, separators=(",", ":"), sort_keys=True)
    f.write("\n")
print("monsters %d (spelly %d melee %d noatk %d), spells %d, items %d" %
      (len(monsters), len(SPELLY), len(MELEE), len(NOATK), len(spells), len(items)))

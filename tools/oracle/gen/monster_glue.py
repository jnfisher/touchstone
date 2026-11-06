"""Writes fixtures/scenarios/monster_glue.json (FAM-13, monster side):
    python3 -I tools/oracle/gen/monster_glue.py fixtures/scenarios/monster_glue.json

PopulateMonsterDataToAttackSim, CalculateMonsterAvgDmg and GetMonsterAttackSummary read the
Monsters, Spells and Items tables, so the scenario carries the rows the Rust test needs under the
top-level key "table" (the oracle ignores it; the test reads it from this file, so the rows are
stored once). Rows are compact positional lists, see TABLE_NOTES. Cases are grids over monster
numbers, so the scenario stays small; the fixture expands them. Deterministic: strides only.
Needs the converted stock SQLite (tools/mdb-fetch.sh)."""
import glob, json, os, sqlite3, sys

out = sys.argv[1]
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


def spell_abs(s):
    return [(s["Abil-%d" % i], s["AbilVal-%d" % i]) for i in range(10)]


def item_abs(i):
    return [(i["Abil-%d" % k], i["AbilVal-%d" % k]) for k in range(20)]


def has(abs_, a):
    return any(x == a for x, _ in abs_)


# ---- what each monster exercises
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
                        if [v for k, v in ab if k == a][0] > 0:
                            t.add("atkab%dval" % a)
                t.add("resist%d" % s["TypeOfResists"])
        if ty in (1, 3):
            t.add("melee")
        if ty in (1, 3) and hs > 0:
            t.add("hitspell")
            s = SPL.get(hs)
            if s is not None:
                ab = spell_abs(s)
                t.add("hit1" if has(ab, 1) else "hit17" if has(ab, 17) else "hitnodmg")
                if s["Dur"] > 0:
                    t.add("hitdur")
        if ty not in (0, 1, 2, 3):
            t.add("junktype")
        ms = m["MidSpell-%d" % x]
        if ms > 0:
            s = SPL.get(ms)
            t.add("mid")
            if s is not None:
                ab = spell_abs(s)
                for a in (1, 17, 8):
                    if has(ab, a):
                        t.add("midab%d" % a)
                if s["Dur"] > 0:
                    t.add("middur")
    names = [m["AttName-%d" % x] for x in range(5) if 0 < m["AttType-%d" % x] < 4]
    if len(names) != len(set(names)):
        t.add("dupname")
    if len(names) == 0:
        t.add("noattack")
    if m["Energy"] != 1000:
        t.add("energy%s" % (m["Energy"] if m["Energy"] in (0, 1, 10, 100) else "other"))
    t.add("align%d" % m["Align"])
    its = [m["Weapon"]] + [m["DropItem-%d" % x] for x in range(10)]
    for it in its:
        i = ITM.get(it) if it else None
        if i is not None and any(has(item_abs(i), a) for a in ITEM_ABS):
            t.add("itembonus" if i["Limit"] == 0 else "itembonuslimited")
    if m["Weapon"] > 0:
        t.add("weapon")
    if m["DeathSpell"] > 0:
        t.add("death")
    pcts = [m["Att%%-%d" % x] for x in range(5) if 0 < m["AttType-%d" % x] < 4]
    if pcts and pcts != sorted(pcts):
        t.add("nonmonotonicpct")
    if m["Number"] in (1, 2):
        t.add("first")
    return t


TAGS = {n: tags(m) for n, m in MON.items()}
ALL = sorted(MON)
by_tag = {}
for n in ALL:
    for t in TAGS[n]:
        by_tag.setdefault(t, []).append(n)


def pick(per_tag, stride_all):
    s = set(ALL[::stride_all])
    for t, ns in by_tag.items():
        step = max(1, len(ns) // per_tag)
        s.update(ns[::step][:per_tag])
    s.update(by_tag.get("itembonus", []) + by_tag.get("itembonuslimited", []))
    return sorted(s)


POP = pick(3, 41)              # full simulator state after Populate
VER = POP[::3]                # older data versions
FLAGS = POP[::2]            # summary flag combinations
AVG = POP[::3][1:]              # seeded average-damage runs
MISSING = [0, -1, 5000, 32768]  # no such monster
SEQ_DUR = (by_tag.get("durspell", []) + by_tag.get("hitdur", []))[::37][:8]
SEQ_PLAIN = [n for n in by_tag.get("melee", []) if "spellatk" not in TAGS[n] and "hitspell" not in TAGS[n]][::97][:8]

# ---- cases
SIM = {"sim": {"class": "clsMonsterAttackSim", "global": "clsMonAtkSim"}}
POPSIM = {"sim": {"class": "clsMonsterAttackSim"}}
READ = {k: "sim.%s[0..4]" % k for k in (
    "sAtkName", "nAtkType", "nAtkSpellType", "nAtkEnergy", "nAtkMin", "nAtkMax", "nAtkChance", "nAtkSuccess",
    "nAtkHitSpellMin", "nAtkHitSpellMax", "nAtkHitSpellType", "sAtkHitSpellName", "nAtkResist", "nAtkMRdmgResist",
    "nAtkDuration", "sBetweenRoundName", "nBetweenRoundMin", "nBetweenRoundMax", "nBetweenRoundSpellType",
    "nBetweenRoundChance", "nBetweenRoundResistType", "nBetweenRoundResistDmgMR", "nBetweenRoundDuration")}
READ["bMobIsEvil"] = "sim.bMobIsEvil"
READ["nEnergyPerRound"] = "sim.nEnergyPerRound"


def pop_case(version=None):
    d = {"objects": POPSIM, "read": READ,
         "steps": [{"call": "sim.ResetValues"},
                   {"call": "PopulateMonsterDataToAttackSim", "args": ["$n", {"obj": "sim"}]}]}
    if version:
        d["globals"] = {"nNMRVer": version}
    return d


def sum_case(flags, version=None):
    d = {"proc": "GetMonsterAttackSummary", "args": ["$n"] + flags}
    if version:
        d["globals"] = {"nNMRVer": version}
    return d


grids = [
    # every stock monster: the max-damage path (no rounds) and the attack summary
    {"name": "maxsum", "axes": {"n": ALL}, "case": {
        "objects": SIM, "steps": [
            {"call": "CalculateMonsterAvgDmg", "args": ["$n", 0], "as": "r"},
            {"call": "GetMonsterAttackSummary", "args": ["$n", True, True], "as": "s"}]}},
    {"name": "pop", "axes": {"n": POP}, "case": pop_case()},
    {"name": "missing", "axes": {"n": MISSING}, "case": {
        "objects": SIM, "steps": [
            {"call": "CalculateMonsterAvgDmg", "args": ["$n", 20], "as": "r"},
            {"call": "GetMonsterAttackSummary", "args": ["$n", True, True], "as": "s"}],
        "read": {"nEnergyPerRound": "sim.nEnergyPerRound", "bMobIsEvil": "sim.bMobIsEvil"}}},
    {"name": "sumff", "axes": {"n": FLAGS}, "case": sum_case([False, False])},
    {"name": "sumtf", "axes": {"n": FLAGS}, "case": sum_case([True, False])},
    {"name": "sumft", "axes": {"n": FLAGS}, "case": sum_case([False, True])},
]
for v, tagname in ((1.7, "v170"), (1.75, "v175")):
    grids.append({"name": "pop" + tagname, "axes": {"n": VER}, "case": pop_case(v)})
    grids.append({"name": "sum" + tagname, "axes": {"n": VER}, "case": sum_case([True, True], v)})

# seeded average-damage runs: 50 and 500 rounds, default Rnd state
for rounds in (50, 500):
    grids.append({"name": "avg%d" % rounds, "axes": {"n": AVG}, "case": {
        "objects": SIM,
        "steps": [{"call": "CalculateMonsterAvgDmg", "args": ["$n", rounds], "as": "r"}],
        "read": {"phys": "sim.nAverageDamagePhys", "spell": "sim.nAverageDamageSpell",
                 "attacks": "sim.nTotalAttacks", "rnd": "%rnd"}}})
# a different seed, and a no-rounds default (Optional nNumRounds is 500)
grids.append({"name": "avgseed", "axes": {"n": AVG[::2]}, "case": {
    "rnd": 4242, "objects": SIM,
    "steps": [{"call": "CalculateMonsterAvgDmg", "args": ["$n", 200], "as": "r"}],
    "read": {"rnd": "%rnd"}}})
grids.append({"name": "avgdef", "axes": {"n": AVG[::7]}, "case": {
    "objects": SIM,
    "steps": [{"call": "CalculateMonsterAvgDmg", "args": ["$n"], "as": "r"}],
    "read": {"rnd": "%rnd"}}})

# the simulator is one global: durations and hit-spell names are not reset between monsters
cases = []
for i, a in enumerate(SEQ_DUR):
    for j, b in enumerate(SEQ_PLAIN):
        if (i + j) % 3 == 0:
            cases.append({"id": "seq-%d-%d" % (a, b), "objects": SIM, "steps": [
                {"call": "CalculateMonsterAvgDmg", "args": [a, 30], "as": "r1"},
                {"call": "CalculateMonsterAvgDmg", "args": [b, 30], "as": "r2"}],
                "read": {"dur": "sim.nAtkDuration[0..4]", "hsn": "sim.sAtkHitSpellName[0..4]",
                         "brd": "sim.nBetweenRoundDuration[0..4]", "rnd": "%rnd"}})

# ---- the table the Rust test builds its World from
need_m = set(POP + VER + FLAGS + AVG + SEQ_DUR + SEQ_PLAIN + ALL)
need_s = set()
for n in ALL:
    m = MON[n]
    need_s.add(m["DeathSpell"])
    for x in range(5):
        need_s.update((m["AttAcc-%d" % x] if m["AttType-%d" % x] == 2 else 0, m["AttHitSpell-%d" % x], m["MidSpell-%d" % x]))
need_s.discard(0)
# EndCast spells (ability 151) are followed by SpellDoesDamage
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
for n in ALL:
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
    monsters.append([n, m["Align"], m["Energy"], m["Weapon"], atk, mid, drops, m["DeathSpell"]])
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
    if i is None:
        continue
    if i["Limit"] != 0 and not any(has(item_abs(i), a) for a in ITEM_ABS):
        continue  # only the limit and the four abilities MME asks about are ever read
    if not any(has(item_abs(i), a) for a in ITEM_ABS):
        continue  # an item without them adds nothing, whatever its limit
    items.append([n, i["Limit"], flat(item_abs(i))])

scn = {
    "family": "monster_glue",
    "database": True,
    "description": "FAM-13 monster side: PopulateMonsterDataToAttackSim, CalculateMonsterAvgDmg, "
                   "GetMonsterAttackSummary (stock engine) over the stock Monsters.",
    "notes": [
        "table (top-level key, ignored by the oracle) holds the rows the Rust test builds its World from:",
        "monsters [Number, Align, Energy, Weapon, [5 x [AttName, AttType, AttAcc, Att%, AttTrue%, AttMin, AttMax, AttEnergy, AttHitSpell]], [5 x [MidSpell, MidSpell%, MidSpellLVL]], [[DropItem, DropItem%]..], DeathSpell], trailing zeros dropped;",
        "spells [Number, Name, AttType, TypeOfResists, Targets, Dur, DurInc, DurIncLVLs, MinBase, MaxBase, MinInc, MinIncLVLs, MaxInc, MaxIncLVLs, ReqLevel, Cap, EnergyCost, flat Abil/AbilVal pairs];",
        "items [Number, Limit, flat Abil/AbilVal pairs] for the items that carry ability 4, 22, 105 or 106.",
    ],
    "tolerance": {"default": 0},
    "globals": {"nNMRVer": 1.83},
    "cases": cases,
    "grids": grids,
    "table": {"monsters": monsters, "spells": spells, "items": items},
}
with open(out, "w", newline="\n") as f:
    json.dump(scn, f, separators=(",", ":"), sort_keys=True)
    f.write("\n")
print("monsters %d, spells %d, items %d; pop %d ver %d flags %d avg %d" %
      (len(monsters), len(spells), len(items), len(POP), len(VER), len(FLAGS), len(AVG)))

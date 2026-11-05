"""Writes fixtures/scenarios/exphr_c.json: python3 -I tools/oracle/gen/exphr_c.py fixtures/scenarios/exphr_c.json

Scenarios for modExpPerHour ceph_ModelC (called directly, 25 positional arguments in the
CalcExpPerHour order). Deterministic: grids are fixed, samples use the oracle's splitmix64 draws.
"""
import json, sys

KNOBS1 = {"nGlobal_cephXP_Knob": 1, "nGlobal_cephDMG_Knob": 1, "nGlobal_cephMana_Knob": 1,
          "nGlobal_cephMove_Knob": 1}
ARGS = ["exp", "regen", "mobs", "tl", "ps", "rtk", "cd", "hp", "hpr", "md", "mhp", "mhr", "th", "sc",
        "so", "mana", "mpr", "med", "aw", "ws", "sd", "smin", "sch", "fr", "mr"]
BASE = dict(exp=1200, regen=5, mobs=2, tl=10, ps=5, rtk=0, cd=100, hp=300, hpr=45, md=40, mhp=150,
            mhr=0, th=0, sc=0, so=0, mana=0, mpr=0, med=0, aw=6, ws=1.25, sd=0, smin=0, sch=0,
            fr=0, mr=0)


def args_of(d, ref=False):
    out = []
    for k in ARGS:
        v = ("$" + k) if ref else d[k]
        out.append({"t": "cur", "v": v} if k == "exp" else v)
    return out


def case(id_, **over):
    d = dict(BASE)
    d.update(over)
    knobs = {k: over.get(k2, 1) for k, k2 in (("nGlobal_cephXP_Knob", "kx"),
             ("nGlobal_cephDMG_Knob", "kd"), ("nGlobal_cephMana_Knob", "km"),
             ("nGlobal_cephMove_Knob", "kv"))}
    for k2 in ("kx", "kd", "km", "kv"):
        d.pop(k2, None)
    return {"id": id_, "globals": knobs, "proc": "ceph_ModelC", "args": args_of(d)}


cases = [
    case("typical"),
    case("exp-zero", exp=0),
    case("mana-overflow", sc=5, mana=40000),
    case("mana-max", sc=5, mana=32767, mpr=3, med=20),
    case("mana-neg", sc=5, mana=-5, mpr=3, med=20),
    case("spellcost-overflow", sc=40000, mana=100),
    case("surprise-chance-overflow", sd=40, sch=40000),
    case("mobs-zero", mobs=0),
    case("mobs-negative", mobs=-3),
    case("mobs-fraction", mobs=2.5, mhp=300),
    case("walkspeed-zero", ws=0),
    case("walkspeed-neg-walk-neg", ws=-2, aw=-4),
    case("walk-zero", aw=0),
    case("no-dmg", cd=0, fr=0),
    case("first-only", cd=0, fr=60),
    case("first-bigger-than-avg", cd=40, fr=200, mhp=900, mobs=1),
    case("mobhp-zero", mhp=0),
    case("mobhp-negative", mhp=-50),
    case("one-shot", cd=500, mhp=150, mobs=1),
    case("one-shot-by-first", cd=10, fr=500, mhp=150, mobs=1),
    case("exact-multiple", cd=50, mhp=150, mobs=1),
    case("hp-zero", hp=0),
    case("recovery-only", th=-1, hp=300, mana=200, sc=5, mpr=2, med=10),
    case("unkillable-ish", md=400, hp=60, hpr=15),
    case("heavy-hit", md=200, hp=300, mhp=600, cd=60),
    case("threshold-covers", th=1000),
    case("boss", tl=0, regen=4, mhp=2000, md=120, hp=400),
    case("boss-neg-lairs", tl=-1, regen=1),
    case("boss-short-regen", tl=0, regen=0.001, mhp=3000, cd=30),
    case("instant", tl=-1, regen=0),
    case("instant-move-knob", tl=-1, regen=0, kv=2),
    case("supply-bound", tl=2, regen=5, md=0, cd=500, mhp=80, mobs=1, aw=1),
    case("supply-nonbound", tl=200, regen=5),
    case("no-walk-with-lairs", tl=10, regen=5, aw=0),
    case("xp-knob", kx=0.5),
    case("dmg-knob", kd=2, md=90),
    case("mana-knob", km=2, sc=5, so=2, mana=300, mpr=2, med=10),
    case("move-knob", kv=2),
    case("knobs-zero", kx=0, kd=0, km=0, kv=0, sc=5, mana=300),
    case("neg-spell", sc=-8, so=3, mana=300),
    case("mana-only", md=0, sc=10, mana=200, mpr=3, med=20, hp=0),
    case("mana-no-regen", md=0, sc=10, mana=200, hp=0),
    case("both-rest", md=60, sc=8, so=2, mana=150, mpr=2, med=15, hp=200, hpr=30, mhp=400, cd=50),
    case("both-rest-no-mp-drain", md=60, sc=0, mana=150, mpr=2, med=15, hp=200, hpr=30, mhp=400, cd=50),
    case("hp-only-regen", md=0, hp=200, hpr=30),
    case("hp-nat-zero-md", md=0, hp=200, hpr=0),
    case("regen-gate-below", mhr=40, mhp=500, cd=100, mobs=1),
    case("regen-gate-edge", mhr=40, mhp=600, cd=100, mobs=1),
    case("regen-applies", mhr=40, mhp=1200, cd=100, mobs=1),
    case("regen-applies-multi", mhr=40, mhp=2400, cd=100, mobs=3),
    case("regen-clamp", mhr=4000, mhp=1200, cd=100, mobs=1),
    case("min-dmg-tail", mr=10, cd=50, mhp=200, mobs=1),
    case("min-dmg-ge-avg", mr=60, cd=50, mhp=200, mobs=1),
    case("min-dmg-zero-avg", mr=10, cd=0, fr=40, mhp=200, mobs=1),
    case("surprise-100", sd=300, sch=100, cd=60, mhp=500),
    case("surprise-50", sd=300, sch=50, cd=60, mhp=500),
    case("surprise-0-chance", sd=300, sch=0, cd=60, mhp=500),
    case("surprise-neg-chance", sd=300, sch=-20, cd=60, mhp=500),
    case("surprise-over-100", sd=300, sch=150, cd=60, mhp=500),
    case("surprise-oneshot", sd=900, sch=100, cd=60, mhp=500, mobs=1),
    case("surprise-min", sd=300, smin=100, sch=100, cd=60, mhp=500),
    case("surprise-min-ge", sd=300, smin=400, sch=100, cd=60, mhp=500),
    case("surprise-weaker-flag", sd=40, sch=60, cd=60, mhp=500, mobs=1),
    case("surprise-weaker-flag-multi", sd=40, sch=60, cd=60, mhp=500, mobs=2),
    case("surprise-weaker-not-flag", sd=40, sch=60, cd=60, mhp=50, mobs=1),
    case("surprise-no-avg", sd=40, sch=60, cd=0, mhp=500),
    case("sparse-density", tl=1, ps=1000, aw=8),
    case("sparse-density-clamp-low", tl=1, ps=100000, aw=8),
    case("dense-edge", tl=1, ps=19, aw=8),
    case("dense-just-below", tl=1, ps=20, aw=8),
    case("poss-negative", tl=5, ps=-3, aw=8),
    case("lairs-negative-regen", tl=-1, regen=5, aw=8),
    case("slack-applies", hp=0, mana=0, md=0, aw=10),
    case("slack-boss-skip", tl=0, regen=2, hp=0),
]

grids = [
    {"name": "combat",
     "axes": {"cd": [0, 18.5, 60, 150], "fr": [0, 50, 200], "mr": [0, 10, 100],
              "mhp": [0, 10, 150, 900], "mobs": [1, 3]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelC",
              "args": [{"t": "cur", "v": 1200}, 5, "$mobs", 10, 5, 0, "$cd", 300, 45, 40, "$mhp", 0,
                       0, 0, 0, 0, 0, 0, 6, 1.25, 0, 0, 0, "$fr", "$mr"]}},
    {"name": "surprise",
     "axes": {"sd": [0, 20, 80, 400], "smin": [0, 10, 100], "sch": [0, 35, 100], "cd": [30, 90],
              "mobs": [1, 4]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelC",
              "args": [{"t": "cur", "v": 1200}, 5, "$mobs", 10, 5, 0, "$cd", 300, 45, 40, 600, 0,
                       0, 0, 0, 0, 0, 0, 6, 1.25, "$sd", "$smin", "$sch", 0, 0]}},
    {"name": "regen",
     "axes": {"mhr": [0, 10, 100, 5000], "mhp": [200, 700, 3000], "cd": [30, 150], "mobs": [1, 2]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelC",
              "args": [{"t": "cur", "v": 1200}, 5, "$mobs", 10, 5, 0, "$cd", 300, 45, 40, "$mhp",
                       "$mhr", 0, 0, 0, 0, 0, 0, 6, 1.25, 0, 0, 0, 0, 0]}},
]

lair_draw = {
    "exp": {"choice": [150, 1200.5, 8800, 45000, 250000, 6000.25]},
    "regen": {"choice": [0, 1, 5, 15, 60, 0.5]},
    "mobs": {"choice": [1, 2, 3, 5, 8]},
    "tl": {"choice": [-1, 1, 3, 10, 40, 200]},
    "ps": {"choice": [0, 5, 50, 800, 20000]},
    "rtk": {"choice": [0, 2, 8]},
    "cd": {"choice": [0, 18.5, 60, 150, 400]},
    "hp": {"int": [30, 1500]},
    "hpr": {"choice": [0, 3, 15, 45, 90, 240]},
    "md": {"choice": [0, 3.5, 12, 30, 75.25, 160, 400]},
    "mhp": {"int": [10, 4000]},
    "mhr": {"choice": [0, 5, 40, 400]},
    "th": {"choice": [-1, 0, 0, 5, 20, 60]},
    "sc": {"choice": [0, 0, 3, 8, 20]},
    "so": {"choice": [0, 0, 2, 5.5]},
    "mana": {"choice": [0, 0, 50, 200, 1200, 5000]},
    "mpr": {"choice": [0, 2, 8, 25]},
    "med": {"choice": [0, 0, 10, 30, 80]},
    "aw": {"choice": [0, 2, 5.5, 12, 30]},
    "ws": {"choice": [0, 1, 1.25, 2.5]},
    "sd": {"choice": [0, 40, 300]},
    "smin": {"choice": [0, 10, 150]},
    "sch": {"choice": [0, 50, 100]},
    "fr": {"choice": [0, 50, 200]},
    "mr": {"choice": [0, 10, 100]},
    "kx": {"choice": [0.5, 0.9, 1, 1.25, 2]},
    "kd": {"choice": [0.5, 1, 1.5]},
    "km": {"choice": [0.8, 1, 1.3]},
    "kv": {"choice": [0.5, 1, 2]},
}
boss_draw = {
    "exp": {"choice": [5000, 60000, 900000]},
    "regen": {"choice": [0.25, 1, 4, 12]},
    "mobs": {"choice": [1, 2, 4]},
    "tl": {"choice": [-1, 0]},
    "cd": {"choice": [40, 120, 300]},
    "hp": {"int": [200, 2000]},
    "hpr": {"choice": [15, 90, 240]},
    "md": {"choice": [10, 60, 200]},
    "mhp": {"int": [500, 20000]},
    "th": {"choice": [0, 10]},
    "sc": {"choice": [0, 10]},
    "mana": {"choice": [0, 400]},
    "mpr": {"choice": [0, 6]},
    "med": {"choice": [0, 30]},
    "aw": {"choice": [0, 10]},
}
samples = [
    {"name": "lair", "seed": 30, "count": 320, "draw": lair_draw,
     "case": {"globals": {"nGlobal_cephXP_Knob": "$kx", "nGlobal_cephDMG_Knob": "$kd",
                          "nGlobal_cephMana_Knob": "$km", "nGlobal_cephMove_Knob": "$kv"},
              "proc": "ceph_ModelC", "args": args_of(BASE, True)}},
    {"name": "boss", "seed": 31, "count": 50, "draw": boss_draw,
     "case": {"globals": KNOBS1, "proc": "ceph_ModelC",
              "args": [{"t": "cur", "v": "$exp"}, "$regen", "$mobs", "$tl", 0, 0, "$cd", "$hp", "$hpr",
                       "$md", "$mhp", 0, "$th", "$sc", 0, "$mana", "$mpr", "$med", "$aw", 1.25,
                       0, 0, 0, 0, 0]}},
]

doc = {
    "family": "exphr_c",
    "description": "modExpPerHour ceph_ModelC (Model C) called directly with all 25 CalcExpPerHour-style arguments.",
    "notes": [
        "Calls the Private ceph_ModelC directly (not via CalcExpPerHour, which blends/overwrites the model's text). Args are positional: nExp (Currency), nRegenTime, nNumMobs, nTotalLairs, nPossSpawns, nRTK (ignored by C), nCharDMG, nCharHP, nCharHPRegen, nMobDmg, nMobHP, nMobHPRegen, nDamageThreshold, nSpellCost, nSpellOverhead, nCharMana (Integer), nCharMPRegen, nMeditateRate, nAvgWalk, nWalkSpeed, nSurpriseDMG, nSurpriseMinDMG, nSurpriseChance, nCharFirstRoundDMG, nMinRoundDMG.",
        "The four knobs nGlobal_ceph{XP,DMG,Mana,Move}_Knob are per-case globals (the oracle resets them to 0 otherwise). bGreaterMUD stays False (stock path only).",
    ],
    "tolerance": {"default": 0},
    "cases": cases,
    "grids": grids,
    "samples": samples,
}
with open(sys.argv[1], "w", newline="\n") as f:
    json.dump(doc, f, indent=1)
    f.write("\n")

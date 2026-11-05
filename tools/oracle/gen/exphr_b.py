"""Writes fixtures/scenarios/exphr_b.json: python3 -I tools/oracle/gen/exphr_b.py fixtures/scenarios/exphr_b.json

Scenarios for modExpPerHour ceph_ModelB (called directly, 21 positional arguments in the
CalcExpPerHour order; Model B takes no surprise-minimum, chance, first-round or min-round values).
Deterministic: grids are fixed, samples use the oracle's splitmix64 draws.
"""
import json, sys

KNOBS1 = {"nGlobal_cephXP_Knob": 1, "nGlobal_cephDMG_Knob": 1, "nGlobal_cephMana_Knob": 1,
          "nGlobal_cephMove_Knob": 1}
ARGS = ["exp", "regen", "mobs", "tl", "ps", "rtk", "cd", "hp", "hpr", "md", "mhp", "mhr", "th", "sc",
        "so", "mana", "mpr", "med", "aw", "ws", "sd"]
BASE = dict(exp=1200, regen=5, mobs=2, tl=10, ps=50, rtk=0, cd=100, hp=300, hpr=45, md=40, mhp=150,
            mhr=0, th=0, sc=0, so=0, mana=0, mpr=0, med=0, aw=6, ws=1.25, sd=0)


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
    return {"id": id_, "globals": knobs, "proc": "ceph_ModelB", "args": args_of(d)}


# A no-meditate caster in the mid band (lairs 28-40, walk 2.4-3.3, 2-4 rooms per lair) with a
# near-one-round kill: the case the MB1..MB5 patches were tuned for.
MICRO = dict(tl=35, ps=100, aw=2.9, sc=6, so=0, mana=300, mpr=4, med=0, rtk=1, mobs=1, cd=200, mhp=150,
             md=20, hp=400, hpr=40)

cases = [
    case("typical"),
    case("exp-zero", exp=0),
    case("mana-overflow", sc=5, mana=40000),
    case("mana-max", sc=5, mana=32767, mpr=3, med=20),
    case("mana-neg", sc=5, mana=-5, mpr=3, med=20),
    case("spellcost-overflow", sc=40000, mana=100),
    case("boss", tl=0, regen=4),
    case("boss-neg-lairs", tl=-1, regen=1),
    case("boss-tiny-regen", tl=0, regen=0.001, exp=777),
    case("boss-tie-round", tl=0, regen=2, exp=5),
    case("boss-exp-zero", tl=0, regen=4, exp=0),
    case("zero-lairs-zero-regen", tl=0, regen=0),
    case("zero-lairs-negative-regen", tl=0, regen=-2),
    case("instant", tl=-1, regen=0),
    case("instant-negative-regen", tl=-1, regen=-3),
    case("instant-caster", tl=-1, regen=0, sc=5, mana=100, mpr=3, med=0, mhp=60, cd=300, mobs=1),
    case("instant-caster-med", tl=-1, regen=0, sc=5, mana=100, mpr=3, med=20, mhp=60, cd=300, mobs=1),
    case("instant-caster-no-regen", tl=-1, regen=0, sc=5, mana=100, mpr=0, med=0, mhp=60, cd=300, mobs=1),
    case("instant-big-loop", tl=-1, regen=0, mhp=3000, cd=20),
    case("instant-move-knob", tl=-1, regen=0, kv=2),
    case("mobs-zero", mobs=0),
    case("mobs-negative", mobs=-3),
    case("mobs-fraction", mobs=2.5, mhp=300),
    case("mobs-half", mobs=0.5, sd=300),
    case("rtk-derived-single", rtk=0, mobs=1, cd=37, mhp=410),
    case("rtk-derived-multi", rtk=0, mobs=3, cd=37, mhp=410),
    case("rtk-derived-multi-exact", rtk=0, mobs=2, cd=50, mhp=200),
    case("rtk-derived-no-dmg", rtk=0, cd=0),
    case("rtk-derived-no-hp", rtk=0, mhp=0),
    case("rtk-negative", rtk=-2),
    case("rtk-given-fraction", rtk=1.37),
    case("rtk-given-large", rtk=14.2),
    case("rtk-given-integer", rtk=3),
    case("rtk-one-shot-melee", rtk=1, cd=500, mhp=100, mobs=1),
    case("rtk-near-one", rtk=1.2),
    case("rtk-spell-fraction", rtk=2.3, sc=5, mana=200, mpr=3, med=10),
    case("rtk-spell-small", rtk=0.6, sc=5, mana=200, mpr=3, med=10),
    case("walkspeed-zero", ws=0),
    case("walkspeed-neg", ws=-2),
    case("walk-zero", aw=0),
    case("walk-log-error", aw=-1),
    case("walk-log-error-neg", aw=-4),
    case("walk-neg-ok", aw=-0.5),
    case("no-dmg", cd=0),
    case("neg-dmg", cd=-40),
    case("overkill-exp-overflow", cd=-30000, mhp=100, sc=5, mana=100, mpr=3, med=10),
    case("overkill-melee-neg-dmg", cd=-30000, mhp=100),
    case("mobhp-zero", mhp=0),
    case("mobhp-negative", mhp=-50),
    case("hp-zero", hp=0),
    case("hpregen-zero", hpr=0),
    case("hpregen-zero-heavy", hpr=0, md=200),
    case("basic-damage", th=-1, md=300),
    case("basic-damage-caster", th=-1, sc=8, mana=100, mpr=3, med=10, md=300),
    case("threshold-covers", th=1000),
    case("threshold-partial", th=20, md=100),
    case("heavy-hit", md=400, hp=300, mhp=600, cd=60),
    case("brutal-short-walk", md=400, hp=300, mhp=600, cd=60, aw=2, tl=8, mobs=1),
    case("crowd-light", mobs=6, md=12, tl=12, aw=3),
    case("crowd-light-pack", mobs=12, md=12, tl=12, aw=3, mhp=400),
    case("tiny-chain-long-walk", tl=6, aw=9, md=60, hp=500, hpr=40),
    case("heavy-band", tl=10, aw=6.5, md=150, hp=500, hpr=40),
    case("midcut-band", tl=14, aw=5.5),
    case("midcut-longwalk", tl=14, aw=6.5),
    case("midcut-tinychain", tl=12, aw=6.5),
    case("midcut-tinychain-edge", tl=13, aw=6),
    case("midcut-edge-low", tl=12, aw=5),
    case("midcut-edge-high", tl=16, aw=5),
    case("midcut-just-out", tl=17, aw=5),
    case("long-route-damp", tl=20, aw=14),
    case("huge-chain", tl=70, ps=210, aw=3),
    case("huge-chain-sparse", tl=70, ps=60, aw=3),
    case("big-chain-short-walk", tl=40, ps=40, aw=2),
    case("big-chain-micro-walk", tl=45, ps=2000, aw=1.4),
    case("dense-micro", tl=10, ps=900, aw=1.5),
    case("dense-ultra", tl=10, ps=1000, aw=1.2, sc=6, mana=300, mpr=4),
    case("dense-ultra-nomed-caster", tl=10, ps=1000, aw=1.5, sc=6, mana=300, mpr=4, med=0),
    case("dense-walk-2", tl=10, ps=900, aw=2.2),
    case("sparse-lairs", tl=50, ps=60, aw=3),
    case("poss-negative", tl=5, ps=-3, aw=8),
    case("poss-zero", tl=5, ps=0, aw=8),
    case("long-walk-dense", tl=15, ps=200, aw=11, sc=3, mana=200, mpr=3, med=0, md=15),
    case("long-walk-relabel", tl=15, ps=200, aw=11, sc=0, so=2, mana=200, mpr=3, med=0, md=30, hp=200),
    case("micro-nomed-caster", **MICRO),
    case("micro-nomed-overhead-only", **dict(MICRO, sc=0, so=4)),
    case("micro-med-caster", **dict(MICRO, med=15)),
    case("micro-two-mobs", **dict(MICRO, mobs=2)),
    case("micro-rtk-two", **dict(MICRO, rtk=2, cd=60, mhp=150)),
    case("micro-edge-lairs-28", **dict(MICRO, tl=28, ps=70)),
    case("micro-edge-lairs-40", **dict(MICRO, tl=40, ps=110)),
    case("micro-ultra-dense-bump", **dict(MICRO, tl=10, ps=900, aw=1.3)),
    case("micro-pool-credit-band", **dict(MICRO, aw=3.2, rtk=1.05)),
    case("surprise-bigger", sd=300, cd=60, mhp=500, mobs=1),
    case("surprise-one-shot", sd=900, cd=60, mhp=500, mobs=1),
    case("surprise-weaker", sd=40, cd=60, mhp=500, mobs=1),
    case("surprise-weaker-multi", sd=40, cd=60, mhp=500, mobs=2),
    case("surprise-weaker-not-flag", sd=40, cd=60, mhp=50, mobs=1),
    case("surprise-no-avg", sd=40, cd=0, mhp=500),
    case("surprise-pack-fade", sd=300, cd=60, mhp=3000, mobs=8),
    case("surprise-pack-fade-big", sd=300, cd=60, mhp=6000, mobs=12),
    case("surprise-regen", sd=300, cd=60, mhp=500, mobs=1, mhr=300),
    case("surprise-regen-weaker", sd=40, cd=60, mhp=500, mobs=1, mhr=300),
    case("surprise-caster", sd=300, cd=60, mhp=500, mobs=1, sc=5, mana=200, mpr=3, med=10),
    case("surprise-equal", sd=60, cd=60, mhp=500, mobs=1),
    case("caster-med", sc=8, mana=200, mpr=3, med=15, mhp=400, cd=50),
    case("caster-no-med", sc=8, mana=200, mpr=3, med=0, mhp=400, cd=50),
    case("caster-no-regen", sc=8, mana=200, mpr=0, med=0, mhp=400, cd=50),
    case("caster-med-no-regen", sc=8, mana=200, mpr=0, med=15, mhp=400, cd=50),
    case("caster-med-big-pool", sc=2, mana=3000, mpr=3, med=15, mhp=200, cd=50),
    case("caster-med-high-rate", sc=8, mana=200, mpr=3, med=300, mhp=400, cd=50),
    case("caster-med-neg-rate", sc=8, mana=200, mpr=3, med=-5, mhp=400, cd=50),
    case("caster-overlap", sc=8, so=2, mana=100, mpr=3, med=10, mhp=400, cd=50, md=6, hp=500, hpr=60),
    case("caster-overlap-heavy", sc=8, so=2, mana=100, mpr=3, med=10, mhp=400, cd=50, md=120, hp=500, hpr=60),
    case("caster-neg-cost", sc=-8, so=3, mana=300, mpr=3, med=10),
    case("overhead-only", sc=0, so=4, mana=200, mpr=3, med=0),
    case("overhead-only-med", sc=0, so=4, mana=200, mpr=3, med=12),
    case("overhead-fraction", sc=0, so=0.5, mana=200, mpr=3, med=0),
    case("overhead-negative", sc=0, so=-2, mana=200, mpr=3, med=0),
    case("mana-only-no-hp", md=0, sc=10, mana=200, mpr=3, med=20, hp=0),
    case("caster-respawn-gate", tl=4, regen=30, sc=5, mana=200, mpr=3, med=0),
    case("caster-respawn-gate-med", tl=4, regen=30, sc=5, mana=100, mpr=3, med=15, mhp=800, cd=40),
    case("respawn-gate", tl=4, regen=30),
    case("respawn-gate-exact-ish", tl=10, regen=1.2),
    case("respawn-no-gate", tl=10, regen=0.1),
    case("xp-overflow", exp=900000000000000, tl=40, regen=0),
    case("xp-large-ok", exp=900000000000000, tl=1, regen=0),
    case("xp-frac", exp=0.0001),
    case("xp-knob", kx=0.5),
    case("dmg-knob", kd=2, md=90),
    case("mana-knob", km=2, sc=5, so=2, mana=300, mpr=2, med=10),
    case("move-knob", kv=2),
    case("knobs-zero", kx=0, kd=0, km=0, kv=0, sc=5, mana=300),
    case("regen-time-fraction", regen=0.5, tl=5),
    case("lairs-one", tl=1),
    case("lairs-large", tl=500, ps=1500, regen=100),
]

MICRO_GRID = {"tl": [32, 38], "ps": [80, 120], "aw": [2.5, 3.1], "sc": [0, 6], "so": [0, 2],
              "med": [0, 12], "rtk": [0.8, 1.05, 1.4], "mobs": [1, 2]}


def full(**ov):
    d = dict(BASE)
    d.update(ov)
    return d


def grid_args(**ov):
    """Argument list for a grid: values given in ov are kept ($refs allowed), the rest are BASE."""
    d = full(**ov)
    return args_of(d)


grids = [
    {"name": "micro",
     "axes": MICRO_GRID,
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(tl="$tl", ps="$ps", aw="$aw", sc="$sc", so="$so", med="$med",
                                rtk="$rtk", mobs="$mobs", mana=300, mpr=4, cd=200, mhp=150, md=20,
                                hp=400, hpr=40)}},
    {"name": "surprise",
     "axes": {"sd": [0, 15, 60, 200, 900], "cd": [30, 90], "mobs": [1, 3, 6, 10], "mhr": [0, 60],
              "mhp": [150, 600]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(sd="$sd", cd="$cd", mobs="$mobs", mhr="$mhr", mhp="$mhp", tl=12, ps=40,
                                aw=3)}},
    {"name": "tiny-chain",
     "axes": {"tl": [6, 10, 14, 20], "aw": [1.5, 5.5, 7, 11], "md": [8, 30, 100], "mobs": [1, 3],
              "hpr": [0, 45]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(tl="$tl", aw="$aw", md="$md", mobs="$mobs", hpr="$hpr", hp=450)}},
    {"name": "route",
     "axes": {"tl": [3, 12, 16, 30, 36, 44, 62, 75, 90], "aw": [1.2, 1.7, 2.6, 3.4, 4.1, 8],
              "ps": [0, 25, 150, 1800]},
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(tl="$tl", aw="$aw", ps="$ps", sc=4, mana=250, mpr=3, med=0)}},
]

lair_draw = {
    "exp": {"choice": [150, 1200.5, 8800, 45000, 250000, 6000.25]},
    "regen": {"choice": [0, 1, 5, 15, 60, 0.5, 240]},
    "mobs": {"choice": [0.5, 1, 1, 1.3, 2, 3, 5, 8, 12]},
    "tl": {"choice": [-1, 1, 3, 6, 10, 13, 14, 20, 30, 34, 36, 38, 42, 50, 70, 200]},
    "ps": {"choice": [0, 3, 20, 50, 70, 110, 150, 400, 2000]},
    "rtk": {"choice": [0, 0, 0.6, 1, 1.15, 1.5, 2.3, 8, 25]},
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
    "aw": {"choice": [0, 1.2, 1.5, 2.5, 2.8, 3.2, 5.5, 6.5, 12, 30, -0.5, -1]},
    "ws": {"choice": [0, 1, 1.25, 2.5, -1]},
    "sd": {"choice": [0, 0, 40, 300, 2000]},
    "kx": {"choice": [0.5, 0.9, 1, 1.25, 2]},
    "kd": {"choice": [0.5, 1, 1.5]},
    "km": {"choice": [0.8, 1, 1.3]},
    "kv": {"choice": [0.5, 1, 2]},
}
boss_draw = {
    "exp": {"choice": [5000, 60000, 900000, 12.5]},
    "regen": {"choice": [0.25, 1, 4, 12]},
    "tl": {"choice": [-1, 0]},
}
# Mid-band casters near one round: the region MB1..MB5 act in.
micro_draw = {
    "tl": {"int": [26, 42]},
    "ps": {"int": [40, 190]},
    "aw": {"choice": [2.3, 2.5, 2.7, 2.9, 3.1, 3.3, 3.6]},
    "rtk": {"choice": [0.7, 1, 1.02, 1.08, 1.3]},
    "mobs": {"choice": [1, 1.2, 1.4, 2]},
    "sc": {"choice": [0, 3, 9]},
    "so": {"choice": [0, 1, 4]},
    "mana": {"choice": [60, 300, 900]},
    "mpr": {"choice": [0, 3, 12]},
    "med": {"choice": [0, 0, 10]},
    "cd": {"choice": [120, 250]},
    "mhp": {"int": [60, 260]},
    "md": {"choice": [4, 20, 60]},
    "hp": {"int": [100, 800]},
    "hpr": {"choice": [15, 45, 90]},
    "regen": {"choice": [0, 1, 5]},
    "sd": {"choice": [0, 0, 80, 300]},
}
samples = [
    {"name": "lair", "seed": 40, "count": 500, "draw": lair_draw,
     "case": {"globals": {"nGlobal_cephXP_Knob": "$kx", "nGlobal_cephDMG_Knob": "$kd",
                          "nGlobal_cephMana_Knob": "$km", "nGlobal_cephMove_Knob": "$kv"},
              "proc": "ceph_ModelB", "args": args_of(BASE, True)}},
    {"name": "boss", "seed": 41, "count": 20, "draw": boss_draw,
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(exp="$exp", regen="$regen", tl="$tl")}},
    {"name": "microband", "seed": 42, "count": 150, "draw": micro_draw,
     "case": {"globals": KNOBS1, "proc": "ceph_ModelB",
              "args": grid_args(tl="$tl", ps="$ps", aw="$aw", rtk="$rtk", mobs="$mobs", sc="$sc",
                                so="$so", mana="$mana", mpr="$mpr", med="$med", cd="$cd", mhp="$mhp",
                                md="$md", hp="$hp", hpr="$hpr", regen="$regen", sd="$sd")}},
]

doc = {
    "family": "exphr_b",
    "description": "modExpPerHour ceph_ModelB (Model B) called directly with its 21 CalcExpPerHour-style arguments.",
    "notes": [
        "Calls the Private ceph_ModelB directly (not via CalcExpPerHour, which formats the text). Args are positional: nExp (Currency), nRegenTime, nNumMobs, nTotalLairs, nPossSpawns, nRTK, nCharDMG, nCharHP, nCharHPRegen, nMobDmg, nMobHP, nMobHPRegen, nDamageThreshold, nSpellCost (Integer), nSpellOverhead, nCharMana (Integer), nCharMPRegen, nMeditateRate, nAvgWalk, nWalkSpeed, nSurpriseDMG. Model B leaves every text field empty.",
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

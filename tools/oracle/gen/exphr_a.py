"""Writes fixtures/scenarios/exphr_a.json: python3 -I tools/oracle/gen/exphr_a.py fixtures/scenarios/exphr_a.json
Model A (ceph_ModelA) takes the first 21 CalcExpPerHour arguments. Deterministic (grids and splitmix64 draws)."""
import json, sys

ORDER = ["exp", "regen", "mobs", "tl", "ps", "rtk", "cd", "hp", "hpr", "md", "mhp", "mhr", "th", "sc", "so",
         "mana", "mpr", "med", "aw", "ws", "sd"]
BASE = dict(exp=1200, regen=5, mobs=2, tl=10, ps=5, rtk=2, cd=100, hp=300, hpr=45, md=40, mhp=150, mhr=0, th=0,
            sc=0, so=0, mana=0, mpr=0, med=0, aw=6, ws=1.25, sd=0)
KNOBS = {"nGlobal_cephXP_Knob": 1, "nGlobal_cephDMG_Knob": 1, "nGlobal_cephMana_Knob": 1, "nGlobal_cephMove_Knob": 1}


def args(**kw):
    v = dict(BASE)
    v.update(kw)
    out = []
    for k in ORDER:
        out.append({"t": "cur", "v": v[k]} if k == "exp" else v[k])
    return out


def case(id_, knobs=None, **kw):
    g = dict(KNOBS)
    g.update(knobs or {})
    return {"id": id_, "globals": g, "proc": "ceph_ModelA", "args": args(**kw)}


def grid(name, axes, **kw):
    # kw values may be "$axis" placeholders; the rest are constants over the base lair.
    return {"name": name, "axes": axes, "case": {"globals": dict(KNOBS), "proc": "ceph_ModelA", "args": args(**kw)}}


cases = [
    case("typical"),
    case("exp-zero", exp=0),
    case("exp-fractional", exp=1234.5678),
    case("lairs-and-regen-zero", tl=0, regen=0),
    case("boss", tl=0, regen=3),
    case("boss-short", tl=0, regen=0.25, exp=250000),
    case("boss-negative-lairs", tl=-1, regen=5),
    case("boss-knob-ignored", tl=0, regen=2, knobs={"nGlobal_cephXP_Knob": 2.5}),
    case("boss-surprise", tl=0, regen=4, sd=80, cd=40, mhp=200),
    case("boss-surprise-error", tl=0, regen=4, sd=250, cd=0, mhp=200, rtk=3),
    case("instant", tl=-1, regen=0),
    case("instant-basic", tl=-1, regen=0, th=-1),
    case("regen-over-cap", regen=70),
    case("regen-negative", regen=-3),
    case("regen-cap-boundary", regen=60),
    case("basic-damage", th=-1),
    case("basic-damage-mana", th=-1, sc=10, mana=500, mpr=5, med=30),
    case("basic-damage-limited-movement", th=-1, tl=5, ps=60, aw=1.5),
    case("other-negative-threshold", th=-5),
    case("threshold-covers", th=60, md=40),
    case("threshold-partial", th=25, md=40),
    case("threshold-huge-regen", th=10, md=40, hpr=300),
    case("char-hp-zero", hp=0),
    case("char-hp-negative", hp=-50),
    case("mob-hp-zero", mhp=0, rtk=0),
    case("hp-regen-zero", hpr=0),
    case("hp-regen-negative", hpr=-9),
    case("rtk-zero-derive", rtk=0),
    case("rtk-zero-derive-multi", rtk=0, mobs=3, mhp=450, cd=40),
    case("rtk-zero-derive-small", rtk=0, mhp=50, cd=100),
    case("rtk-zero-derive-exact-one", rtk=0, mhp=100, cd=100),
    case("rtk-zero-no-dmg", rtk=0, cd=0),
    case("rtk-sub-one", rtk=0.4),
    case("rtk-negative", rtk=-3),
    case("rtk-fractional", rtk=2.5, mobs=3),
    case("mobs-zero", mobs=0),
    case("mobs-fractional", mobs=2.5),
    case("mobs-many", mobs=12, rtk=1.5, mhp=1800),
    case("walk-speed-zero", ws=0),
    case("walk-speed-low", ws=0.5),
    case("walk-speed-high", ws=3),
    case("walk-speed-boundary", ws=2),
    case("avg-walk-zero", aw=0),
    case("avg-walk-negative", aw=-4),
    case("cluster", tl=5, ps=60, aw=1.5),
    case("cluster-exact-ratio", tl=5, ps=50, aw=1.5),
    case("cluster-just-below", tl=5, ps=49, aw=1.5),
    case("cluster-walk-boundary", tl=5, ps=60, aw=2),
    case("cluster-walk-too-long", tl=5, ps=60, aw=2.01),
    case("cluster-mana", tl=5, ps=60, aw=1.5, sc=8, mana=300, mpr=4, med=20),
    case("cluster-instant", tl=3, ps=40, aw=1, regen=0),
    case("lairs-single", tl=1, ps=0, regen=5),
    case("lairs-heavy-supply", tl=200, ps=50, regen=1),
    case("poss-spawns-negative", ps=-5),
    case("rooms-raw-zero", tl=10, ps=-10),
    case("rooms-raw-negative", tl=10, ps=-30),
    case("rooms-overflow", tl=10, ps=2147483647),
    case("spawn-gated", tl=2, ps=1, regen=30, rtk=1, mobs=1),
    case("spawn-gated-limited", tl=2, ps=40, aw=1.5, regen=30, rtk=1, mobs=1),
    case("xp-knob", knobs={"nGlobal_cephXP_Knob": 1.7}),
    case("dmg-knob", knobs={"nGlobal_cephDMG_Knob": 1.6}, md=90),
    case("mana-knob", knobs={"nGlobal_cephMana_Knob": 0.6}, sc=8, mana=300, mpr=4, med=20),
    case("move-knob", knobs={"nGlobal_cephMove_Knob": 2}),
    case("move-knob-one-fifth", knobs={"nGlobal_cephMove_Knob": 5}),
    case("move-knob-zero", knobs={"nGlobal_cephMove_Knob": 0}),
    case("move-knob-negative", knobs={"nGlobal_cephMove_Knob": -1}),
    case("dmg-knob-zero", knobs={"nGlobal_cephDMG_Knob": 0}, md=90),
    case("mana-knob-zero", knobs={"nGlobal_cephMana_Knob": 0}, sc=8, mana=300, mpr=4, med=20),
    case("all-knobs-zero", knobs={k: 0 for k in KNOBS}),
    case("mana-direct-spell", sc=10, mana=400, mpr=3, med=25),
    case("mana-overhead-only", so=6, mana=400, mpr=3, med=25),
    case("mana-overhead-only-no-meditate", so=6, mana=400, mpr=3, med=0),
    case("mana-no-regen-no-meditate", sc=10, mana=400),
    case("mana-pool-zero", sc=10, mana=0, mpr=3, med=25),
    case("mana-negative", sc=10, mana=-50, mpr=3, med=25),
    case("mana-regen-covers", sc=1, mana=400, mpr=60, med=25),
    case("mana-negative-cost", sc=-5, mana=400, mpr=3, med=25),
    case("mana-huge", sc=10, mana=2000000000, mpr=3, med=25),
    case("mana-28-lairs-cluster-frac", so=6, mana=400, mpr=3, med=25, tl=28, aw=3, ps=5),
    case("mana-28-lairs-far-walk", so=6, mana=400, mpr=3, med=25, tl=28, aw=4, ps=5),
    case("walk-credit-micro-room", sc=3, mana=300, mpr=6, med=20, rtk=1, mobs=1, md=10),
    case("walk-credit-long-room", sc=3, mana=300, mpr=6, med=20, rtk=8, mobs=2, md=10),
    case("fallback-drip", md=3, th=0, hp=900, hpr=60, tl=40, ps=60, rtk=2, mobs=2),
    case("fallback-drip-sustained", md=3, th=2, hp=900, hpr=300, tl=40, ps=60),
    case("fallback-skipped-mana", md=3, hp=900, hpr=60, tl=40, ps=60, sc=5, mana=300, mpr=3, med=20),
    case("zero-mob-damage", md=0),
    case("negative-mob-damage", md=-20),
    case("tiny-mob-damage", md=0.04),
    case("mob-damage-deadband", md=0.05),
    case("heavy-hit", md=500, hp=200, hpr=30),
    case("char-damage-zero", cd=0, rtk=3),
    case("char-damage-negative", cd=-10, rtk=3),
    case("one-shot-overkill", cd=400, mhp=100, mobs=1, rtk=1),
    case("overshoot-fractional-rtk", cd=70, mhp=100, mobs=1, rtk=1.5),
    case("surprise-better", sd=60, cd=50, mhp=150, mobs=1, rtk=3, md=80),
    case("surprise-one-shot", sd=500, cd=50, mhp=150, mobs=1, rtk=3, md=80),
    case("surprise-worse", sd=10, cd=100, mhp=150, mobs=1, rtk=2, md=80),
    case("surprise-equal", sd=100, cd=100, mhp=150, mobs=1, rtk=2),
    case("surprise-regen-atten", sd=90, cd=50, mhp=300, mobs=1, rtk=6, mhr=100, md=30),
    case("surprise-regen-large", sd=90, cd=50, mhp=300, mobs=1, rtk=6, mhr=500, md=30),
    case("surprise-regen-negative", sd=90, cd=50, mhp=300, mobs=1, rtk=6, mhr=-40, md=30),
    case("surprise-pack-fade", sd=90, cd=50, mhp=1800, mobs=6, rtk=4, md=60),
    case("surprise-pack-fade-full", sd=90, cd=50, mhp=2400, mobs=9, rtk=4, md=60),
    case("surprise-damage-zero-error", sd=400, cd=0, mhp=300, mobs=1, rtk=6),
    case("surprise-damage-zero-error-multi", sd=400, cd=0, mhp=900, mobs=3, rtk=6),
    case("surprise-damage-zero-mob-regen-zero", sd=400, cd=0, mhp=300, mobs=1, rtk=6, mhr=0),
    case("surprise-damage-negative", sd=400, cd=-5, mhp=300, mobs=1, rtk=6),
    case("surprise-damage-negative-small", sd=90, cd=-5, mhp=300, mobs=1, rtk=6),
    case("surprise-damage-zero-no-saving", sd=90, cd=0, mhp=300, mobs=1, rtk=1),
    case("surprise-negative-ignored", sd=-50),
]

grids = [
    # Combat and recovery: rounds, pack size, damage, HP and threshold (including basic damage).
    grid("combat", {"rtk": [0, 1, 2.5, 6], "mobs": [1, 3, 6], "md": [0, 20, 120], "hp": [100, 600], "th": [0, 25, -1]},
         rtk="$rtk", mobs="$mobs", md="$md", hp="$hp", th="$th", mhp=300, hpr=90, cd=60),
    # Mana pool, meditate and instant / boss variants.
    grid("mana", {"sc": [0, 10], "mana": [0, 100, 2000], "med": [0, 40], "mpr": [0, 10], "tl": [-1, 5], "regen": [0, 10]},
         sc="$sc", mana="$mana", med="$med", mpr="$mpr", tl="$tl", regen="$regen", so=1.5, mobs=3, cd=100, hp=250, md=30),
    # Movement: lair density, cluster detection, regen, route length.
    grid("movement", {"tl": [1, 10, 40, 200], "ps": [0, 10, 100, 1000], "regen": [0, 2, 30], "aw": [0, 1.5, 6]},
         tl="$tl", ps="$ps", regen="$regen", aw="$aw", sc=2, mana=200, mpr=4, med=20, so=1),
    grid("walk-speed", {"ws": [0.5, 1.25, 2.5], "tl": [10, 40], "ps": [3, 30, 200], "regen": [1, 8], "knob": [1, 0.5]},
         ws="$ws", tl="$tl", ps="$ps", regen="$regen"),
    # Surprise opener against pack size, damage and mob regen (cd 0 hits the IIf division error).
    grid("surprise", {"sd": [1, 20, 60, 100, 300, 1000], "cd": [0, 15, 50, 100], "mobs": [1, 2, 4, 8], "mhr": [0, 10, 60]},
         sd="$sd", cd="$cd", mobs="$mobs", mhr="$mhr", mhp=400, rtk=6, md=45),
]
# 'knob' above is only an axis label that doubles the grid; make it effective on the move knob.
grids[3]["case"]["globals"]["nGlobal_cephMove_Knob"] = "$knob"

samples = [
    {"name": "lair", "seed": 22, "count": 320,
     "draw": {"exp": {"choice": [150, 1200.5, 8800, 45000, 250000, 6000.25]},
              "regen": {"choice": [0, 1, 5, 15, 60, 0.5]},
              "mobs": {"choice": [1, 2, 3, 5, 8]},
              "tl": {"choice": [-1, 1, 3, 10, 40, 200]},
              "ps": {"choice": [0, 5, 50, 400]},
              "rtk": {"choice": [0, 1, 1.5, 2, 3, 4.5, 8, 14, 25]},
              "cd": {"choice": [0, 18.5, 60, 150, 400]},
              "hp": {"int": [30, 1500]},
              "hpr": {"choice": [0, 3, 15, 45, 90, 240]},
              "md": {"choice": [0, 3.5, 12, 30, 75.25, 160, 400]},
              "mhp": {"int": [10, 4000]},
              "mhr": {"choice": [0, 5, 40]},
              "th": {"choice": [-1, 0, 0, 5, 20, 60]},
              "sc": {"choice": [0, 0, 3, 8, 20]},
              "so": {"choice": [0, 0, 2, 5.5]},
              "mana": {"choice": [0, 0, 50, 200, 1200, 5000]},
              "mpr": {"choice": [0, 2, 8, 25]},
              "med": {"choice": [0, 0, 10, 30, 80]},
              "aw": {"choice": [0, 1.5, 2, 5.5, 12, 30]},
              "ws": {"choice": [0, 1, 1.25, 2.5]},
              "sd": {"choice": [0, 0, 40, 130, 300]},
              "kx": {"choice": [0.5, 0.9, 1, 1.25, 2]},
              "kd": {"choice": [0.5, 1, 1.5]},
              "km": {"choice": [0.8, 1, 1.3]},
              "kv": {"choice": [0.5, 1, 2]}},
     "case": {"globals": {"nGlobal_cephXP_Knob": "$kx", "nGlobal_cephDMG_Knob": "$kd",
                          "nGlobal_cephMana_Knob": "$km", "nGlobal_cephMove_Knob": "$kv"},
              "proc": "ceph_ModelA",
              "args": args(**{k: "$" + k for k in ORDER})},
     },
    # Realistic farming rooms: a character who fights, rests and walks, no degenerate zeros.
    {"name": "farm", "seed": 23, "count": 220,
     "draw": {"exp": {"choice": [900, 3500, 12000, 60000, 400000]},
              "regen": {"choice": [1, 2, 5, 10, 20, 45]},
              "mobs": {"choice": [1, 2, 3, 4]},
              "tl": {"int": [2, 60]},
              "ps": {"int": [0, 80]},
              "rtk": {"choice": [1, 1.5, 2, 2.5, 3, 5, 9]},
              "cd": {"int": [20, 220]},
              "hp": {"int": [120, 900]},
              "hpr": {"choice": [9, 21, 36, 60, 120]},
              "md": {"int": [5, 200]},
              "mhp": {"int": [40, 900]},
              "mhr": {"choice": [0, 0, 8, 30]},
              "th": {"choice": [0, 0, 0, 8, 30]},
              "sc": {"choice": [0, 0, 4, 9]},
              "so": {"choice": [0, 1, 3]},
              "mana": {"choice": [0, 80, 250, 700]},
              "mpr": {"choice": [0, 3, 9]},
              "med": {"choice": [0, 15, 40]},
              "aw": {"int": [1, 18]},
              "ws": {"choice": [1, 1.25, 1.5, 2]},
              "sd": {"choice": [0, 0, 0, 70, 160]}},
     "case": {"globals": dict(KNOBS), "proc": "ceph_ModelA", "args": args(**{k: "$" + k for k in ORDER})}},
    {"name": "boss", "seed": 24, "count": 40,
     "draw": {"exp": {"choice": [5000, 60000, 900000]}, "regen": {"choice": [0.25, 1, 4, 12]},
              "mobs": {"choice": [1, 2, 4]}, "tl": {"choice": [-1, 0]}, "rtk": {"choice": [0, 3, 8, 20, 40]},
              "cd": {"choice": [40, 120, 300]}, "hp": {"int": [200, 2000]}, "hpr": {"choice": [15, 90, 240]},
              "md": {"choice": [10, 60, 200]}, "mhp": {"int": [500, 20000]}, "th": {"choice": [0, 10]},
              "sd": {"choice": [0, 100]}},
     "case": {"globals": dict(KNOBS), "proc": "ceph_ModelA",
              "args": args(exp="$exp", regen="$regen", mobs="$mobs", tl="$tl", rtk="$rtk", cd="$cd", hp="$hp",
                           hpr="$hpr", md="$md", mhp="$mhp", th="$th", sd="$sd")}},
]

scn = {
    "family": "exphr_a",
    "description": "modExpPerHour ceph_ModelA (Model A) called directly with its 21 CalcExpPerHour-style arguments.",
    "notes": [
        "Calls the Private ceph_ModelA directly (not via CalcExpPerHour, which averages the enabled models). Args are positional: nExp (Currency), nRegenTime, nNumMobs, nTotalLairs, nPossSpawns, nRTK, nCharDMG, nCharHP, nCharHPRegen, nMobDmg, nMobHP, nMobHPRegen, nDamageThreshold, nSpellCost, nSpellOverhead, nCharMana, nCharMPRegen, nMeditateRate, nAvgWalk, nWalkSpeed, nSurpriseDMG.",
        "The four knobs nGlobal_ceph{XP,DMG,Mana,Move}_Knob are per-case globals (the oracle resets them to 0 otherwise) and are recorded in inputs.globals.",
        "ceph_ModelA swallows VB6 run-time errors (On Error ... Resume out), so a failing case yields an empty record (all fields 0) plus an event, never an escaped error.",
    ],
    "tolerance": {"default": 0},
    "cases": cases,
    "grids": grids,
    "samples": samples,
}
with open(sys.argv[1], "w") as f:
    json.dump(scn, f, indent=None, separators=(",", ":"))
    f.write("\n")

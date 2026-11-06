"""Writes fixtures/scenarios/exp_dispatch.json: python3 -I tools/oracle/gen/exp_dispatch.py fixtures/scenarios/exp_dispatch.json

Scenarios for modExpPerHour CalcExpPerHour (the public dispatcher), called with all 25 positional
arguments. Each case sets the model flags, the recovery-only and show-all flags and the four knobs
as globals. The lair shapes follow the model families (exphr_a..d); the model x flag x knob
grids are written out here, and a seeded sample adds random lairs with random settings.
Deterministic: nothing here is random, the sample uses the oracle's splitmix64 draws.
"""
import json, sys

ARGS = ["exp", "regen", "mobs", "tl", "ps", "rtk", "cd", "hp", "hpr", "md", "mhp", "mhr", "th", "sc",
        "so", "mana", "mpr", "med", "aw", "ws", "sd", "smin", "sch", "fr", "mr"]
BASE = dict(exp=1200, regen=5, mobs=2, tl=10, ps=5, rtk=0, cd=100, hp=300, hpr=45, md=40, mhp=150,
            mhr=0, th=0, sc=0, so=0, mana=0, mpr=0, med=0, aw=6, ws=1.25, sd=0, smin=0, sch=0,
            fr=0, mr=0)
KNOB_NAMES = ("nGlobal_cephXP_Knob", "nGlobal_cephDMG_Knob", "nGlobal_cephMana_Knob",
              "nGlobal_cephMove_Knob")
MODEL_NAMES = ("bGlobal_cephModelA", "bGlobal_cephModelB", "bGlobal_cephModelC",
               "bGlobal_cephModelD")


def args_of(d):
    return [{"t": "cur", "v": d[k]} if k == "exp" else d[k] for k in ARGS]


def settings(models=(1, 1, 1, 1), show=0, rec=0, knobs=(1, 1, 1, 1)):
    g = {n: bool(m) for n, m in zip(MODEL_NAMES, models)}
    g["bGlobal_cephShowAll"] = bool(show)
    g["bGlobal_cephRecoveryOnly"] = bool(rec)
    g.update({n: k for n, k in zip(KNOB_NAMES, knobs)})
    return g


def case(id_, over=None, **kw):
    d = dict(BASE)
    d.update(over or {})
    return {"id": id_, "globals": settings(**kw), "proc": "CalcExpPerHour", "args": args_of(d)}


# Lair shapes: the same families the model fixtures cover (typical, caster, boss, surprise, ...).
LAIRS = {
    "typical": {},
    "caster": dict(sc=8, so=2, mana=300, mpr=4, med=20),
    "mana-only": dict(md=0, sc=10, mana=200, mpr=3, med=20, hp=0),
    "both-rest": dict(md=60, sc=8, so=2, mana=150, mpr=2, med=15, hp=500, hpr=60, mhp=400, cd=50),
    "boss": dict(tl=0, regen=4, mhp=2000, md=30, hp=1200, hpr=90),
    "instant": dict(tl=-1, regen=0),
    "cluster": dict(tl=5, ps=60, aw=2, sc=3, mana=150, mpr=3, med=10),
    "sparse": dict(tl=2, ps=1000, aw=12, regen=30),
    "heavy": dict(md=60, hp=900, hpr=90, mhp=600, cd=60),
    "surprise": dict(sd=90, smin=20, sch=70, cd=50, mhp=300, mobs=1, rtk=3, md=60),
    "first-round": dict(fr=200, mr=15, cd=40, mhp=500, mobs=1, rtk=4),
    "pack": dict(mobs=6, mhp=1800, md=40, hp=1500, hpr=60, cd=80, rtk=3, sd=60, sch=50),
    "farm": dict(exp=12000, regen=10, mobs=3, tl=30, ps=40, rtk=2, cd=120, hp=500, hpr=60, md=45,
                 mhp=450, mhr=8, sc=4, mana=250, mpr=3, med=15, aw=5, sd=70, sch=60),
    "regen-mob": dict(mhr=100, mhp=1200, cd=100, mobs=1, rtk=6),
    "slow-walk": dict(ws=2.5, tl=40, ps=30, aw=9),
    "threshold": dict(th=25, md=60, hp=600, hpr=90),
}
COMBOS = [tuple((n >> b) & 1 for b in range(4)) for n in range(16)]
KNOB_SETS = [(1, 1, 1, 1), (0.5, 1, 1, 1), (1, 2, 1, 1), (1, 1, 0.6, 1), (1, 1, 1, 2),
             (1.7, 1.5, 0.8, 0.5), (0, 0, 0, 0), (10, 10, 10, 10)]

cases = [
    # The first cases pin the defaults: MME's Form_Load starts with A, B and C on and D off, but
    # LoadExpPerHourKnobs (run right after, with an absent key defaulting to 1) turns all four on.
    case("default-all-four-on", models=(1, 1, 1, 1), show=0),
    case("default-all-four-on-show-all", models=(1, 1, 1, 1), show=1),
    case("form-load-a-b-c", models=(1, 1, 1, 0), show=0),
    case("form-load-a-b-c-show-all", models=(1, 1, 1, 0), show=1),
    case("no-model-ticked", models=(0, 0, 0, 0), show=1),
    case("no-model-ticked-plain", models=(0, 0, 0, 0), show=0),
    case("typical-d-only", models=(0, 0, 0, 1)),
    # Early exits and error paths of CalcExpPerHour itself.
    case("exp-zero", dict(exp=0)),
    case("exp-zero-no-models", dict(exp=0), models=(0, 0, 0, 0)),
    case("exp-fraction", dict(exp=0.5)),
    case("exp-negative", dict(exp=-500)),
    case("char-damage-zero", dict(cd=0), show=1),
    case("char-damage-negative", dict(cd=-10), show=1),
    case("mob-hp-zero", dict(mhp=0), show=1),
    case("mob-hp-negative", dict(mhp=-50), show=1),
    case("not-killable-damage", dict(md=400, hp=60, hpr=15, mhp=900), show=1),
    case("not-killable-no-models", dict(md=400, hp=60, hpr=15, mhp=900), models=(0, 0, 0, 0)),
    case("not-killable-regen-beats-damage", dict(mhr=4000, mhp=1200, cd=20, mobs=1)),
    case("not-killable-too-long", dict(cd=1, mhp=4000, mobs=1)),
    case("killable-by-surprise-offset", dict(sd=3000, cd=0, mhp=3000, mobs=1)),
    case("surprise-exceeds-hp", dict(sd=900, mhp=500, cd=0)),
    case("surprise-fraction-hp", dict(sd=10.5, mhp=150)),
    case("surprise-fraction-hp-half-down", dict(sd=0.5, mhp=150)),
    case("surprise-fraction-hp-half-up", dict(sd=1.5, mhp=150)),
    case("mob-hp-minus-surprise-overflow", dict(sd=-3000000000, mhp=2000000000)),
    case("char-hp-regen-integer-overflow", dict(hpr=40000)),
    case("char-hp-regen-integer-max", dict(hpr=32767)),
    case("char-hp-regen-integer-min", dict(hpr=-32768)),
    case("char-hp-regen-negative", dict(hpr=-30)),
    case("spell-cost-overflow", dict(sc=40000, mana=100)),
    case("surprise-chance-overflow", dict(sd=40, sch=40000)),
    case("mana-long-overflow-in-model", dict(sc=5, mana=40000, mpr=3, med=20), show=1),
    case("mana-max", dict(sc=5, mana=32767, mpr=3, med=20), show=1),
    case("mob-damage-zero", dict(md=0), show=1),
    case("mob-damage-negative", dict(md=-20), show=1),
    case("mobs-zero", dict(mobs=0), show=1),
    case("walk-speed-zero", dict(ws=0), show=1),
    case("walk-zero", dict(aw=0), show=1),
    case("poss-spawns-negative", dict(ps=-5), show=1),
    case("lairs-negative-regen", dict(tl=-1, regen=5), show=1),
    case("regen-zero-lairs-zero", dict(tl=0, regen=0), show=1),
    case("boss-negative-lairs", dict(tl=-1, regen=1), show=1),
    # Text composition: the flags and thresholds that choose each sentence.
    case("text-recovery-only", dict(sc=5, so=1, mana=200, mpr=2, med=10), rec=1, show=1),
    case("text-recovery-only-d", dict(sc=5, so=1, mana=200, mpr=2, med=10), rec=1, models=(0, 0, 0, 1)),
    case("text-overkill", dict(cd=400, mhp=100, mobs=1, rtk=1), show=1),
    case("text-overkill-huge-damage", dict(cd=10000000, mhp=100, mobs=1, rtk=1), show=1),
    case("text-backstab-worse", dict(sd=10, cd=100, mhp=150, mobs=1, rtk=2, md=80), show=1),
    case("text-cluster-limited", dict(tl=5, ps=60, aw=1.5, sc=8, mana=300, mpr=4, med=20), show=1),
    case("text-insufficient-lairs", dict(tl=2, regen=30, rtk=1, mobs=1, ps=1, aw=1.5), show=1),
    case("text-large-exp", dict(exp=900000000, regen=1, mobs=1), show=1),
    case("text-million-exp", dict(exp=20000000, regen=3, mobs=1, tl=4), show=1),
    case("text-thousand-exp", dict(exp=700, regen=60, mobs=1, tl=3), show=1),
    case("text-small-exp", dict(exp=3, regen=60, mobs=1, tl=3), show=1),
    case("text-tiny-exp", dict(exp=0.01, regen=60, mobs=1, tl=3), show=1),
]

# Every lair shape under every model combination, with and without the per-model breakdown.
for lair in ("typical", "caster", "both-rest", "boss", "instant", "cluster", "heavy", "surprise",
             "first-round", "pack", "farm", "threshold"):
    for show in (1, 0):
        for m in COMBOS:
            cases.append(case("combo-%s-%s-show%d" % (lair, "".join(map(str, m)), show),
                              LAIRS[lair], models=m, show=show))

# Recovery-only (basic damage) mode against a spread of lairs and model sets.
for lair in ("typical", "caster", "mana-only", "both-rest", "heavy", "farm", "threshold", "pack"):
    for m in ((1, 1, 1, 1), (1, 1, 1, 0), (0, 0, 1, 1), (1, 0, 0, 0)):
        cases.append(case("recovery-%s-%s" % (lair, "".join(map(str, m))), LAIRS[lair],
                          models=m, rec=1, show=1))

# The four knobs against all-on and single-model settings.
for lair in ("typical", "caster", "both-rest", "boss", "cluster", "surprise", "farm", "slow-walk"):
    for ki, k in enumerate(KNOB_SETS):
        cases.append(case("knobs-%s-%d" % (lair, ki), LAIRS[lair], knobs=k, show=1))
    for ki in (1, 5):
        cases.append(case("knobs-%s-%d-d-only" % (lair, ki), LAIRS[lair], knobs=KNOB_SETS[ki],
                          models=(0, 0, 0, 1)))

# Seeded random lairs with random flags and knobs.
lair_draw = {
    "exp": {"choice": [150, 1200.5, 8800, 45000, 250000, 6000.25]},
    "regen": {"choice": [0, 1, 5, 15, 60, 0.5]},
    "mobs": {"choice": [1, 2, 3, 5, 8]},
    "tl": {"choice": [-1, 1, 3, 10, 40, 200]},
    "ps": {"choice": [0, 5, 50, 800]},
    "rtk": {"choice": [0, 2, 8]},
    "cd": {"choice": [0, 18.5, 60, 100, 150, 400]},
    "hp": {"int": [300, 3000]},
    "hpr": {"choice": [0, 3, 15, 45, 90, 240]},
    "md": {"choice": [0, 3.5, 12, 30, 45, 75.25, 160]},
    "mhp": {"int": [10, 2000]},
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
    "ma": {"choice": [True, True, False]},
    "mb": {"choice": [True, True, False]},
    "mc": {"choice": [True, True, False]},
    "md_on": {"choice": [True, False]},
    "show": {"choice": [True, False]},
    "rec": {"choice": [False, False, False, True]},
    "kx": {"choice": [0.5, 0.9, 1, 1.25, 2]},
    "kd": {"choice": [0.5, 1, 1.5]},
    "km": {"choice": [0.8, 1, 1.3]},
    "kv": {"choice": [0.5, 1, 2]},
}
sample_case = {
    "globals": {"bGlobal_cephModelA": "$ma", "bGlobal_cephModelB": "$mb", "bGlobal_cephModelC": "$mc",
                "bGlobal_cephModelD": "$md_on", "bGlobal_cephShowAll": "$show",
                "bGlobal_cephRecoveryOnly": "$rec", "nGlobal_cephXP_Knob": "$kx",
                "nGlobal_cephDMG_Knob": "$kd", "nGlobal_cephMana_Knob": "$km",
                "nGlobal_cephMove_Knob": "$kv"},
    "proc": "CalcExpPerHour",
    "args": [{"t": "cur", "v": "$exp"}] + ["$" + k for k in ARGS[1:]],
}
samples = [{"name": "lair", "seed": 260, "count": 140, "draw": lair_draw, "case": sample_case}]

doc = {
    "family": "exp_dispatch",
    "description": "modExpPerHour CalcExpPerHour (the dispatcher that runs and blends the enabled models) called with all 25 arguments.",
    "notes": [
        "Args are positional: nExp (Currency), nRegenTime, nNumMobs, nTotalLairs, nPossSpawns, nRTK, nCharDMG, nCharHP, nCharHPRegen, nMobDmg, nMobHP, nMobHPRegen, nDamageThreshold, nSpellCost, nSpellOverhead, nCharMana, nCharMPRegen, nMeditateRate, nAvgWalk, nWalkSpeed, nSurpriseDMG, nSurpriseMinDMG, nSurpriseChance, nCharFirstRoundDMG, nMinRoundDMG.",
        "Settings are per-case globals: bGlobal_cephModelA..D, bGlobal_cephRecoveryOnly, bGlobal_cephShowAll and nGlobal_ceph{XP,DMG,Mana,Move}_Knob (the oracle resets them to False/0 otherwise).",
        "The MME defaults: frmMain Form_Load starts with A, B, C on and D off; LoadExpPerHourKnobs then reads each model key with default 1, so with no INI keys all four models are on. The first cases pin both. The settings dialog turns A+B+C on when none is ticked, but CalcExpPerHour itself with no model on returns the empty record.",
        "bGreaterMUD stays False (stock path only).",
    ],
    "tolerance": {"default": 0},
    "cases": cases,
    "samples": samples,
}
with open(sys.argv[1], "w", newline="\n") as f:
    json.dump(doc, f, separators=(",", ":"))
    f.write("\n")

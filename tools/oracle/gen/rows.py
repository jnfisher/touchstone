"""Writes fixtures/scenarios/rows.json: python3 -I tools/oracle/gen/rows.py fixtures/scenarios/rows.json
List-row builders (card P3-28): modMain AddWeapon2LV, AddArmour2LV, AddOtherItem2LV, AddSpell2LV, AddRace2LV, AddShop2LV,
AddClass2LV with the colour/sort passes RaceColorCode, SetArmourACDRSortTags and ArmourACDRTagsAreByDR. Each case builds rows
into a recording ListView (a `rows` step: seek a record, call the procedure, keep the row it added). Three characters come from
the pipeline scenario (a warrior, a mage and a mystic: RefreshAll runs once per case, then rows of many records are built with
the profiles PopulateCharacterProfile gave). "No character" cases have the global filter off. Deterministic (splitmix64).
The tables the Rust test needs are written once into the scenario's "data" as field lists."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
PIPE = json.load(open(os.path.join(os.path.dirname(OUT), "pipeline.json")))
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

MASK = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & MASK

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def below(self, n):
        return self.next() % n

    def pick(self, seq, n):
        """n distinct members of seq, in seq's order."""
        seq = list(seq)
        if n >= len(seq):
            return seq
        idx = sorted(self_idx for self_idx in self._distinct(len(seq), n))
        return [seq[i] for i in idx]

    def _distinct(self, size, n):
        chosen = set()
        while len(chosen) < n:
            chosen.add(self.below(size))
        return chosen


rng = Rng(0x28C0DE)

items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
shops = {r["Number"]: dict(r) for r in con.execute("select * from Shops order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}
monsters = {r["Number"]: r["Name"] for r in con.execute("select Number, Name from Monsters order by Number")}
tbinfo = {r["Number"]: r["Action"] for r in con.execute("select Number, Action from TBInfo order by Number")}


def ab(row, n):
    return [[row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n) if row["Abil-%d" % i] != 0]


def named(table):
    """Numbers of the records whose Name is not empty (the others add no row)."""
    return [n for n, r in table.items() if r["Name"]]


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


WEAPONS = [n for n in named(items) if items[n]["ItemType"] == 1]
ARMOURS = [n for n in named(items) if items[n]["ItemType"] == 0]
OTHERS = [n for n in named(items) if items[n]["ItemType"] not in (0, 1)]
SPELLS = named(spells)


def stride(seq, n, offset=0):
    """About n members spread evenly over seq."""
    seq = list(seq)
    if n >= len(seq):
        return seq
    return [seq[(offset + (i * len(seq)) // n) % len(seq)] for i in range(n)]


def with_ability(table, numbers, abil, n):
    return [k for k in numbers if any(table[k]["Abil-%d" % x] == abil for x in range(20))][:n]


# ---------------------------------------------------------------------------------- scenario pieces
SETUP_FORMS = json.loads(json.dumps(PIPE["forms"]))
SETUP_FORMS["frmMain"].update({
    "chkWeaponOptions": arr({0: {"Value": 5}, 3: {"Value": 0}, 4: {"Value": 0}}),
    "chkSpellOptions": arr({0: {"Value": 0}, 1: {"Value": 5}, 2: {"Value": 0}}),
    "txtWeaponExtras": arr({2: {"Text": "0"}, 3: {"Text": "0"}, 4: {"Text": "0"}}),
    "txtSpellOptions": arr({0: {"Text": "0"}}),
    "cmbWeaponCombos": arr({1: {"ListIndex": 0, "ItemData": [5]}}),
    "cmbCharBless": arr({k: {} for k in range(10)}),
})


def prof():
    return {"udt": "tCharacterProfile", "fields": {}, "out": True}


def populate(name, use_char, mode=0, weapon=0, no_char=False):
    return {"call": "PopulateCharacterProfile", "args": [prof(), use_char, True, mode, weapon, no_char], "as": name}


def keyed(keys, call, args, as_, **extra):
    st = {"rows": dict({"table": extra.pop("table"), "index": extra.pop("index"), "keys": list(keys)}, **extra),
          "call": call, "args": args, "as": as_}
    return st


def weapon_rows(as_, keys, profile, ability=0, mode=0, **extra):
    """AddWeapon2LV(lv, tChar, AddToInven:=False, nAbility, nAttackTypeMUD, , bForceCalc)."""
    force = extra.pop("force", False)
    return keyed(keys, "AddWeapon2LV", [{"lv": True}, {"saved": profile}, False, ability, mode, {"t": "missing"}, force], as_,
                 table="tabItems", index="pkItems", **extra)


def spell_rows(as_, keys, profile, bless=False, **extra):
    return keyed(keys, "AddSpell2LV", [{"lv": True}, {"saved": profile}, bless], as_, table="tabSpells", index="pkSpells", **extra)


def item_rows(as_, keys, proc, args=(), **extra):
    return keyed(keys, proc, [{"lv": True}] + list(args), as_, table="tabItems", index="pkItems", **extra)


cases = []


def case(cid, steps, forms=None, globals_=None):
    c = {"id": cid, "steps": steps}
    if forms:
        c["forms"] = {"frmMain": forms}
    if globals_:
        c["globals"] = globals_
    cases.append(c)


# ---------------------------------------------------------------------------------- A. no character
NOCHAR = {"chkGlobalFilter": {"Value": 0}}


def nochar_case(cid, steps, **controls):
    f = dict(NOCHAR)
    f.update(controls)
    case(cid, [populate("p5", False, 5), populate("p4", False, 4)] + steps, forms=f)


nochar_case("nochar-armour-all", [
    item_rows("rows", ARMOURS, "AddArmour2LV", [False, 0])])
case("nochar-sundry-all", [item_rows("rows", OTHERS, "AddOtherItem2LV")])
case("nochar-shops-all", [keyed(named(shops), "AddShop2LV", [{"lv": True}], "rows", table="tabShops", index="pkShops")])
case("nochar-classes-all", [keyed(named(classes), "AddClass2LV", [{"lv": True}], "rows", table="tabClasses", index="pkClasses")])
case("nochar-races-all", [keyed(named(races), "AddRace2LV", [{"lv": True}], "rows", table="tabRaces", index="pkRaces")])

# ---------------------------------------------------------------------------------- B. weapons without a character
def combat(speed=0, vs=(40, 5, 20), mode=5, level_box=None):
    f = {"chkWeaponOptions": arr({0: {"Value": 5}, 3: {"Value": 1}, 4: {"Value": speed}}),
         "txtWeaponExtras": arr({2: {"Text": str(vs[0])}, 3: {"Text": str(vs[1])}, 4: {"Text": str(vs[2])}}),
         "cmbWeaponCombos": arr({1: {"ListIndex": 0, "ItemData": [mode]}})}
    return f


BS_WEAPONS = with_ability(items, WEAPONS, 116, 400)
ABIL_WEAPONS = {a: with_ability(items, WEAPONS, a, 400) for a in (43, 22, 58, 135, 116, 105)}


def nochar_weapon_cases():
    plain = dict(NOCHAR)
    case("nochar-weapons-plain", [populate("p5", False, 5), weapon_rows("rows", WEAPONS, "p5")], forms=plain)
    f = dict(NOCHAR)
    f.update(combat(1))
    case("nochar-weapons-combat", [populate("p5", False, 5), weapon_rows("rows", stride(WEAPONS, 70), "p5", 0, 5)], forms=f)
    # mode 0 reads the mode from cmbWeaponCombos(1); backstab needs the stealth profile
    f4 = dict(NOCHAR)
    f4.update(combat(0, (10, 3, 10), 4))
    case("nochar-weapons-backstab", [populate("p4", False, 4), weapon_rows("rows", stride(BS_WEAPONS, 40) + stride(WEAPONS, 12, 3), "p4", 0, 0)],
         forms=f4)
    steps = [populate("p%d" % m, False, m) for m in range(1, 8)]
    for m in range(1, 8):
        steps.append(weapon_rows("lab%d" % m, stride(WEAPONS, 5, m), "p%d" % m, -1, m))
    case("nochar-weapons-labels", steps, forms=dict(NOCHAR, **combat(0, (0, 0, 0), 5)))
    steps = [populate("p5", False, 5)]
    for a, keys in ABIL_WEAPONS.items():
        # ability 43 shows the cast spell: FilterWeapons only offers it for weapons that have the ability, and a spell
        # number of 0 would mean "the spell under the table cursor", which the rows cannot depend on
        steps.append(weapon_rows("abil%d" % a, keys[:14] + ([] if a == 43 else stride(WEAPONS, 4, a)), "p5", a, 5))
    case("nochar-weapons-abilities", steps, forms=dict(NOCHAR))
    # surprise punch: no record read for the name, but the cursor must be on a record (the IIf arms read the table)
    f = dict(NOCHAR)
    f.update(combat(0, (0, 0, 0), 4))
    case("nochar-weapons-surprise-punch", [populate("p4", False, 4), weapon_rows("rows", [WEAPONS[0]], "p4", 0, -4)], forms=f)


nochar_weapon_cases()

# ---------------------------------------------------------------------------------- C. armour passes, races
ARM_SAMPLE = stride(ARMOURS, 60)
case("nochar-armour-abilities", [item_rows("a%d" % a, with_ability(items, ARMOURS, a, 12), "AddArmour2LV", [False, a])
                                 for a in (22, 58, 135, 105, 106, 4, 2)])
case("nochar-armour-sort-tags", [
    dict(item_rows("rows", ARM_SAMPLE, "AddArmour2LV", [False, 0], lv="shared"),
         post=[{"call": "SetArmourACDRSortTags", "args": [{"lv": True}, False]},
               {"call": "ArmourACDRTagsAreByDR", "args": [{"lv": True}]},
               {"call": "SetArmourACDRSortTags", "args": [{"lv": True}, True]},
               {"call": "ArmourACDRTagsAreByDR", "args": [{"lv": True}]}])])
case("nochar-races-color", [
    dict(keyed(named(races), "AddRace2LV", [{"lv": True}], "all", table="tabRaces", index="pkRaces", lv="shared"),
         post=[{"call": "RaceColorCode", "args": [{"lv": True}]}]),
    dict(keyed(named(races)[:5], "AddRace2LV", [{"lv": True}], "five", table="tabRaces", index="pkRaces", lv="shared"),
         post=[{"call": "RaceColorCode", "args": [{"lv": True}]}]),
    dict(keyed(named(races)[7:8], "AddRace2LV", [{"lv": True}], "one", table="tabRaces", index="pkRaces", lv="shared"),
         post=[{"call": "RaceColorCode", "args": [{"lv": True}]}])])

# ---------------------------------------------------------------------------------- D. spells without a character
SPELL_STRIDE = stride(SPELLS, 120)
case("nochar-spells", [populate("ps", False), spell_rows("rows", SPELL_STRIDE, "ps")], forms=dict(NOCHAR))
case("nochar-spells-bless", [populate("ps", False), spell_rows("rows", stride(SPELLS, 150, 7), "ps", True, watch="cmbCharBless(0)")],
     forms=dict(NOCHAR))

# ---------------------------------------------------------------------------------- D2. edges of the form values
EDGE_KEYS = stride(WEAPONS, 4, 9)
for tag, extras in (("overflow", ("3000000000", "0", "0")), ("round", ("2.5", "3.5", "-0.5")), ("negative", ("-40", "-3", "-7"))):
    f = dict(NOCHAR)
    f.update(combat(0, extras, 5))
    case("edge-weapon-extras-" + tag, [populate("p5", False, 5), weapon_rows("rows", EDGE_KEYS, "p5", 0, 0)], forms=f)
EDGE_SPELLS = stride(SPELLS, 24, 5)
for tag, box in (("half", "27.5"), ("tiny", "0.4"), ("overflow", "40000"), ("zero", "0"), ("neg", "-3")):
    case("edge-spell-level-" + tag, [populate("ps", False, 0, 0, True), spell_rows("rows", EDGE_SPELLS, "ps")], forms={
        "chkGlobalFilter": {"Value": 1}, "txtGlobalLevel": arr({0: {"Text": "50"}, 1: {"Text": box}}),
        "chkSpellOptions": arr({0: {"Value": 1}, 1: {"Value": 5}, 2: {"Value": 1}}), "txtSpellOptions": arr({0: {"Text": "999999999999"}})})

# ---------------------------------------------------------------------------------- D3. a lair party: no loaded character, level 0
PARTY = {"chkGlobalFilter": {"Value": 1}, "optMonsterFilter": arr({1: {"Value": True}}),
         "txtMonsterLairFilter": arr({k: {"Text": {0: "3", 5: "1200", 7: "40", 8: "250"}.get(k, "0")} for k in range(9)}),
         "txtMonsterDamage": {"Text": "35"}}


def party_populate(name, mode=0):
    return {"call": "PopulateCharacterProfile", "args": [prof(), False, False, mode, 0, False], "as": name}


for tag, calc, box in (("plain", 0, "27"), ("combat", 1, "27.5"), ("nolevel", 0, "0"), ("overflow", 1, "40000")):
    case("party-spells-" + tag, [party_populate("ps"), spell_rows("rows", stride(SPELLS, 40, 3), "ps")], forms=dict(
        PARTY, txtGlobalLevel=arr({0: {"Text": "50"}, 1: {"Text": box}}),
        chkSpellOptions=arr({0: {"Value": calc}, 1: {"Value": 5}, 2: {"Value": calc}}), txtSpellOptions=arr({0: {"Text": "30"}})))
f = dict(PARTY)
f.update(combat(0, (10, 3, 10), 5))
case("party-weapons", [party_populate("p5", 5), party_populate("p4", 4), weapon_rows("rows", stride(WEAPONS, 30, 2), "p5", 0, 5),
                       weapon_rows("bs", stride(BS_WEAPONS, 10), "p4", 0, 4)], forms=f)

# ---------------------------------------------------------------------------------- E. three characters
CHARS = [("warrior", "ladder-c1-l50"), ("mage", "ladder-c12-l50"), ("mystic", "ladder-c15-l50")]
PC = {c["id"]: c for c in PIPE["cases"]}


def char_forms(base, **more):
    f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
    f["txtGlobalLevel"]["$array"]["1"] = {"Text": f["txtGlobalLevel"]["$array"]["0"]["Text"]}
    f.update(more)
    return f


def char_case(who, base, suffix, steps, **more):
    cid = "%s-%s" % (who, suffix)
    f = char_forms(base, **more)
    case(cid, [{"call": "RefreshAll"}] + steps, forms=f, globals_=PC[base]["globals"])


for who, base in CHARS:
    eq = PC[base]["globals"]["nEquippedItem"]
    mine = [n for n in (eq[16], eq[12]) if n in items and items[n]["ItemType"] == 1]
    sample = sorted(set(stride(WEAPONS, 90) + mine))
    char_case(who, base, "weapons-plain", [populate("p5", True, 5), weapon_rows("rows", sample, "p5")])
    char_case(who, base, "weapons-combat", [populate("p5", True, 5), weapon_rows("rows", stride(WEAPONS, 60, 1) + mine, "p5", 0, 0)],
              **combat(1, (30, 6, 15), 5))
    bs = stride(BS_WEAPONS, 30) + mine
    char_case(who, base, "weapons-backstab", [populate("p4", True, 4), weapon_rows("rows", bs, "p4", 0, 4)], **combat(0, (10, 3, 10), 4))
    steps = [populate("p%d" % m, True, m) for m in range(1, 8)]
    for m in range(1, 8):
        steps.append(weapon_rows("lab%d" % m, stride(WEAPONS, 6, m) + mine, "p%d" % m, -1, m))
    char_case(who, base, "weapons-labels", steps, **combat(0, (20, 4, 10), 5))
    steps = [populate("p5", True, 5)]
    for a in (43, 22, 58, 135, 116):
        steps.append(weapon_rows("abil%d" % a, ABIL_WEAPONS[a][:10], "p5", a, 5))
    char_case(who, base, "weapons-abilities", steps, **combat(0, (20, 4, 10), 5))
    char_case(who, base, "spells", [populate("ps", False), spell_rows("rows", stride(SPELLS, 160, 3), "ps")])
    char_case(who, base, "spells-combat", [populate("ps", False), spell_rows("rows", stride(SPELLS, 90, 11), "ps")],
              chkSpellOptions=arr({0: {"Value": 1}, 1: {"Value": 5}, 2: {"Value": 1}}), txtSpellOptions=arr({0: {"Text": "35"}}))
    char_case(who, base, "spells-combat2", [populate("ps", False), spell_rows("rows", stride(SPELLS, 60, 5), "ps")],
              chkSpellOptions=arr({0: {"Value": 1}, 1: {"Value": 5}, 2: {"Value": 0}}), txtSpellOptions=arr({0: {"Text": "0"}}))
    # global filter on but no loaded character (forced off): the level comes from txtGlobalLevel(1)
    char_case(who, base, "spells-filter-level", [populate("ps", False, 0, 0, True), spell_rows("rows", stride(SPELLS, 60, 2), "ps")],
              chkSpellOptions=arr({0: {"Value": 1}, 1: {"Value": 5}, 2: {"Value": 0}}), txtSpellOptions=arr({0: {"Text": "20"}}),
              txtGlobalLevel=arr({0: {"Text": "50"}, 1: {"Text": "27"}}))

# ---------------------------------------------------------------------------------- data for the Rust test
EQUIPPED = {n for _, base in CHARS for n in PC[base]["globals"]["nEquippedItem"] if n in items}
FULL_ITEMS = sorted(set(WEAPONS) | set(ARMOURS) | EQUIPPED)
ITEM_FIELDS = ["Number", "Name", "Limit", "Encum", "ItemType", "Min", "Max", "ArmourClass", "DamageResist", "WeaponType",
               "ArmourType", "Worn", "Accy", "StrReq", "Speed"]


def item_row(n):
    r = items[n]
    return [r[k] for k in ITEM_FIELDS] + [[r["ClassRest-%d" % k] for k in range(10) if r["ClassRest-%d" % k]], ab(r, 20)]


SPELL_FIELDS = ["Number", "Name", "Short", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists",
                "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc",
                "DurIncLVLs", "DurInc", "Learnable"]


def spell_row(n):
    r = spells[n]
    return ([r["Number"], r["Name"] or "", r["Short"] or ""] + [r[k] for k in SPELL_FIELDS[3:]]
            + [1 if r["Learned From"] else 0, ab(r, 10)])


CLASS_FIELDS = ["Number", "Name", "MinHits", "MaxHits", "ExpTable", "MageryType", "MageryLVL", "WeaponType", "ArmourType", "CombatLVL"]
RACE_FIELDS = ["Number", "Name", "mSTR", "mINT", "mWIL", "mAGL", "mHEA", "mCHM", "xSTR", "xINT", "xWIL", "xAGL", "xHEA", "xCHM",
               "HPPerLVL", "ExpTable", "BaseCP"]
# Textblocks a spell ability (148) can reach; GetAbilityStats prints parts of their Action text
TB_NUMBERS = sorted({r["AbilVal-%d" % k] for r in spells.values() for k in range(10) if r["Abil-%d" % k] == 148 and r["AbilVal-%d" % k] in tbinfo})

DATA = {
    "item_fields": ITEM_FIELDS + ["ClassRest(nonzero)", "Abilities(ability,value)"],
    "items": [item_row(n) for n in FULL_ITEMS],
    "sundry_fields": ["Number", "Name", "Limit", "Encum", "ItemType"],
    "sundry": [[n, items[n]["Name"], items[n]["Limit"], items[n]["Encum"], items[n]["ItemType"]] for n in OTHERS if n not in EQUIPPED],
    "spell_fields": SPELL_FIELDS + ["LearnedFrom(1 if not empty)", "Abilities(ability,value)"],
    "spells": [spell_row(n) for n in sorted(spells)],
    "class_fields": CLASS_FIELDS + ["Abilities(ability,value)"],
    "classes": [[classes[n][k] for k in CLASS_FIELDS] + [ab(classes[n], 10)] for n in sorted(classes)],
    "race_fields": RACE_FIELDS + ["Abilities(ability,value)"],
    "races": [[races[n][k] for k in RACE_FIELDS] + [ab(races[n], 10)] for n in sorted(races)],
    "shops": [[n, shops[n]["Name"], shops[n]["ShopType"]] for n in sorted(shops)],
    "monster_names": [[n, monsters[n] or ""] for n in sorted(monsters)],
    "textblocks": [[n, tbinfo[n] or ""] for n in TB_NUMBERS],
}

scn = {
    "family": "rows",
    "description": "List-row builders (FAM-21 without monsters): AddWeapon2LV, AddArmour2LV, AddOtherItem2LV, AddSpell2LV, "
                   "AddRace2LV, AddShop2LV, AddClass2LV and the passes RaceColorCode, SetArmourACDRSortTags, ArmourACDRTagsAreByDR",
    "notes": [
        "Each case builds rows with a `rows` step (see tools/oracle/README.md): the record is seeked in the table's global "
        "recordset, the Add*2LV procedure runs against a recording ListView and the row it added (or null) is stored. "
        "Row = [Text, Tag, cells]; a cell is its Text, or [Text, Tag, ForeColor] when one was assigned.",
        "Characters: cases warrior/mage/mystic reuse the pipeline scenario's ladder-c1-l50, ladder-c12-l50 and ladder-c15-l50 "
        "(RefreshAll runs once, then PopulateCharacterProfile per attack kind, outputs 'p<kind>.refs'); the weapon profile is "
        "PopulateCharacterProfile(force_use_char, force_no_party, kind) as FilterWeapons calls it, the spell profile the one "
        "FilterSpells calls (no kind). 'nochar' cases have chkGlobalFilter off; 'party' cases a lair party of 3 with the filter on "
        "(a profile of level 0 that is not a loaded character).",
        "Form values the procedures read: chkWeaponOptions(3) calculate combat, (4) 85% speed, txtWeaponExtras(2..4) versus AC/DR/dodge, "
        "cmbWeaponCombos(1) attack kind, chkSpellOptions(0) calculate, (2) anti-magic, txtSpellOptions(0) versus MR, txtGlobalLevel(1).",
        "nAbility 43 is only run on weapons that have the ability: a spell number of 0 means 'the spell under the table cursor' "
        "in PullSpellEQ, which the rows would then depend on. Armour never has abilities 22, 105 or 106 in the stock data.",
        "edge-* cases: txtWeaponExtras that overflow CLng (the row keeps only its Text and Tag), round and negative extras, and "
        "spell level boxes that round, are zero or overflow Integer (no row).",
        "data holds the tables the Rust test rebuilds the world from: full item rows for weapons, armour and the gear the three "
        "characters wear, minimal rows for the other items, all spells (name, short and every column the rows and spell text read), "
        "classes, races, shops, monster names and the textblocks a spell ability can cast.",
    ],
    "database": True,
    "frm": PIPE["frm"],
    "data": DATA,
    "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0},
    "forms": SETUP_FORMS,
    "read": {"accuracy_tag": "%c.lblInvenCharStat(10).Tag"},
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

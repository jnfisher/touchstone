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

scn = {
    "family": "rows",
    "description": "List-row builders (FAM-21 without monsters): AddWeapon2LV, AddArmour2LV, AddOtherItem2LV, AddSpell2LV, "
                   "AddRace2LV, AddShop2LV, AddClass2LV and the passes RaceColorCode, SetArmourACDRSortTags, ArmourACDRTagsAreByDR",
    "notes": [],
    "database": True,
    "frm": PIPE["frm"],
    "data": {},
    "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0},
    "forms": SETUP_FORMS,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

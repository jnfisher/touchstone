"""Writes fixtures/scenarios/class_race_builder.json: python3 -I tools/oracle/gen/class_race_builder.py fixtures/scenarios/class_race_builder.json
The Classes/Races builder (card P4-12): frmMain RefreshClassRaceBuilder and the two action branches of
cmdClassRace_Click (Equip, Spellbook). Cases use real stock rows only (the fixture's "data" repeats the
rows the cases use, so the Rust test does not need the database)."""
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}


def ab(row):
    return [[row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(10)]


FRM = {
    "file": "frmMain.frm",
    "decls": [],
    "procs": ["RefreshClassRaceBuilder"],
    # the two lists' selected rows are form state the oracle has no ListView for: the selected record
    # number of each list is read from the Tag of the matching detail box instead (empty = nothing selected)
    "subs": {"RefreshClassRaceBuilder": {
        "If lvClasses.ListItems.count > 0 Then": "If True Then",
        "If Not lvClasses.SelectedItem Is Nothing Then nClass = val(lvClasses.SelectedItem.Text)":
            "nClass = val(frmMain.txtClassRaceDetail(0).Tag)",
        "If lvRaces.ListItems.count > 0 Then": "If True Then",
        "If Not lvRaces.SelectedItem Is Nothing Then nRace = val(lvRaces.SelectedItem.Text)":
            "nRace = val(frmMain.txtClassRaceDetail(1).Tag)",
    }},
}

used_classes, used_races = set(), set()
cases = []


def case(cid, cls, race, avg="stock", text_before=""):
    """`avg`: "stock" runs ComputeAvgLevelMaxStats first (what the load does), a number sets the global."""
    if cls in classes:
        used_classes.add(cls)
    if race in races:
        used_races.add(race)
    steps = []
    c = {"id": cid, "forms": {"frmMain": {"txtClassRaceDetail": {"$array": {
        "0": {"Tag": str(cls) if cls else ""}, "1": {"Tag": str(race) if race else ""}, "2": {"Text": text_before}}}}}}
    if avg == "stock":
        steps.append({"call": "ComputeAvgLevelMaxStats"})
    else:
        c["globals"] = {"gAvgLevelMaxSingleStat": avg}
    steps.append({"call": "RefreshClassRaceBuilder"})
    c["steps"] = steps
    c["read"] = {"text": "%c.txtClassRaceDetail(2).Text", "avg": "%g.gAvgLevelMaxSingleStat"}
    cases.append(c)


# every stock class with a rotating race, so all fifteen classes and all thirteen races appear
for k, cl in enumerate(sorted(classes)):
    case("class-%02d" % cl, cl, 1 + (k * 4) % 13)
# every race with a fighter, a hybrid caster or a pure caster, in turn
for r in sorted(races):
    cl = (1, 9, 12)[r % 3]
    case("race-%02d-class-%02d" % (r, cl), cl, r)
# nothing, one side only, numbers that name no row
case("none", 0, 0)
case("class-only", 1, 0)
case("race-only", 0, 1)
case("bad-class", 99, 1)
case("bad-race", 1, 99)
case("bad-both", 99, 99)
case("negative-class", -4, 2)
# the old text stays when the handler runs, and when nothing is selected the box is written empty
case("keeps-text-none", 0, 0, text_before="stale")
# gAvgLevelMaxSingleStat edge values: unset, under 50, 50 to 100, over 100 (halved), very large
for avg in (0, 20.5, 49.5, 75.5, 100, 100.5, 133, 255.5, 410, 2000):
    case("avg-%s" % str(avg).replace(".", "p"), 12, 4, avg)
case("avg-0-warrior", 1, 10, 0)
case("avg-133-warrior", 1, 10, 133)
# an Integer overflow in the level steps ends the procedure in its handler: the old text stays
case("avg-40000-overflow", 12, 4, 40000, text_before="old text")
case("avg-6000", 12, 4, 6000)
# the Spellbook button reads the class's magery kind and level
for cl in sorted(classes) + [99]:
    cases.append({"id": "spellbook-%02d" % cl, "steps": [
        {"call": "GetClassMagery", "args": [cl], "as": "magery"},
        {"call": "GetClassMageryLVL", "args": [cl], "as": "level"}]})

data = {
    "classes": [{"n": n, "name": classes[n]["Name"], "min": classes[n]["MinHits"], "max": classes[n]["MaxHits"],
                 "exp": classes[n]["ExpTable"], "combat": classes[n]["CombatLVL"], "mt": classes[n]["MageryType"],
                 "ml": classes[n]["MageryLVL"], "wt": classes[n]["WeaponType"], "at": classes[n]["ArmourType"],
                 "ab": ab(classes[n])} for n in sorted(used_classes)],
    "races": [{"n": n, "name": races[n]["Name"], "mhea": races[n]["mHEA"], "xhea": races[n]["xHEA"],
               "magl": races[n]["mAGL"], "xagl": races[n]["xAGL"], "hp": races[n]["HPPerLVL"],
               "exp": races[n]["ExpTable"], "cp": races[n]["BaseCP"], "ab": ab(races[n])}
              for n in sorted(used_races)],
}

scn = {
    "family": "class_race_builder",
    "description": "Class/Race builder text (frmMain RefreshClassRaceBuilder)",
    "notes": [
        "RefreshClassRaceBuilder is cut out of frmMain.frm (scenario key frm). The selected rows of lvClasses and "
        "lvRaces are replaced (scenario subs) by reads of the Tag of txtClassRaceDetail(0) and (1); an empty Tag is "
        "no selection. The result is txtClassRaceDetail(2).Text.",
        "avg 'stock' cases run ComputeAvgLevelMaxStats first, as the load does; the others set gAvgLevelMaxSingleStat.",
        "data holds the real rows the cases use, for the Rust test.",
    ],
    "database": True,
    "frm": FRM,
    "data": data,
    "forms": {"frmMain": {"lblDatVer": {"Caption": "v1.11p"}, "txtClassRaceDetail": {"$array": {"0": {}, "1": {}, "2": {}}}}},
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d classes, %d races" % (len(cases), len(data["classes"]), len(data["races"])), file=sys.stderr)

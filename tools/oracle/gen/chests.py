"""Writes fixtures/scenarios/chests.json: python3 -I tools/oracle/gen/chests.py fixtures/scenarios/chests.json
The Sundry explorer's chest button (card P4-32): frmMain cmdSundryChests_Click on lvOtherItems with one item selected,
and the rows it leaves in lvOtherItemLoc (percent text, tag, "Item: name" and the item number). GetChestItems and the
rest of the chest arithmetic are MME's own (modMMudDatabase). Every real chest, some items that are not chests, and
invented chests (numbered 9301+) that reach each failure and branch. Deterministic."""
import glob
import json
import os
import re
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row
items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
tbs = {r["Number"]: dict(r) for r in con.execute("select * from TBInfo order by Number")}


def item(n, name, itype=8, abil=()):
    r = {"Number": n, "Name": name, "ItemType": itype}
    for i, (a, v) in enumerate(abil):
        r["Abil-%d" % i] = a
        r["AbilVal-%d" % i] = v
    return r


def spell(n, name, min_base=0, max_base=0, abil=()):
    r = {"Number": n, "Name": name, "MinBase": min_base, "MaxBase": max_base}
    for i, (a, v) in enumerate(abil):
        r["Abil-%d" % i] = a
        r["AbilVal-%d" % i] = v
    return r


REAL = sorted(n for n, r in items.items() if r["ItemType"] == 8)
GIVE = sorted(n for n, r in items.items() if r["ItemType"] != 8)[:6]
G1, G2, G3 = GIVE[0], GIVE[1], GIVE[2]
EXTRA_TB = [
    # leaves: a chest's own table (cumulative chances, then what is given)
    {"Number": 79501, "LinkTo": 0, "Action": "50:giveitem %d\n100:giveitem %d\n" % (G1, G2)},
    {"Number": 79502, "LinkTo": 0, "Action": "10:giveitem %d\n20:giveitem %d\n100:giveitem %d\n" % (G1, G1, G3)},
    {"Number": 79504, "LinkTo": 0, "Action": "\u0000"},
    {"Number": 79505, "LinkTo": 0, "Action": "100:message 5\n"},
    {"Number": 79507, "LinkTo": 0, "Action": "50:random 79507\n100:giveitem %d\n" % G1},         # points at itself
    {"Number": 79509, "LinkTo": 0, "Action": "33:giveitem %d\n66:giveitem %d\n100:giveitem %d\n" % (G1, G2, G3)},
    {"Number": 79510, "LinkTo": 0, "Action": "100:giveitem 99999\n"},
    {"Number": 79511, "LinkTo": 0, "Action": "50:random 79501\n100:giveitem %d\n" % G3},
    # what the chest spell points at: the "random" lines name the leaves
    {"Number": 79601, "LinkTo": 0, "Action": "message 1:random 79501\n"},
    {"Number": 79602, "LinkTo": 0, "Action": "random 79502\n"},
    {"Number": 79603, "LinkTo": 0, "Action": "random 79501:random 79501\n"},
    {"Number": 79606, "LinkTo": 0, "Action": "RANDOM 79501:Random 79502\n"},
    {"Number": 79607, "LinkTo": 0, "Action": "random 79507\n"},
    {"Number": 79608, "LinkTo": 0, "Action": "random x\nrandom \nrandom 99999999999\nrandom 79501"},
    {"Number": 79609, "LinkTo": 0, "Action": "random 79509\n"},
    {"Number": 79610, "LinkTo": 0, "Action": "random 79510\n"},
    {"Number": 79611, "LinkTo": 0, "Action": "random 79511:random 79502\n"},
]
EXTRA_SPELLS = [
    spell(9401, "tst chest spell", abil=((148, 79601),)),
    spell(9402, "tst chest min base", min_base=79602, abil=((148, 0),)),
    spell(9403, "tst chest max base", max_base=79601, abil=((148, 0),)),
    spell(9404, "tst chest nul", abil=((148, 79504),)),
    spell(9405, "tst chest nul then good", abil=((148, 79504), (148, 79601))),
    spell(9406, "tst chest missing tb", abil=((148, 99999),)),
    spell(9407, "tst chest no 148", abil=((1, 5),)),
    spell(9408, "tst chest random", abil=((148, 79603),)),
    spell(9409, "tst chest random mixed", abil=((148, 79606),)),
    spell(9410, "tst chest self", abil=((148, 79607),)),
    spell(9411, "tst chest junk", abil=((148, 79608),)),
    spell(9412, "tst chest thirds", abil=((148, 79609),)),
    spell(9413, "tst chest ghost item", abil=((148, 79610),)),
    spell(9414, "tst chest no data", abil=((148, 79505),)),
    spell(9415, "tst chest nested", abil=((148, 79611),)),
]
EXTRA_ITEMS = [
    item(9301, "tst chest", abil=((43, 9401),)),
    item(9302, "tst chest minbase", abil=((43, 9402),)),
    item(9303, "tst chest maxbase", abil=((43, 9403),)),
    item(9304, "tst chest nul", abil=((43, 9404),)),
    item(9305, "tst chest nul good", abil=((43, 9405),)),
    item(9306, "tst chest missing tb", abil=((43, 9406),)),
    item(9307, "tst chest no castsp", abil=((43, 9407),)),
    item(9308, "tst chest nested", abil=((43, 9408),)),
    item(9309, "tst chest nested mixed", abil=((43, 9409),)),
    item(9310, "tst chest self", abil=((43, 9410),)),
    item(9311, "tst chest junk", abil=((43, 9411),)),
    item(9312, "tst chest thirds", abil=((43, 9412),)),
    item(9313, "tst chest ghost item", abil=((43, 9413),)),
    item(9314, "tst chest no ability"),
    item(9315, "tst chest zero spell", abil=((43, 0),)),
    item(9316, "tst chest missing spell", abil=((43, 99999),)),
    item(9317, "tst chest second ability", abil=((5, 1), (43, 9407), (43, 9401))),
    item(9318, "tst chest non-chest data", abil=((43, 9401),)),
    item(9319, "tst chest no action", abil=((43, 9414),)),
    item(9320, "tst chest nested leaves", abil=((43, 9415),)),
]
EXTRA_ITEMS[17]["ItemType"] = 0      # looks like a chest, is not a container
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)}, **r}
base_spell = {**{"Abil-%d" % i: 0 for i in range(10)}, **{"AbilVal-%d" % i: 0 for i in range(10)}, "MinBase": 0, "MaxBase": 0}
for r in EXTRA_SPELLS:
    spells[r["Number"]] = {**base_spell, **r}
for r in EXTRA_TB:
    tbs[r["Number"]] = r

cases = []
LVO = lambda n: {"$cliplv": {"name": "lvOtherItems", "columns": [["#", 600], ["Name", 2000]],
                              "rows": [{"Text": str(n), "Sub": [items.get(n, {}).get("Name", "")], "Sel": True}]}}


def add(cid, n, hide=False, sel=True):
    f = LVO(n)
    if not sel:
        f["$cliplv"]["rows"][0]["Sel"] = False
    add_case = {"id": cid, "forms": {"frmMain": {"lvOtherItems": f, "lvOtherItemLoc": {"$recordlv": []}}},
                "steps": [{"call": "cmdSundryChests_Click", "capture": {"rows": "%lv.lvOtherItemLoc"}}]}
    if hide:
        add_case["globals"] = {"bHideRecordNumbers": True}
    cases.append(add_case)


for n in REAL:
    add("chest-%d" % n, n, n % 4 == 0)
for n in range(9301, 9321):
    add("chest-%d" % n, n, n % 5 == 0)
for n in (G1, G2, 906 + 0 * G3):
    add("notchest-%d" % n, n)
add("duplicate-chest-hidden", 907, True)
add("missing-item", 99999)
add("none-selected", 906, sel=False)

# ------------------------------------------------------------------------------- data (the closure of the cases)
want_tb, want_spells = set(), set()


def walk(tb):
    if tb in want_tb or tb not in tbs:
        return
    want_tb.add(tb)
    for m in re.finditer(r"(?i)random (\d+)", tbs[tb]["Action"] or ""):
        if int(m.group(1)) < 2 ** 31:
            walk(int(m.group(1)))


for c in cases:
    n = int(c["forms"]["frmMain"]["lvOtherItems"]["$cliplv"]["rows"][0]["Text"])
    it = items.get(n)
    if not it or it["ItemType"] != 8:
        continue
    for i in range(20):
        if it["Abil-%d" % i] == 43 and it["AbilVal-%d" % i] > 0:
            s = spells.get(it["AbilVal-%d" % i])
            if s:
                want_spells.add(s["Number"])
                for j in range(10):
                    if s["Abil-%d" % j] == 148:
                        walk(s["AbilVal-%d" % j] or s["MinBase"] or s["MaxBase"])
data = {
    "items": [[n, r["Name"] or "", r["ItemType"], [r["AbilVal-%d" % i] if r["Abil-%d" % i] == 43 else 0 for i in range(20)]]
              for n, r in sorted(items.items())],
    "spells": [[n, spells[n]["MinBase"], spells[n]["MaxBase"],
                [x for j in range(10) for x in (spells[n]["Abil-%d" % j], spells[n]["AbilVal-%d" % j]) if spells[n]["Abil-%d" % j]]]
               for n in sorted(want_spells)],
    "tb": [[n, tbs[n]["Action"]] for n in sorted(want_tb)],
}

scn = {
    "family": "chests",
    "description": "Sundry explorer chest contents (frmMain cmdSundryChests_Click with GetChestItems)",
    "notes": [
        "The selected row stands in for lvOtherItems.SelectedItem; the rows read are lvOtherItemLoc's: percent text, tag "
        "(the rounded chance), a sub-item 'Item: name' tagged with the item number.",
        "SortListViewByTag is recorded as a sort_listview_by_tag event, not run: the port returns the rows in insertion "
        "order and sorts by chance, highest first.",
        "data.items is [number, name, type, the AbilVal of each of the 20 slots holding ability 43 (else 0)]; data.spells "
        "[number, MinBase, MaxBase, [ability, value ...]]; data.tb [number, action].",
        "Rows numbered 9301 and up (items), 9401 (spells) and 79501 (textblocks) are invented (db_extra).",
    ],
    "database": True,
    "frm": {"file": "frmMain.frm", "me_is_form": True, "procs": ["cmdSundryChests_Click"]},
    "db_extra": {"Items": EXTRA_ITEMS, "Spells": EXTRA_SPELLS, "TBInfo": EXTRA_TB},
    "data": data,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

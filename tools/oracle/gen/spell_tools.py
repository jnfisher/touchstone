"""Writes fixtures/scenarios/spell_tools.json: python3 -I tools/oracle/gen/spell_tools.py fixtures/scenarios/spell_tools.json
The Spells explorer's lookups (card P4-32): frmMain FindNegates ("Find counters"), the code the spells list menu and
the monster pane menu run to build FindNegates' arguments, and LookUpSpellCast ("What casts this spell").
Deterministic. Real Spells and Items rows from the stock SQLite plus invented rows numbered 9001+; every row a case can
reach is written once into the scenario's "data" so the Rust test needs no database."""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
mons = {r["Number"]: r["Name"] or "" for r in con.execute("select Number, Name from Monsters")}

# ------------------------------------------------------------------------------ invented rows
SPELL_A, SPELL_B, ABIL_X, ABIL_Y = 57, 106, 40, 41    # real spell numbers; ability numbers some real spells carry


def item(n, name, in_game=1, neg=(), abil=()):
    r = {"Number": n, "Name": name, "In Game": in_game, "ItemType": 0}
    for i, v in enumerate(neg):
        r["NegateSpell-%d" % i] = v
    for i, (a, v) in enumerate(abil):
        r["Abil-%d" % i] = a
        r["AbilVal-%d" % i] = v
    return r


def spell(n, name, learnable=0, cast="", abil=()):
    r = {"Number": n, "Name": name, "Learnable": learnable, "Casted By": cast}
    for i, (a, v) in enumerate(abil):
        r["Abil-%d" % i] = a
        r["AbilVal-%d" % i] = v
    return r


EXTRA_ITEMS = [
    item(9001, "tst negating amulet", neg=(0, SPELL_A)),
    item(9002, "tst negating ring late", neg=(1, 2, 3, 4, 5, 6, 7, 8, SPELL_B)),
    item(9003, "tst dispel ring", abil=((73, ABIL_X),)),
    item(9004, "tst negate-ability ring", abil=((5, 3), (124, ABIL_X))),
    item(9005, "tst remove-spell ring", abil=((122, SPELL_A),)),
    item(9006, "tst kill-spell ring", abil=((153, SPELL_B),)),
    item(9007, "tst immunity ring", abil=((139, 30),)),
    item(9008, "tst immunity zero", abil=((139, 0),)),
    item(9009, "tst hidden negator", in_game=0, neg=(SPELL_A,)),
    item(9010, "tst remove zero", abil=((122, 0), (73, 0))),
    item(9011, "tst last slot", abil=tuple((1, 1) for _ in range(19)) + ((122, SPELL_B),)),
    item(9012, "tst wrong ability", abil=((122, 9999), (73, 9999))),
]
EXTRA_SPELLS = [
    spell(9101, "tst dispeller", 1, "x", abil=((73, ABIL_X),)),
    spell(9102, "tst remover", 0, "Monster #1", abil=((122, SPELL_A),)),
    spell(9103, "tst killer", 0, "", abil=((153, SPELL_A),)),            # not in game: skipped with bOnlyInGame
    spell(9104, "tst immune learnable", 1, "", abil=((139, 30),)),
    spell(9105, "tst immune item", 0, "Cast: Item #5", abil=((139, 30),)),
    spell(9106, "tst immune item first", 0, "Item #5, Item #6", abil=((139, 30),)),
    spell(9107, "tst immune plain", 0, "Monster #1", abil=((139, 30),)),
    spell(9108, "tst immune upper", 0, "Cast: ITEM #5", abil=((139, 30),)),
    spell(9109, "tst short cast", 0, "ab", abil=((124, ABIL_Y),)),
    spell(9110, "tst three chars", 0, "abc", abil=((124, ABIL_Y),)),
    spell(9111, "tst endcast fixed", 0, "Monster #1", abil=((151, SPELL_A), (73, ABIL_X))),
    spell(9112, "tst endcast range", 0, "Monster #1", abil=((151, 0),)),
]
EXTRA_SPELLS[11].update({"MinBase": 9101, "MaxBase": 9103})
EXTRA_SPELLS[10].update({"ReqLevel": 0})
# a spell with a damage that grows with level and nothing at level 0 (the level prompt), with and without an endcast
EXTRA_SPELLS += [
    spell(9113, "tst grows", 0, "Monster #1", abil=((1, 0),)),
    spell(9114, "tst grows endcast", 0, "Monster #1", abil=((151, 0),)),
    spell(9115, "tst big range", 0, "Monster #1", abil=((151, 0),)),
    spell(9116, "tst grows range", 0, "Monster #1", abil=((151, 0),)),
]
EXTRA_SPELLS[12].update({"MinInc": 2, "MinIncLVLs": 0})                  # nothing at any level: MME asks for one
EXTRA_SPELLS[13].update({"MaxInc": 3, "MaxIncLVLs": 0})
EXTRA_SPELLS[15].update({"MaxInc": 3, "MaxIncLVLs": 1, "Cap": 20, "ReqLevel": 1, "MinBase": 9101, "MaxBase": 9101})
EXTRA_SPELLS[14].update({"MinBase": 40000, "MaxBase": 40001})
for r in EXTRA_ITEMS:
    items[r["Number"]] = {**{"Abil-%d" % i: 0 for i in range(20)}, **{"AbilVal-%d" % i: 0 for i in range(20)},
                          **{"NegateSpell-%d" % i: 0 for i in range(10)}, "Name": "", **r}
base_spell = {**{"Abil-%d" % i: 0 for i in range(10)}, **{"AbilVal-%d" % i: 0 for i in range(10)},
              "Name": "", "Casted By": "", "Learnable": 0, "MinBase": 0, "MaxBase": 0, "MinInc": 0, "MaxInc": 0,
              "MinIncLVLs": 0, "MaxIncLVLs": 0, "Dur": 0, "DurInc": 0, "DurIncLVLs": 0, "Cap": 0, "ReqLevel": 0}
for r in EXTRA_SPELLS:
    spells[r["Number"]] = {**base_spell, **r}


def cb_pos(text):
    return (text or "").lower().find("item #") + 1


def spell_row(r):
    return [r["Number"], r["Name"] or "", int(r["Learnable"] or 0), len(r["Casted By"] or ""), cb_pos(r["Casted By"]),
            r["MinBase"], r["MaxBase"], r["MinInc"], r["MaxInc"], r["MinIncLVLs"], r["MaxIncLVLs"], r["Dur"], r["DurInc"],
            r["DurIncLVLs"], r["Cap"], r["ReqLevel"],
            [x for i in range(10) for x in (r["Abil-%d" % i], r["AbilVal-%d" % i]) if r["Abil-%d" % i]]]


def item_row(r):
    return [r["Number"], r["Name"] or "", int(r["In Game"] or 0),
            [r["NegateSpell-%d" % i] for i in range(10) if r["NegateSpell-%d" % i]],
            [x for i in range(20) for x in (r["Abil-%d" % i], r["AbilVal-%d" % i]) if r["Abil-%d" % i]]]


cases = []


def array(vals):
    return {"ref": "array:long", "values": list(vals)}


def add(cid, steps, forms=None, globs=None, extra=None):
    c = {"id": cid, "steps": steps}
    if forms:
        c["forms"] = forms
    if globs:
        c["globals"] = globs
    c.update(extra or {})
    cases.append(c)


RES = "%lvf.frmResults.lvResults"
FIND = lambda sp, ab, lv=0: {"call": "FindNegates", "args": [array(sp), array(ab), lv], "capture": {"rows": RES}}

# ---------------------------------------------------------------------------------------------- FindNegates
only = {"bOnlyInGame": True}
anyg = {"bOnlyInGame": False}
add("neg-empty", [FIND([0], [0])], globs=only)
add("neg-empty-level", [FIND([0], [0], 50)], globs=only)
add("neg-only-abil-zero", [FIND([0], [0, ABIL_X])], globs=only)
add("neg-spell-a", [FIND([SPELL_A], [0])], globs=only)
add("neg-spell-a-all", [FIND([SPELL_A], [0])], globs=anyg)
add("neg-spell-b", [FIND([SPELL_B], [0])], globs=only)
add("neg-spell-b-all", [FIND([SPELL_B], [0])], globs=anyg)
add("neg-two-spells", [FIND([SPELL_A, SPELL_B], [0])], globs=only)
add("neg-zero-and-spell", [FIND([0, SPELL_A], [0, 0])], globs=only)
add("neg-ability-x", [FIND([9999], [0, ABIL_X])], globs=only)
add("neg-ability-x-all", [FIND([9999], [0, ABIL_X])], globs=anyg)
add("neg-ability-y", [FIND([9999], [0, ABIL_Y])], globs=only)
add("neg-ability-y-all", [FIND([9999], [0, ABIL_Y])], globs=anyg)
add("neg-ability-and-spell", [FIND([SPELL_A], [0, ABIL_X, ABIL_Y])], globs=only)
add("neg-level-20", [FIND([9999], [0], 20)], globs=only)
add("neg-level-20-all", [FIND([9999], [0], 20)], globs=anyg)
add("neg-level-29", [FIND([9999], [0], 29)], globs=anyg)
add("neg-level-30", [FIND([9999], [0], 30)], globs=anyg)
add("neg-level-1000", [FIND([9999], [0], 1000)], globs=anyg)
add("neg-level-and-spell", [FIND([SPELL_A], [0, ABIL_X], 25)], globs=anyg)
add("neg-unmatched", [FIND([9999], [0, 9999])], globs=anyg)
# real spells the data actually targets
targeted = {}
for r in spells.values():
    for i in range(10):
        if r["Abil-%d" % i] in (122, 153) and r["AbilVal-%d" % i] > 0:
            targeted.setdefault(r["AbilVal-%d" % i], 0)
            targeted[r["AbilVal-%d" % i]] += 1
for n in pick("target", list(targeted), 8):
    add("neg-real-%d" % n, [FIND([n], [0])], globs=only if n % 2 else anyg)
dispel = sorted({r["AbilVal-%d" % i] for r in spells.values() for i in range(10)
                 if r["Abil-%d" % i] in (73, 124) and r["AbilVal-%d" % i] > 0})
for n in pick("dispel", dispel, 4):
    add("neg-realabil-%d" % n, [FIND([9999], [0, n])], globs=only)

# ---------------------------------------------------------------------------- the menu handlers' arrays
LISTS = [
    {"proc": "mnuSpellsPopUpItem_Click", "name": "lvSpells"},
]
FRM_CAP = {"sp": "%g.gNegSpells", "ab": "%g.gNegAbils", "lv": "%g.gNegLevel"}


def list_menu(cid, n, answers=None, globs=None, avg=None):
    g = dict(globs or {})
    g["bOnlyInGame"] = True
    add(cid, [{"call": "mnuSpellsPopUpItem_Click", "args": [5], "capture": {**FRM_CAP, "rows": RES}}],
        globs=g, extra={"work_list": {"name": "lvSpells", "text": str(n)}, "inputbox": answers or []})


def pane_menu(cid, tag, globs=None):
    g = dict(globs or {})
    g["bOnlyInGame"] = True
    add(cid, [{"call": "mnuAuxPopUpItem_Click", "args": [5], "capture": {**FRM_CAP, "rows": RES}}],
        globs=g, extra={"work_list": {"name": "lvMonsterDetail", "text": "1", "tag": "spell", "subtag": tag}})


plain = pick("menu", [n for n in spells if n < 9000], 6)
for n in plain:
    list_menu("menu-%d" % n, n)
for n in (416, 417, 418, 486, 512, 541, 581, 9111, 9112):
    list_menu("menu-endcast-%d" % n, n)
list_menu("menu-missing", 99999)
list_menu("menu-zero", 0)
list_menu("menu-grows-ask-cancel", 9113, [""])
list_menu("menu-grows-ask-30", 9113, ["30"])
list_menu("menu-grows-ask-zero", 9113, ["0"])
list_menu("menu-grows-ask-neg", 9113, ["-5"])
list_menu("menu-grows-ask-frac", 9113, ["12.5"])
list_menu("menu-grows-ask-huge", 9113, ["20000"])
list_menu("menu-grows-ask-overflow", 9113, ["70000"])
list_menu("menu-grows-endcast-ask", 9114, ["10"])
list_menu("menu-35-ask", 35, ["25"])
list_menu("menu-35-avg", 35, ["0"], globs={"gAvgLevelMaxAllStats": 40.0})
list_menu("menu-bigrange", 9115)
list_menu("menu-grows-range-ask", 9116, ["5"])
list_menu("menu-grows-range-avg", 9116, globs={"gAvgLevelMaxAllStats": 12.0})
pane_menu("pane-grows-range", "9116@3")
for n in (35, 265):
    list_menu("menu-level-spell-%d" % n, n, ["15"])
for n in (416, 9112):
    list_menu("menu-avg-%d" % n, n, globs={"gAvgLevelMaxAllStats": 55.5})
pane_menu("pane-plain", "57")
pane_menu("pane-level", "9113@30")
pane_menu("pane-level-endcast", "9114@12")
pane_menu("pane-at-first", "@30")
pane_menu("pane-endcast-real", "417")
pane_menu("pane-endcast-level", "416@20")
pane_menu("pane-missing", "99999@5")
pane_menu("pane-at-end", "35@")
pane_menu("pane-huge-level", "35@40000")
pane_menu("pane-35", "35@25")

# ----------------------------------------------------------------------------------------- LookUpSpellCast
def lv(n):
    return {"cliplv": {"name": "lvSpells", "columns": [["#", 600]], "rows": [{"Text": str(n), "Sel": True}]}}


def simple_cast(cb):
    refs = re.findall(r"(?i)(room |monster #|textblock #|textblock\(rndm\) #|item #|spell #|shop[^ ]* #|group[^ ]*: |npc #|room\()", cb or "")
    ok = all(x.lower() in ("item #", "monster #", "spell #", "npc #") for x in refs)
    for m in re.finditer(r"(?i)item #(\d+)", cb or ""):
        it = items.get(int(m.group(1)))
        if it is None or it["ItemType"] == 8:
            ok = False
    return ok and 5 <= len(cb or "") <= 300


castable = [n for n, r in spells.items() if n < 9000 and simple_cast(r["Casted By"])]
cast_needs = {"items": set(), "mons": set(), "spells": set()}
for n in pick("cast", castable, 18):
    add("cast-%d" % n, [{"call": "LookUpSpellCast", "args": [lv(n)], "capture": {"rows": RES, "caption": "%f.frmResults.lblCaption.Caption"}}],
        globs={"bHideRecordNumbers": n % 5 == 0})
    cast_needs.setdefault("pick", []).append(n)
short = [n for n, r in spells.items() if n < 9000 and len(r["Casted By"] or "") < 5]
for n in pick("castshort", short, 3):
    add("cast-short-%d" % n, [{"call": "LookUpSpellCast", "args": [lv(n)]}])
add("cast-missing", [{"call": "LookUpSpellCast", "args": [lv(99999)]}])
add("cast-none-selected", [{"call": "LookUpSpellCast",
                            "args": [{"cliplv": {"name": "lvSpells", "columns": [], "rows": [{"Text": "5"}]}}]}])
add("cast-invented", [{"call": "LookUpSpellCast", "args": [lv(9102)], "capture": {"rows": RES, "caption": "%f.frmResults.lblCaption.Caption"}}])

# ------------------------------------------------------------------------------------------------- data
need = {"items": set(), "mons": set(), "spells": set()}
for c in cases:
    if not c["id"].startswith("cast-"):
        continue
    for st in c["steps"]:
        n = int(st["args"][0]["cliplv"]["rows"][0]["Text"])
        for m in re.finditer(r"(?i)(item|monster|npc|spell) #(\d+)", spells.get(n, {}).get("Casted By", "") or ""):
            k = {"item": "items", "monster": "mons", "npc": "mons", "spell": "spells"}[m.group(1).lower()]
            need[k].add(int(m.group(2)))
data = {
    "items": [item_row(r) for r in items.values()],
    "spells": [spell_row(r) for r in spells.values()],
    "cast": {str(n): spells[n]["Casted By"] for n in sorted(spells) if n in castable or n in short or n == 9102},
    "names": {"items": [[n, items[n]["Name"] if n in items else None] for n in sorted(need["items"])],
              "mons": [[n, mons.get(n)] for n in sorted(need["mons"])],
              "spells": [[n, spells[n]["Name"] if n in spells else None] for n in sorted(need["spells"])]},
}
need_items = need["items"]

FRM = {
    "file": "frmMain.frm",
    "decls": ["bMapCancelFind", "nMapStartMap"],
    "extra": ["Public gNegSpells() As Long", "Public gNegAbils() As Long", "Public gNegLevel As Long",
              "Public bNoAlwaysOnTop As Boolean"],
    "me_is_form": True,
    "procs": ["FindNegates", "LookUpSpellCast", "mnuSpellsPopUpItem_Click", "mnuAuxPopUpItem_Click"],
    "subs": {},
    "subs_all": {},
}
COPY = ["Dim zz As Long", "ReDim gNegSpells(UBound(nSpells()))", "ReDim gNegAbils(UBound(nAbils()))",
        "For zz = 0 To UBound(nSpells()): gNegSpells(zz) = nSpells(zz): Next zz",
        "For zz = 0 To UBound(nAbils()): gNegAbils(zz) = nAbils(zz): Next zz", "gNegLevel = nLevel",
        "Call FindNegates(nSpells(), nAbils(), nLevel)"]
for proc in ("mnuSpellsPopUpItem_Click", "mnuAuxPopUpItem_Click"):
    FRM["subs"][proc] = {"Call FindNegates(nSpells(), nAbils(), nLevel)": "\n".join(COPY)}
FRM["subs_all"]["FindNegates"] = {
    "Load frmProgressBar": "", "Set frmProgressBar.objFormOwner = Me": "", "frmProgressBar.Show vbModeless, Me": "", "Call frmResults.SetupResultsWindow(False, Me)": "",
    "frmResults.Show vbModeless, IIf(bNoAlwaysOnTop, Nothing, Me)": "", "frmResults.SetFocus": "", "frmMain.SetFocus": "",
}
FRM["subs_all"]["LookUpSpellCast"] = {
    "Call frmResults.SetupResultsWindow(False, Me, nMapStartMap)": "",
    "frmResults.Show vbModeless, IIf(bNoAlwaysOnTop, Nothing, Me)": "",
}

scn = {
    "family": "spell_tools",
    "description": "Spells explorer lookups: FindNegates, the arrays the list and pane menus build for it, LookUpSpellCast",
    "notes": [
        "FindNegates and LookUpSpellCast are MME's own procedures cut from frmMain.frm; the progress bar is a no-op and the "
        "Results window is a recording ListView (frmResults.lvResults). Rows are read as [Text, Tag, cells].",
        "The menu cases run mnuSpellsPopUpItem_Click(5) and mnuAuxPopUpItem_Click(5) with objWorkingListView standing for the "
        "selected row; the call to FindNegates is preceded by a copy of its three arguments (gNegSpells, gNegAbils, gNegLevel).",
        "InputBox answers are the case's \"inputbox\" list (an empty answer is Cancel).",
        "data.items is [number, name, in game, [negated spells], [ability, value ...]]; data.spells is [number, name, learnable, "
        "length of Casted By, 1-based position of 'item #' in it (0 none), MinBase, MaxBase, MinInc, MaxInc, MinIncLVLs, "
        "MaxIncLVLs, Dur, DurInc, DurIncLVLs, Cap, ReqLevel, [ability, value ...]].",
        "Rows numbered 9001 and up are invented (db_extra).",
    ],
    "database": True,
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Spells": EXTRA_SPELLS},
    "data": data,
    "forms": {"frmMain": {"lvCast": {"$recordlv": []}}, "frmResults": {"lvResults": {"$recordlv": []}}},
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

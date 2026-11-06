"""Writes fixtures/scenarios/item_manager.json (FEAT-LISTS Item Manager actions):
python3 -I tools/oracle/gen/item_manager.py

Runs modListViewExt LV_SetActionCell, LV_CopySelectedActionsToClipboard, LV_DeleteSelected and
LV_InvertSelection (cut from the module, unchanged) over a mutable ListView standing in for
frmMain.lvItemManager, with the stock Items table behind GetItemWeight / GetItemType. The stock
items the cases name are copied into the scenario's "data" for the Rust test. Needs the converted
stock SQLite (tools/mdb-fetch.sh). Deterministic (splitmix64)."""
import glob
import json
import os
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "item_manager.json")
cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
dbs = sorted(glob.glob(os.path.join(cache, "mdb", "eba20f06*.sqlite")))
if not dbs:
    sys.exit("no converted database; run tools/mdb-fetch.sh")
con = sqlite3.connect(dbs[0])
con.row_factory = sqlite3.Row
M64 = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def int(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)

    def chance(self, pct):
        return self.int(1, 100) <= pct

    def pick(self, xs):
        return xs[self.int(0, len(xs) - 1)]


rng = Rng(0x19A11)
ITEMS = {r["Number"]: dict(r) for r in con.execute("select * from Items order by rowid")}


def first(pred, n):
    return [k for k, r in ITEMS.items() if pred(r)][:n]


# a few items of each type MME's copy command distinguishes, with and without weight
POOL = (first(lambda r: r["ItemType"] == 0 and r["Encum"] > 0, 4) + first(lambda r: r["ItemType"] == 1 and r["Encum"] > 0, 4)
        + first(lambda r: r["ItemType"] == 8, 2) + first(lambda r: r["ItemType"] == 9, 2)
        + first(lambda r: r["ItemType"] == 5, 2) + first(lambda r: r["ItemType"] == 10, 2)
        + first(lambda r: r["Encum"] == 0, 2) + first(lambda r: r["Encum"] >= 30, 3))
BAD = [0, 99999, -4]

FLAGS = ["", "", "DROP", "DROP x3", "HIDE", "HIDE x2", "PICKUP", "PICKUP x2", "USE", "USE x4", "SELL", "SELL x5", "BUY", "BUY x2",
         "CARRIED", "CARRIED x2", "carried x3", "STASH", "STASH x5", "MANUAL", "  drop  x2 ", "a x", "DROPx3", "x7", "CARRIED x1"]
SOURCES = ["Inventory", "Inventory", "Ground", "Equipped", "Saved", "Manual", "inventory stash"]
QTYS = ["", "1", "1", "2", "3", "7", "0", "-2", "x"]
NAMES = ["rope", "iron sword", "Bronze Key", "healing potion", "old scroll", "mule pack", "wand of fire"]


def row(item=None, flag=None, nsubs=10, sel=None):
    item = rng.pick(POOL + BAD[:2]) if item is None else item
    flag = rng.pick(FLAGS) if flag is None else flag
    subs = [rng.pick(NAMES), flag, rng.pick(QTYS), rng.pick(SOURCES), str(rng.int(0, 40)), "", "", "", "", ""][:nsubs]
    r = {"Text": str(item), "Sub": subs}
    if sel is None:
        sel = rng.chance(55)
    if sel:
        r["Sel"] = True
    return r


def rows(n, **kw):
    return [row(**kw) for _ in range(n)]


FRM = [{"file": "modListViewExt.bas", "procs": ["ParseActionAndQty", "LV_SetActionCell", "LV_CopySelectedActionsToClipboard",
                                                 "LV_DeleteSelected", "LV_InvertSelection"]}]
LV = {"control": "lvItemManager"}
READ = {"lv": "%lv.lvItemManager", "weight": "%c.txtInvenAddWeight.Text", "clip": "%clip", "prompt": "%g.bPromptSave"}
cases = []


def add(prefix, proc, args, lvrows, **kw):
    c = {"id": "%s%03d" % (prefix, sum(1 for x in cases if x["id"].startswith(prefix))),
         "forms": {"frmMain": {"lvItemManager": {"$listview": lvrows}}},
         "steps": [{"call": proc, "args": [LV] + args}], "read": READ}
    c["forms"]["frmMain"]["txtInvenAddWeight"] = {"Text": str(kw.pop("weight", 0))}
    c.update(kw)
    cases.append(c)


def msgs():
    return rng.pick([None, [6], [7], [6, 6], [7, 6]])


# --- a. LV_SetActionCell
for action in (9, 10, 11, 12, 15, 16, 17, 18, 99, 13, 0):
    for k in range(26 if action != 99 else 6):
        kw = {"weight": rng.pick([0, 3, 20, 80, 500, 100000]), "globals": {"bCharLoaded": rng.chance(50), "bStartup": rng.chance(20)}}
        m = msgs()
        if m:
            kw["msgbox"] = m
        add("a", "LV_SetActionCell", [action], rows(rng.int(1, 5), nsubs=rng.pick([10, 10, 10, 10, 5, 4, 3, 2, 1, 0])), **kw)
    # hand cases that walk the whole cycle on single rows
    for flag in ("", "DROP", "HIDE", "PICKUP", "USE", "SELL", "BUY", "CARRIED", "STASH", "DROP x2", "CARRIED x3", "STASH x1",
                 "DROP x99999999999", "DROP x2147483647", "SELL x2147483647"):
        for q in ("", "4"):
            r = row(item=POOL[0], flag=flag, sel=True)
            r["Sub"][2] = q
            add("a", "LV_SetActionCell", [action], [r], weight=50)
# carrying with the weight prompt: the three conditions, item weight and quantity
heavy = first(lambda r: r["Encum"] >= 30, 1)[0]
for weight in (0, 29, ITEMS[heavy]["Encum"], ITEMS[heavy]["Encum"] * 3, 5000):
    for src in ("Inventory", "Ground", "inventory"):
        for flag in ("", "CARRIED", "SELL x3", "STASH"):
            for ans in ([6], [7]):
                r = row(item=heavy, flag=flag, sel=True)
                r["Sub"][2] = "3"
                r["Sub"][3] = src
                add("a", "LV_SetActionCell", [12], [r], weight=weight, msgbox=ans)
for qty in ("40000000", "99999999999"):
    r = row(item=heavy, flag="", sel=True)
    r["Sub"][2] = qty
    add("a", "LV_SetActionCell", [12], [r], weight=100)
add("a", "LV_SetActionCell", [12], [], weight=5)
add("a", "LV_SetActionCell", [12], rows(3, sel=False), weight=5)
r1, r2 = row(item=POOL[0], flag="DROP x99999999999", sel=True), row(item=POOL[1], flag="DROP", sel=True)
add("a", "LV_SetActionCell", [9], [r2, r1, dict(r2, Text=str(POOL[2]))], weight=0)

# --- b. LV_CopySelectedActionsToClipboard
for k in range(70):
    add("b", "LV_CopySelectedActionsToClipboard", [], rows(rng.int(1, 6), nsubs=rng.pick([10, 10, 10, 2, 1, 0])))
for t in (0, 1, 8, 9, 5, 10):
    for flag in ("USE", "USE x2"):
        for item in [i for i in POOL if ITEMS[i]["ItemType"] == t][:1] + [0, 99999]:
            add("b", "LV_CopySelectedActionsToClipboard", [], [row(item=item, flag=flag, sel=True)])
add("b", "LV_CopySelectedActionsToClipboard", [], [])
add("b", "LV_CopySelectedActionsToClipboard", [], [row(item=POOL[0], flag="DROP x99999999999", sel=True)])
add("b", "LV_CopySelectedActionsToClipboard", [], [row(item=POOL[0], flag="DROP x5", sel=True), row(item=POOL[1], flag="STASH x2", sel=True)])
add("b", "LV_CopySelectedActionsToClipboard", [], [dict(row(item=POOL[0], flag="USE", sel=True), Text="99999999999")])

# --- c. LV_DeleteSelected
for k in range(60):
    n = rng.int(0, 7)
    kw = {"globals": {"bCharLoaded": rng.chance(60), "bStartup": rng.chance(15)}}
    m = rng.pick([None, [6], [7]])
    if m:
        kw["msgbox"] = m
    add("c", "LV_DeleteSelected", [], rows(n, nsubs=rng.pick([10, 10, 10, 1, 0])), **kw)
for n in range(1, 6):
    for pick in range(n):
        add("c", "LV_DeleteSelected", [], [row(flag="CARRIED" if i == pick else "", sel=(i == pick)) for i in range(n)])

# --- d. LV_InvertSelection
for n in (0, 1, 2, 3, 5, 8):
    for k in range(3):
        add("d", "LV_InvertSelection", [], rows(n))

used = sorted({int(r["Text"]) for c in cases for r in c["forms"]["frmMain"]["lvItemManager"]["$listview"]
               if int(r["Text"]) in ITEMS})
data = {"items": [{"n": n, "name": ITEMS[n]["Name"], "type": ITEMS[n]["ItemType"], "encum": ITEMS[n]["Encum"]} for n in used]}
scn = {
    "family": "item_manager",
    "description": "FEAT-LISTS Item Manager actions (modListViewExt LV_SetActionCell, LV_CopySelectedActionsToClipboard, "
                   "LV_DeleteSelected, LV_InvertSelection) over a mutable ListView.",
    "notes": [
        "The Item Manager is frmMain.lvItemManager (ListView shim). A row is {Text, Sub, Sel}; the dump after the call has the same "
        "shape (sel only when selected). The action cell is sub item 2 (Flag), QTY 3, Source 4, Enc 5 (1-based, as MME).",
        "frmMain.RefreshAll, lvItemManager_ItemClick and SetFocus do nothing here; bPromptSave is read back from the global.",
        "A case's msgbox lists the answers to the boxes MME shows (6 Yes, 7 No); without it the default button answers "
        "(Yes for the encumbrance question, No for the delete confirmation).",
        "data holds the stock Items rows the cases name (name, ItemType, Encum) for the Rust test. Generated by "
        "tools/oracle/gen/item_manager.py.",
    ],
    "database": True,
    "paste_procs": True,
    "frm": FRM,
    "data": data,
    "forms": {"frmMain": {"lvItemManager": {"$listview": []}, "txtInvenAddWeight": {"Text": "0"}}},
    "tolerance": {"default": 0},
    "cases": cases,
}
with open(OUT, "w", newline="\n") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("wrote", OUT, len(cases), "cases,", len(used), "items", file=sys.stderr)

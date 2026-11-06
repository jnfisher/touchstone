#!/usr/bin/env python3
"""Writes fixtures/scenarios/misc_detail.json (FAM-20 detail panes, card P3-37).

    python3 -I tools/oracle/gen/misc_detail.py [DB.sqlite]

The oracle runs MME's own modMain procedures against the stock database and records what they write into the
detail controls:

- `class-*`, `race-*`  PullClassDetail / PullRaceDetail: every class and race, a missing record and bStartup.
- `shop-*`             PullShopDetail: every shop (stock list or the training cost table), several charm and
                       buy/sell combinations; the rows, the text, the charm label and the "Assigned To" rows.
- `npc-*`              AddRoomNPCCommandRefs: the room NPCs of the stock data plus invented NPCs (rows numbered
                       9001 and up) that reach the teleport rows, which no stock greet command does.
- `spell-*`            PullSpellDetail with no character, `spellc-*` with the combat option on.
- `char-*`             PullSpellDetail for two loaded characters (RefreshAll cut from frmMain.frm as the pipeline
                       family does, taken from that family's scenario): one case per character and option set,
                       many spells per case, each followed by a `capture` of the text box and the location list.

Deterministic: selection by sha256 of fixed tags; the clock and Python's random are never used. The rows the
Rust test needs sit once under "data" (the closure of every record the cases can reach).
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "misc_detail.json")
PIPELINE = os.path.join(ROOT, "fixtures", "scenarios", "pipeline.json")

TB = {"control": "txtDetail"}
LV_LOC = {"control": "lvLoc"}
LV_DET = {"control": "lvDetail"}
LV_MAP = {"control": "lvMap"}


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


# ---------------------------------------------------------------------------------------- invented rows
# Rows numbered 9001 and up are added to the database the oracle reads (db_extra) and to "data".
EXTRA_TB = [
    # a greet tree reaching every branch of AddRoomNPCCommandRefs
    {"Number": 20101, "LinkTo": 0, "Action": "port me:20102\nchat|talk*:20103\nvia link:20104\nsame spot:20105\n"
                                           "nomap:20106\nlost:20107\nbad number:0\nzero:\n:20103\nalso port:20102\nlast:20103\n"},
    {"Number": 20102, "LinkTo": 0, "Action": "say off you go\nteleport 334 1\n"},
    {"Number": 20103, "LinkTo": 0, "Action": "say hello there\n"},
    {"Number": 20104, "LinkTo": 20108, "Action": "say here we go\n"},
    {"Number": 20105, "LinkTo": 0, "Action": "teleport 334 1\n"},
    {"Number": 20106, "LinkTo": 0, "Action": "teleport 335\n"},
    {"Number": 20107, "LinkTo": 0, "Action": "teleport 99999 1\n"},
    {"Number": 20108, "LinkTo": 0, "Action": "teleport 340 1\n"},
    # greet filters, shapes of the text
    {"Number": 20111, "LinkTo": 0, "Action": "heh:20103\n"},
    {"Number": 20112, "LinkTo": 0, "Action": "NOTHING:20103\n"},
    {"Number": 20113, "LinkTo": 0, "Action": "yada:20103\n"},
    {"Number": 20114, "LinkTo": 0, "Action": "hehe:20103\n"},
    {"Number": 20115, "LinkTo": 0, "Action": "Shit:20103\n"},
    {"Number": 20116, "LinkTo": 0, "Action": "no colon here\n"},
    {"Number": 20117, "LinkTo": 0, "Action": "only:20103"},
    {"Number": 20118, "LinkTo": 0, "Action": "a:20102\nb:20102\n"},
    {"Number": 20119, "LinkTo": 0, "Action": "go:20105"},
    {"Number": 20120, "LinkTo": 0, "Action": "a:20103\r\nb:20103\r\n"},
    {"Number": 20121, "LinkTo": 0, "Action": "key|word:20103\n*star*:20103\n"},
    {"Number": 20122, "LinkTo": 0, "Action": "q:99999\nr:20102\n"},
    {"Number": 20123, "LinkTo": 0, "Action": "x:20109\n"},
    {"Number": 20109, "LinkTo": 20102, "Action": ""},
    {"Number": 20124, "LinkTo": 0, "Action": "x:20103\n\n\ny:20102\n"},
    {"Number": 20125, "LinkTo": 0, "Action": "tele a:20102\ntele b:20106\ntele c:20108\nchat:20103\n"},
    {"Number": 20126, "LinkTo": 0, "Action": "a:\n"},
    {"Number": 20127, "LinkTo": 0, "Action": "a:20102 and more\nb:12abc\nc:1e3\n"},
    {"Number": 20128, "LinkTo": 0, "Action": "a:99999999999\nb:20102\n"},
]
EXTRA_MON = [
    {"Number": 9001, "Name": "Test Guide", "GreetTXT": 20101},
    {"Number": 9002, "Name": "Heh One", "GreetTXT": 20111},
    {"Number": 9003, "Name": "Nothing One", "GreetTXT": 20112},
    {"Number": 9004, "Name": "Yada One", "GreetTXT": 20113},
    {"Number": 9005, "Name": "Hehe One", "GreetTXT": 20114},
    {"Number": 9006, "Name": "Shit One", "GreetTXT": 20115},
    {"Number": 9007, "Name": "No Colon", "GreetTXT": 20116},
    {"Number": 9008, "Name": "Only One", "GreetTXT": 20117},
    {"Number": 9009, "Name": "Two Ports", "GreetTXT": 20118},
    {"Number": 9010, "Name": "No Trailing LF", "GreetTXT": 20119},
    {"Number": 9011, "Name": "CRLF", "GreetTXT": 20120},
    {"Number": 9012, "Name": "Keywords", "GreetTXT": 20121},
    {"Number": 9013, "Name": "Dangling", "GreetTXT": 20122},
    {"Number": 9014, "Name": "Link Only", "GreetTXT": 20123},
    {"Number": 9015, "Name": "Blank Lines", "GreetTXT": 20124},
    {"Number": 9016, "Name": "Teleports", "GreetTXT": 20125},
    {"Number": 9017, "Name": "Empty Line", "GreetTXT": 20126},
    {"Number": 9018, "Name": "Odd Numbers", "GreetTXT": 20127},
    {"Number": 9019, "Name": "Big Number", "GreetTXT": 20128},
    {"Number": 9020, "Name": "Missing Greet", "GreetTXT": 99998},
    {"Number": 9021, "Name": "No Greet", "GreetTXT": 0},
]
EXTRA_SHOPS = []
# Invented spells (9001 and up): the ability shapes no stock spell has (castsp/learnspell rows, ranged
# teleport/textblock/summon/endcast), class lists and difficulty/resist variants.
EXTRA_SPELLS = [
    {"Number": 9001, "Name": "Test Caster", "ReqLevel": 5, "Cap": 20, "EnergyCost": 1000, "ManaCost": 6, "Targets": 4,
     "Abil-0": 43, "AbilVal-0": 1, "Abil-1": 42, "AbilVal-1": 2, "Abil-2": 160, "AbilVal-2": 3, "Abil-3": 1, "AbilVal-3": 0,
     "MinBase": 4, "MaxBase": 9, "MinInc": 2, "MinIncLVLs": 1, "MaxInc": 3, "MaxIncLVLs": 1, "Dur": 5, "DurInc": 1, "DurIncLVLs": 2,
     "Learnable": 1, "Learned From": "Shop #3, Monster #2", "Casted By": "Item #1, Monster #5, Spell #1"},
    {"Number": 9002, "Name": "Test Hop", "Targets": 1, "Abil-0": 140, "AbilVal-0": 0, "Abil-1": 141, "AbilVal-1": 1,
     "MinBase": 334, "MaxBase": 336, "Learnable": 1},
    {"Number": 9003, "Name": "Test Block", "Targets": 1, "Abil-0": 148, "AbilVal-0": 0, "MinBase": 3, "MaxBase": 5,
     "Abil-1": 148, "AbilVal-1": 20101, "ReqLevel": 3},
    {"Number": 9004, "Name": "Test Summons", "Targets": 1, "Abil-0": 12, "AbilVal-0": 0, "MinBase": 1, "MaxBase": 3,
     "Abil-1": 12, "AbilVal-1": 7, "Abil-2": 140, "AbilVal-2": 334, "Abil-3": 141, "AbilVal-3": 1},
    {"Number": 9005, "Name": "Test Ender", "Targets": 1, "Abil-0": 164, "AbilVal-0": 50, "Abil-1": 151, "AbilVal-1": 0,
     "MinBase": 1, "MaxBase": 3, "Abil-2": 151, "AbilVal-2": 9002, "EnergyCost": 250},
    {"Number": 9006, "Name": "Test Nest", "Targets": 6, "Abil-0": 153, "AbilVal-0": 9001, "Abil-1": 43, "AbilVal-1": 9006,
     "Abil-2": 122, "AbilVal-2": 1, "Abil-3": 122, "AbilVal-3": 2},
    {"Number": 9007, "Name": "Test Classes", "Targets": 3, "Learnable": 1, "Diff": 50, "TypeOfResists": 1, "AttType": 3,
     "Classes": "(4,5,12)", "Abil-0": 17, "AbilVal-0": 0, "MinBase": 10, "MaxBase": 20, "Magery": 1, "MageryLVL": 2},
    {"Number": 9008, "Name": "Test Resist", "Targets": 13, "Learnable": 1, "Diff": 0, "TypeOfResists": 2, "AttType": 5,
     "Classes": "(7)", "Abil-0": 1, "AbilVal-0": 0, "MinBase": 5, "MaxBase": 8, "Abil-1": 144, "AbilVal-1": 0},
    {"Number": 9009, "Name": "Test Kai", "Targets": 99, "Magery": 5, "ReqLevel": 4, "Diff": 0, "AttType": 9,
     "Abil-0": 15, "AbilVal-0": 0, "MinBase": 1, "MaxBase": 1},
    {"Number": 9010, "Name": "Test Hard", "Targets": 2, "Diff": 250, "Abil-0": 8, "AbilVal-0": 0, "MinBase": 5, "MaxBase": 5,
     "Dur": 10, "Learnable": 1, "Abil-1": 185, "AbilVal-1": 3, "Abil-2": 12, "AbilVal-2": 29999},
]


class Tables:
    """Stock rows plus the invented ones, as dicts keyed the way MME's tables are."""

    def __init__(self, con):
        con.row_factory = sqlite3.Row

        def load(name, key, extra=()):
            rows = [dict(r) for r in con.execute('select * from "%s" order by rowid' % name)]
            rows += [dict(zip([c[1] for c in con.execute('pragma table_info("%s")' % name)], [None] * 99), **e) for e in extra]
            return rows
        self.items = {r["Number"]: r for r in load("Items", "Number")}
        self.monsters = {r["Number"]: r for r in load("Monsters", "Number", EXTRA_MON)}
        self.spells = {r["Number"]: r for r in load("Spells", "Number", EXTRA_SPELLS)}
        self.shops = {r["Number"]: r for r in load("Shops", "Number", EXTRA_SHOPS)}
        self.classes = {r["Number"]: r for r in load("Classes", "Number")}
        self.races = {r["Number"]: r for r in load("Races", "Number")}
        self.tb = {r["Number"]: r for r in load("TBInfo", "Number", EXTRA_TB)}
        self.rooms = {(r["Map Number"], r["Room Number"]): r for r in load("Rooms", "Number")}
        self.lairs = {r["GroupIndex"]: r for r in load("Lairs", "GroupIndex")}


def nz(v):
    return 0 if v is None else v


def ab(row, n):
    return [x for i in range(n) for x in (nz(row["Abil-%d" % i]), nz(row["AbilVal-%d" % i])) if True] if False else \
        [[nz(row["Abil-%d" % i]), nz(row["AbilVal-%d" % i])] for i in range(n) if nz(row["Abil-%d" % i]) or nz(row["AbilVal-%d" % i])]


# ---------------------------------------------------------------------------------------- data rows
# Each table is written once as {"fields": [...], "rows": [[...]]}; the Rust test indexes by field name.
ITEM_F = ["Number", "Name", "ItemType", "Price", "Currency", "Encum", "ArmourClass", "DamageResist", "WeaponType",
          "ArmourType", "Worn", "Accy", "StrReq", "Speed", "ClassRest", "Abilities", "Obtained From"]
SPELL_F = ["Number", "Name", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists", "Targets",
           "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs",
           "DurInc", "Abilities", "Learnable", "Learned From", "Casted By", "Classes"]
SHOP_F = ["Number", "ShopType", "MinLVL", "MaxLVL", "Markup%", "ClassRest", "Stock", "Assigned To"]
CLASS_F = ["Number", "Name", "MinHits", "MaxHits", "ExpTable", "MageryType", "MageryLVL", "WeaponType", "ArmourType",
           "CombatLVL", "Abilities"]
RACE_F = ["Number", "Name", "mINT", "mWIL", "mSTR", "mHEA", "mAGL", "mCHM", "xINT", "xWIL", "xSTR", "xHEA", "xAGL", "xCHM",
          "HPPerLVL", "ExpTable", "BaseCP", "Abilities"]


def item_row(t, n, full):
    """Gear stats only for the items a character wears or carries; Obtained From only for chests (the one
    place GetLocations reads it)."""
    r = t.items[n]
    if not full:
        return [n, r["Name"] or "", r["ItemType"], r["Price"], r["Currency"], 0, 0, 0, 0, 0, 0, 0, 0, 0, [], [],
                (r["Obtained From"] or "") if r["ItemType"] == 8 else ""]
    return [n, r["Name"] or "", r["ItemType"], r["Price"], r["Currency"], r["Encum"], r["ArmourClass"], r["DamageResist"],
            r["WeaponType"], r["ArmourType"], r["Worn"], r["Accy"], r["StrReq"], r["Speed"],
            [r["ClassRest-%d" % i] for i in range(10) if r["ClassRest-%d" % i]], ab(r, 20),
            (r["Obtained From"] or "") if r["ItemType"] == 8 else ""]


def spell_row(t, n):
    r = t.spells[n]
    f = ["ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists", "Targets", "Dur", "AttType",
         "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc"]
    return [n, r["Name"] or ""] + [nz(r[k]) for k in f] + [ab(r, 10), 1 if r["Learnable"] else 0,
                                                              r["Learned From"] or "", r["Casted By"] or "", r["Classes"] or ""]


def shop_row(t, n):
    r = t.shops[n]
    return [n, r["ShopType"], r["MinLVL"], r["MaxLVL"], r["Markup%"], r["ClassRest"],
            [[r["Item-%d" % x], r["Max-%d" % x], r["Time-%d" % x], r["Amount-%d" % x], r["%%-%d" % x]]
             for x in range(20) if r["Item-%d" % x]], r["Assigned To"] or ""]


# ---------------------------------------------------------------------------------------- closure
KEY = re.compile(r"(?i)(room |monster #|textblock #|textblock\(rndm\) #|item #|spell #|shop #|shop\(sell\) #|shop\(nogen\) #|"
                 r"group\(lair\): |group: |npc #)(\d+(?:/\d+)?)")
PAIR = re.compile(r"(\d+)/(\d+)")


class Need:
    """The records the cases can reach, grown from reference texts the way GetLocations follows them."""

    def __init__(self, t):
        self.t = t
        self.items, self.mons, self.spells, self.shops = set(), set(), set(), set()
        self.rooms, self.tbs, self.lairs = set(), set(), set()
        self._seen = set()

    def text(self, s):
        """A reference text (Obtained From, Assigned To, ...): what GetLocations opens from it."""
        work = [s or ""]
        while work:
            s = work.pop().lower()
            if s in self._seen:
                continue
            self._seen.add(s)
            for m in re.finditer(r"\[([\d\-]+)\]", s):
                self.lairs.add(m.group(1))
            for p in PAIR.finditer(s):
                self.rooms.add((int(p.group(1)), int(p.group(2))))
            for m in KEY.finditer(s):
                k, v = m.group(1).lower(), m.group(2)
                if "/" in v:
                    continue
                v = int(v)
                if k.startswith("monster") or k.startswith("npc"):
                    self.mons.add(v)
                elif k.startswith("item"):
                    if v not in self.items:
                        self.item(v)
                        if v in self.t.items and self.t.items[v]["ItemType"] == 8:
                            work.append(self.t.items[v]["Obtained From"])
                elif k.startswith("spell"):
                    self.spells.add(v)
                elif k.startswith("shop"):
                    if v not in self.shops:
                        self.shops.add(v)
                        if v in self.t.shops:
                            work.append(self.t.shops[v]["Assigned To"])
                            for x in range(20):
                                if self.t.shops[v]["Item-%d" % x]:
                                    self.items.add(self.t.shops[v]["Item-%d" % x])

    def item(self, n):
        self.items.add(n)

    def shop(self, n):
        self.shops.add(n)
        s = self.t.shops[n]
        for x in range(20):
            if s["Item-%d" % x]:
                self.items.add(s["Item-%d" % x])
        self.text(s["Assigned To"])

    def tb(self, n):
        """A textblock and what NPC commands reach from it (its LinkTo chain one deep, as the port reads)."""
        if n in self.t.tb:
            self.tbs.add(n)
            self.tbs.add(self.t.tb[n]["LinkTo"]) if self.t.tb[n]["LinkTo"] in self.t.tb else None
        else:
            self.tbs.add(n)

    def spell_refs(self, n, depth=0):
        """A spell and the spells, monsters and items its abilities name (PullSpellEQ / GetAbilityStats follow them)."""
        if n in self.spells and depth:
            return
        self.spells.add(n)
        r = self.t.spells.get(n)
        if not r or depth > 12:
            return
        for i in range(10):
            a, v = nz(r["Abil-%d" % i]), nz(r["AbilVal-%d" % i])
            if a in (42, 43, 122, 151, 153, 160) and v:
                self.spell_refs(v, depth + 1)
            elif a in (12, 146) and v:
                self.mons.add(v)
            elif a in (185, 1115) and v:
                self.items.add(v)
            elif a == 59 and v:
                pass
            elif a == 148 and v:
                self.textblock_casts(v, depth)
        # ranges: ability 12 / 151 with value 0 take their numbers from the spell's Min/Max
        mn, mx = nz(r["MinBase"]), nz(r["MaxBase"])
        for i in range(10):
            a, v = nz(r["Abil-%d" % i]), nz(r["AbilVal-%d" % i])
            if v == 0 and a in (12, 151) and 0 <= mn <= mx and mx - mn < 40:
                for y in range(mn, mx + 1):
                    (self.spell_refs(y, depth + 1) if a == 151 else self.mons.add(y))

    def textblock_casts(self, tbn, depth):
        self.tbs.add(tbn)
        a = (self.t.tb.get(tbn, {}).get("Action") or "")
        for part in a.split(":"):
            if part[:5] == "cast ":
                m = re.match(r"\s*(\d+)", part[5:])
                if m:
                    self.spell_refs(int(m.group(1)), depth + 1)


def tele_rooms(t, need, cur_map):
    """Rooms named by the teleport lines of the textblocks collected so far (the map defaults to the case's)."""
    for n in list(need.tbs):
        for a in [t.tb[n]["Action"] or ""] if n in t.tb else []:
            for m in re.finditer(r"teleport (\d+)(?: (\d+))?", a):
                need.rooms.add((int(m.group(2)) if m.group(2) else cur_map, int(m.group(1))))


def table_json(fields, rows):
    return {"fields": fields, "rows": rows}


def build_data(t, need, char_items, char_spells):
    char_items = set(char_items)
    items = sorted(n for n in (need.items | set(char_items)) if n in t.items)
    spells = sorted(n for n in (need.spells | set(char_spells)) if n in t.spells)
    shops = sorted(n for n in need.shops if n in t.shops)
    mons = sorted(n for n in need.mons if n in t.monsters)
    tbs = sorted(n for n in need.tbs if n in t.tb)
    rooms = sorted(k for k in need.rooms if k in t.rooms)
    lairs = {}
    return {
        "items": table_json(ITEM_F, [item_row(t, n, n in char_items) for n in items]),
        "spells": table_json(SPELL_F, [spell_row(t, n) for n in spells]),
        "shops": table_json(SHOP_F, [shop_row(t, n) for n in shops]),
        "classes": table_json(CLASS_F, [[n, t.classes[n]["Name"]] + [t.classes[n][k] for k in CLASS_F[2:-1]] + [ab(t.classes[n], 10)]
                                         for n in sorted(t.classes)]),
        "races": table_json(RACE_F, [[n, t.races[n]["Name"]] + [t.races[n][k] for k in RACE_F[2:-1]] + [ab(t.races[n], 10)]
                                      for n in sorted(t.races)]),
        "monsters": table_json(["Number", "Name", "GreetTXT"], [[n, t.monsters[n]["Name"] or "", nz(t.monsters[n]["GreetTXT"])] for n in mons]),
        "tbinfo": table_json(["Number", "LinkTo", "Action"], [[n, nz(t.tb[n]["LinkTo"]), t.tb[n]["Action"] or ""] for n in tbs]),
        "rooms": table_json(["Map Number", "Room Number", "Name", "Delay"],
                            [[m, r, t.rooms[(m, r)]["Name"] or "", nz(t.rooms[(m, r)]["Delay"])] for (m, r) in rooms]),
        "lairs": lairs,
    }


def lair_row(t, g):
    """Invented but fixed lair numbers on the real group index (the oracle's lair shim answers from this)."""
    r = t.lairs.get(g)
    mobs, exp = (nz(r["Mobs"]), nz(r["AvgExp"])) if r else (0, 0)
    d = {"nMobs": mobs, "nAvgExp": exp}
    if H("dc", g) % 3:
        d["nAvgDmgClearRound"] = H("dcv", g) % 60 + [0, 0.25, 0.5][H("dcf", g) % 3]
    if H("do", g) % 3:
        d["nDamageOut"] = H("dov", g) % 300
    return {k: v for k, v in d.items() if v != 0}


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    con = sqlite3.connect(db)
    t = Tables(con)
    need = Need(t)
    cases = []

    def add(cid, steps, forms=None, globals_=None, read=None, extra=None):
        d = {"id": cid, "steps": steps}
        if forms:
            d["forms"] = {"frmMain": forms}
        if globals_:
            d["globals"] = globals_
        if read:
            d["read"] = read
        if extra:
            d.update(extra)
        cases.append(d)

    # ---- class and race
    for n in sorted(t.classes):
        add("class-%d" % n, [{"call": "PullClassDetail", "args": [n, TB]}], read={"text": "%c.txtDetail.Text"})
    for n in sorted(t.races):
        add("race-%d" % n, [{"call": "PullRaceDetail", "args": [n, TB]}], read={"text": "%c.txtDetail.Text"})
    for n in (0, 99, -1, 9999):
        add("class-missing-%d" % n, [{"call": "PullClassDetail", "args": [n, TB]}], read={"text": "%c.txtDetail.Text"})
        add("race-missing-%d" % n, [{"call": "PullRaceDetail", "args": [n, TB]}], read={"text": "%c.txtDetail.Text"})
    add("class-startup", [{"call": "PullClassDetail", "args": [1, TB]}], globals_={"bStartup": True}, read={"text": "%c.txtDetail.Text"})
    add("race-startup", [{"call": "PullRaceDetail", "args": [1, TB]}], globals_={"bStartup": True}, read={"text": "%c.txtDetail.Text"})
    add("class-hide", [{"call": "PullClassDetail", "args": [13, TB]}], globals_={"bHideRecordNumbers": True}, read={"text": "%c.txtDetail.Text"})
    add("race-hide", [{"call": "PullRaceDetail", "args": [3, TB]}], globals_={"bHideRecordNumbers": True}, read={"text": "%c.txtDetail.Text"})

    # ---- shops
    combos = [(0, False), (30, False), (80, False), (50, True), (0, True), (100, True), (25, False), (15, True)]
    shop_read = {"text": "%c.txtDetail.Text", "rows": "%lv.lvDetail", "loc": "%lv.lvAssigned", "charm": "%c.lblCharmMod.Caption"}
    for n in sorted(t.shops):
        need.shop(n)
        combo_list = [(0, False)]
        if H("shopcombo", n) % 6 == 0:
            combo_list.append(combos[1 + H("shopcombo2", n) % (len(combos) - 1)])
        for k, (charm, sell) in enumerate(combo_list):
            f = {"txtCharStats": {"$array": {"5": {"Tag": str(charm)}}}, "chkGlobalFilter": {"Value": 1 if charm else 0}}
            g = {"bHideRecordNumbers": H("shophide", n) % 9 == 0, "nGlobalAttackTypeMME": H("shopatt", n) % 6}
            add("shop-%d-%d" % (n, k), [{"call": "PullShopDetail", "args": [n, LV_DET, TB, {"control": "lvAssigned"}, charm, sell]}],
                f, g, shop_read)
    for n in (0, 9999):
        add("shop-missing-%d" % n, [{"call": "PullShopDetail", "args": [n, LV_DET, TB, {"control": "lvAssigned"}, 0, False]}],
            None, None, shop_read)
    add("shop-startup", [{"call": "PullShopDetail", "args": [3, LV_DET, TB, {"control": "lvAssigned"}, 0, False]}],
        None, {"bStartup": True}, shop_read)
    # a list that already holds rows is cleared; the training table keeps its columns
    add("shop-clear", [{"call": "PullShopDetail", "args": [3, LV_DET, TB, {"control": "lvAssigned"}, 10, False]}],
        {"lvDetail": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}]}, "lvAssigned": {"$recordlv": [{"Text": "old", "Sub": []}]}},
        None, shop_read)
    train = sorted(n for n in t.shops if t.shops[n]["ShopType"] == 8)
    for n in train[:3]:
        for k, (charm, sell) in enumerate(((60, False), (60, True))):
            add("shop-train-%d-%d" % (n, k), [{"call": "PullShopDetail", "args": [n, LV_DET, TB, {"control": "lvAssigned"}, charm, sell]}],
                None, None, shop_read)

    # ---- NPC command references
    npc_rooms = {}
    for (m, r), row in t.rooms.items():
        if nz(row["NPC"]) > 0:
            npc_rooms.setdefault(row["NPC"], (m, r))
    greeted = sorted(n for n in npc_rooms if n in t.monsters and nz(t.monsters[n]["GreetTXT"]) > 0)
    plain = sorted(n for n in npc_rooms if n not in greeted)
    stock_npcs = pick("npc", greeted, 120) + pick("npcplain", plain, 8)
    for n in sorted(stock_npcs):
        m, r = npc_rooms[n]
        need.mons.add(n)
        g = t.monsters[n]["GreetTXT"] if n in t.monsters else 0
        if g:
            need.tb(g)
            for line in (t.tb.get(g, {}).get("Action") or "").split("\n"):
                mm = re.match(r"\s*(\d+)", line.split(":", 1)[-1])
                if mm:
                    need.tb(int(mm.group(1)))
        add("npc-%d" % n, [{"call": "AddRoomNPCCommandRefs", "args": [LV_MAP, n, m]}], read={"rows": "%lv.lvMap"})
        tele_rooms(t, need, m)
    for e in EXTRA_MON:
        n = e["Number"]
        need.mons.add(n)
        g = e["GreetTXT"]
        work = [g]
        while work:
            x = work.pop()
            if x in need.tbs or x not in t.tb:
                need.tbs.add(x)
                continue
            need.tbs.add(x)
            if t.tb[x]["LinkTo"]:
                work.append(t.tb[x]["LinkTo"])
            for line in (t.tb[x]["Action"] or "").split("\n"):
                mm = re.match(r"\s*(\d+)", line.split(":", 1)[-1])
                if mm and int(mm.group(1)) < 10 ** 8:
                    work.append(int(mm.group(1)))
        for cur_map in (1, 17):
            add("npc-%d-m%d" % (n, cur_map), [{"call": "AddRoomNPCCommandRefs", "args": [LV_MAP, n, cur_map]}], read={"rows": "%lv.lvMap"})
    for r in ((1, 334), (1, 335), (1, 340)):
        need.rooms.add(r)
    need.rooms.add((17, 334))
    for n, cur_map in ((0, 1), (-5, 1), (99999, 1), (9001, 0)):
        add("npc-odd-%d-%d" % (n, cur_map), [{"call": "AddRoomNPCCommandRefs", "args": [LV_MAP, n, cur_map]}], read={"rows": "%lv.lvMap"})
    add("npc-append", [{"call": "AddRoomNPCCommandRefs", "args": [LV_MAP, 9001, 1]}], {"lvMap": {"$recordlv": [
        {"Text": "Room: old (1/1)", "Tag": "1/1", "Sub": []}]}}, read={"rows": "%lv.lvMap"})


    # ---- spells: no character
    spell_read = {"text": "%c.txtDetail.Text", "loc": "%lv.lvLoc"}
    cap = {"text": "%c.txtDetail.Text", "loc": "%lv.lvLoc"}

    def sig(n):
        r = t.spells[n]
        a = tuple(sorted({nz(r["Abil-%d" % i]) for i in range(10) if nz(r["Abil-%d" % i])}))
        return (a[:4], nz(r["AttType"]), nz(r["Cap"]) > 0, nz(r["Dur"]) > 0)

    stock_spells = sorted(n for n in t.spells if n < 9000)
    groups = {}
    for n in stock_spells:
        groups.setdefault(sig(n), []).append(n)
    chosen = []
    for key in sorted(groups, key=lambda k: (len(groups[k]), k)):
        n = sorted(groups[key], key=lambda n: H("spsig", n))[0]
        if n not in chosen:
            chosen.append(n)
        if len(chosen) >= 70:
            break
    for n in (434, 336, 334, 198, 90, 935, 747, 515, 72):
        if n in t.spells and n not in chosen:
            chosen.append(n)
    for n in sorted(stock_spells, key=lambda n: H("spfill", n)):
        if len(chosen) >= 112:
            break
        if n not in chosen:
            chosen.append(n)
    spell_nums = sorted(chosen) + sorted(n for n in t.spells if n >= 9000)

    def spell_needs(n):
        need.spell_refs(n)
        r = t.spells[n]
        need.text(r["Learned From"])
        need.text(r["Casted By"])
        for i in range(10):
            if nz(r["Abil-%d" % i]) == 140:
                m = [nz(r["AbilVal-%d" % j]) for j in range(10) if nz(r["Abil-%d" % j]) == 141]
                if m and m[-1] > 0:
                    v = nz(r["AbilVal-%d" % i])
                    for y in ([v] if v else range(nz(r["MinBase"]), min(nz(r["MaxBase"]), nz(r["MinBase"]) + 30) + 1)):
                        need.rooms.add((m[-1], y))
        for k in list(need.spells):
            if k in t.spells and k not in need_done:
                need_done.add(k)
                rr = t.spells[k]
                need.text(rr["Learned From"])
                need.text(rr["Casted By"])

    need_done = set()

    def sp_step(n, capture=True):
        st = {"call": "PullSpellDetail", "args": [n, TB, LV_LOC], "as": "s%d" % n}
        if capture:
            st["capture"] = cap
        return st

    for n in spell_nums:
        spell_needs(n)
        g = {"bHideRecordNumbers": H("sphide", n) % 11 == 0}
        add("spell-%d" % n, [{"call": "PullSpellDetail", "args": [n, TB, LV_LOC]}], None, g, spell_read)
    for n in (0, 99999):
        add("spell-missing-%d" % n, [{"call": "PullSpellDetail", "args": [n, TB, LV_LOC]}], None, None, spell_read)
    add("spell-startup", [{"call": "PullSpellDetail", "args": [1, TB, LV_LOC]}], None, {"bStartup": True}, spell_read)
    add("spell-old-nmr", [{"call": "PullSpellDetail", "args": [9007, TB, LV_LOC]}], None, {"nNMRVer": 1.6}, spell_read)
    add("spell-nmr-17", [{"call": "PullSpellDetail", "args": [9007, TB, LV_LOC]}], None, {"nNMRVer": 1.75}, spell_read)
    add("spell-kai-off", [{"call": "PullSpellDetail", "args": [9009, TB, LV_LOC]}], None, {"bDisableKaiAutolearn": True}, spell_read)
    add("spell-clear", [{"call": "PullSpellDetail", "args": [1, TB, LV_LOC]}],
        {"lvLoc": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}]}}, None, spell_read)

    # ---- spells: combat option on (no character)
    combat = [n for n in spell_nums if H("spcombat", n) % 3 == 0]
    for k, n in enumerate(combat):
        f = {"chkSpellOptions": {"$array": {"0": {"Value": 1}, "2": {"Value": k % 2}}},
             "txtSpellOptions": {"$array": {"0": {"Text": str((15, 40, 75, 5)[k % 4])}}}}
        add("spellc-%d" % n, [{"call": "PullSpellDetail", "args": [n, TB, LV_LOC]}], f, None, spell_read)
    # the combat option with a zero value is off
    add("spellc-zero", [{"call": "PullSpellDetail", "args": [1, TB, LV_LOC]}],
        {"chkSpellOptions": {"$array": {"0": {"Value": 1}}}, "txtSpellOptions": {"$array": {"0": {"Text": "0"}}}}, None, spell_read)

    # ---- spells: loaded characters (RefreshAll cut from frmMain.frm, taken from the pipeline scenario)
    pipe = json.load(open(PIPELINE))
    pcases = {c["id"]: c for c in pipe["cases"]}
    char_items, char_spells = set(), set()
    for pid, tag in (("grid-c4-r7-l15", "cleric"), ("grid-c12-r5-l15", "mage")):
        pc = pcases[pid]
        base_forms = json.loads(json.dumps(pc["forms"]["frmMain"]))
        base_globals = json.loads(json.dumps(pc["globals"]))
        cls = base_forms["cmbGlobalClass"]["$array"]["0"]["ItemData"]["1"]
        lvl = base_forms["txtGlobalLevel"]["$array"]["0"]["Text"]
        base_forms["txtGlobalLevel"]["$array"]["1"] = {"Text": lvl}
        base_forms["chkGlobalFilter"] = {"Value": 1}
        char_items |= {x for x in base_globals["nEquippedItem"] if x}
        for e in base_forms.get("cmbEquip", {}).get("$array", {}).values():
            char_items |= set(e.get("ItemData", []))
        for e in base_forms.get("cmbCharBless", {}).get("$array", {}).values():
            char_spells |= {x for x in e.get("ItemData", []) if x}
        char_spells |= {base_globals[k] for k in ("nGlobalAttackSpellNum", "nGlobalAttackHealSpellNum") if base_globals.get(k)}
        mt, ml = t.classes[cls]["MageryType"], t.classes[cls]["MageryLVL"]
        own = [n for n in stock_spells if nz(t.spells[n]["Magery"]) == mt and nz(t.spells[n]["MageryLVL"]) <= ml and mt
               and t.spells[n]["Learnable"]]
        own = pick("chown" + tag, own, 30)
        others = pick("chother" + tag, [n for n in stock_spells if n not in own and any(nz(t.spells[n]["Abil-%d" % i]) for i in range(10))], 10)
        picks = sorted(own + others) + [9001, 9007, 9006]
        for n in picks:
            spell_needs(n)
        for opt, (combat_on, anti, mr) in (("plain", (0, 0, 0)), ("combat", (1, 1, 35))):
            f = json.loads(json.dumps(base_forms))
            f["chkSpellOptions"] = {"$array": {"0": {"Value": combat_on}, "2": {"Value": anti}}}
            f["txtSpellOptions"] = {"$array": {"0": {"Text": str(mr)}}}
            steps = [{"call": "RefreshAll"}] + [sp_step(n) for n in picks]
            add("char-%s-%s" % (tag, opt), steps, f, base_globals, {})
        # a character whose level box (1) is empty: the spell pane ignores the character
        f = json.loads(json.dumps(base_forms))
        f["txtGlobalLevel"]["$array"]["1"] = {"Text": "0"}
        steps = [{"call": "RefreshAll"}] + [sp_step(n) for n in picks[:12]]
        add("char-%s-nolevel" % tag, steps, f, base_globals, {})

    scn = finish(t, need, cases, char_items, char_spells)
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "bytes", os.path.getsize(OUT), file=sys.stderr)


def finish(t, need, cases, char_items, char_spells):
    for tb in list(need.tbs):
        pass
    data = build_data(t, need, char_items, char_spells)
    lair_ids = sorted(need.lairs)
    data["lairs"] = {g: lair_row(t, g) for g in lair_ids}
    pipe = json.load(open(PIPELINE))
    forms = json.loads(json.dumps(pipe["forms"]))
    forms["frmMain"].update({
        "txtDetail": {"Text": ""}, "lvDetail": {"$recordlv": []}, "lvLoc": {"$recordlv": []},
        "lvAssigned": {"$recordlv": []}, "lvMap": {"$recordlv": []},
        "lblCharmMod": {"Caption": ""}, "chkGlobalFilter": {"Value": 0},
    })
    return {
        "family": "misc_detail",
        "description": "Detail panes (FAM-20, stock): PullClassDetail, PullRaceDetail, PullShopDetail, "
                       "AddRoomNPCCommandRefs and PullSpellDetail over the stock database.",
        "notes": [
            "Each case calls one procedure with recording controls: txtDetail (the text box), lvDetail (the shop list), "
            "lvAssigned and lvLoc (location lists), lvMap (the map list); output 'text' is the text box, rows are "
            "[Text, Tag, cells] per row as in the locations family.",
            "Rows numbered 9001 and up are invented (db_extra) to reach NPC greet trees with teleports, which no stock "
            "command has.",
            "GetLairInfo answers from data.lairs (invented numbers on the real group indexes), as in the locations family.",
            "data holds every record the cases can reach as {fields, rows}; the Rust test builds its World from it.",
        ],
        "database": True,
        "shims": ["lairs"],
        "tolerance": {"default": 0},
        "frm": pipe["frm"],
        "db_extra": {"TBInfo": EXTRA_TB, "Monsters": EXTRA_MON, "Shops": EXTRA_SHOPS, "Spells": EXTRA_SPELLS},
        "globals": {**pipe["globals"], "nNMRVer": 1.83, "bHideRecordNumbers": False, "nGlobalAttackTypeMME": 0},
        "forms": forms,
        "data": data,
        "cases": cases,
    }


if __name__ == "__main__":
    main()

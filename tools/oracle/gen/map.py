#!/usr/bin/env python3
"""Writes fixtures/scenarios/map.json (FEAT-MAP, card M-001).

    python3 -I tools/oracle/gen/map.py [DB.sqlite]

The oracle runs MME's own map procedures, cut from frmMain.frm and frmMap.frm, against the stock database:

- `lay-*`    MapStartMapping, MapActivateCell and MapMapExits of frmMain (the fixed 30x23 grid, `m-` ids) and of frmMap
             (`w<size>-` ids) from start rooms in towns, dungeons, a maze, rooms with up/down, hidden, restricted and
             map-change exits, long corridors that leave the grid, and invented rooms (map 9001 and up) for the
             collisions: duplicates, overwrite passes, rooms missing from the data. The drawing procedure is replaced
             by a recorder: every MapDrawOnRoom call (cell, shape, size, QBColor), the cells' rooms, states and fills.
- `go-*`     frmMain MapGoDirection and frmMap cmdMove_Click: where a direction leads.
- `leads-*`  cmdMapLeadsHere_Click: the rooms, spells and monsters that lead to a room.
- `data.presets` is the default preset list read from MME's LoadPresets by pattern.

The Rooms table does not fit a fixture, so the oracle database holds the rooms the cases reach: a first pass runs every
layout case against the whole table and notes the cells' rooms. The same rows sit once under "data" for the Rust test.
Deterministic: selection by sha256 of fixed tags; the clock and Python's random are never used.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "map.json")
sys.path.insert(0, os.path.join(ROOT, "tools", "oracle"))

DIRS = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"]
ROOM_F = ["Map Number", "Room Number", "Name", "CMD", "NPC", "Lair", "Shop", "Spell"] + DIRS


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


# ---------------------------------------------------------------------------------------- invented rows
def room(m, r, name="", cmd=0, npc=0, lair="", shop=0, spell=0, **ex):
    d = {"Map Number": m, "Room Number": r, "Name": name, "Light": 0, "Shop": shop, "NPC": npc, "CMD": cmd,
         "Spell": spell, "Lair": lair, "Delay": 0, "Placed": ""}
    d.update({k: "0" for k in DIRS})
    d.update(ex)
    return d


INV_ROOMS = [
    # a hub with every kind of marker and one exit of each kind of line
    room(9001, 1, "Hub", cmd=20301, npc=9001, lair="(Max 2): 53,[10-10-10]", shop=3, spell=5,
         N="9001/2", E="9001/3 (Door [any picklocks/strength])", S="9001/4 (Hidden/Searchable)",
         W="9001/5 (Class: 4 OK, 0 NO)", NE="9001/6 (Toll: 5)", NW="9001/99", SE="9002/1", SW="Action#1 [on the S exit]: pull lever",
         U="9001/7", D="9001/7"),
    # N of the hub: its E exit points at the cell the hub's NE exit chose for room 6, and its SE exit at room 3 (a second cell for it)
    room(9001, 2, "North", N="9001/8", E="9001/6", SE="9001/3", S="9001/1", U="9001/7"),
    # E of the hub: a W exit that tries to put another room on the hub's cell, an NW exit that tries to replace room 2's cell
    room(9001, 3, "East", W="9001/9", NW="9001/10", E="9001/11 (Text: say hello)", S="9001/12 (Trap, 30 damage)"),
    room(9001, 4, "South Hidden", N="9001/1", S="9001/13 (Timed: 0*5 minutes)", E="9001/14 (Level: 0 to 3)"),
    room(9001, 5, "West Class", E="9001/1", W="9001/15 (Race: 3 OK, 4 NO)", NW="9001/16 (Alignment: Saint to Neutral)"),
    room(9001, 6, "Toll Booth", SW="9001/1", N="9001/17 (Key: 7 [or any picklocks])", E="9001/18 (Item: 5)"),
    room(9001, 7, "Stair", D="9001/1", U="9001/8", N="9001/19 (Abilities)", S="9001/20 (Spell Trap: 3)"),
    room(9001, 8, "Upper", D="9001/7", S="9001/2"),
    room(9001, 9, "Usurper West"),
    room(9001, 10, "Usurper North"),
    room(9001, 11, "East Text", W="9001/3"),
    room(9001, 12, "East Trap", N="9001/3", E="9001/21 (Max 3 times)", W="9001/22 (Delay: 3)"),
    room(9001, 13, "Timed", N="9001/4"),
    room(9001, 14, "Level", W="9001/4"),
    room(9001, 15, "Race", E="9001/5"),
    room(9001, 16, "Alignment", SE="9001/5"),
    room(9001, 17, "Key", S="9001/6"),
    room(9001, 18, "Item", W="9001/6"),
    room(9001, 19, "Ability", S="9001/7", E="9001/23 (Ticket/Item: 7)"),
    room(9001, 20, "Spell Trap", N="9001/7", W="9001/24 (Blocked)"),
    room(9001, 21, "Max"),
    room(9001, 22, "Delay"),
    room(9001, 23, "Ticket"),
    room(9001, 24, "Blocked"),
    # a map change both ways
    room(9002, 1, "Annex", cmd=1, NW="9001/1", E="9002/2"),
    room(9002, 2, "Annex Two", W="9002/1", NPC=0, npc=9002),
]
# long corridors: east-west (60 rooms) and a diagonal one, to leave every grid
for i in range(1, 61):
    INV_ROOMS.append(room(9003, i, "Corridor %d" % i, E="9003/%d" % (i + 1) if i < 60 else "0", W="9003/%d" % (i - 1) if i > 1 else "0",
                          S="9003/%d (Door)" % (100 + i) if i % 10 == 0 else "0"))
for i in range(1, 41):
    INV_ROOMS.append(room(9004, i, "Diagonal %d" % i, SE="9004/%d" % (i + 1) if i < 40 else "0", NW="9004/%d" % (i - 1) if i > 1 else "0",
                          NE="9004/%d" % (i + 100) if i == 20 else "0"))
for i in range(101, 121):
    INV_ROOMS.append(room(9004, i, "Diagonal Back %d" % (i - 100), NE="9004/%d" % (i + 1) if i < 120 else "0",
                          SW="9004/%d" % (i - 1) if i > 101 else "9004/20"))
# a north-south corridor that is also a two-way link between rooms of different maps
for i in range(1, 61):
    INV_ROOMS.append(room(9005, i, "Shaft %d" % i, S="9005/%d" % (i + 1) if i < 60 else "0", N="9005/%d" % (i - 1) if i > 1 else "0"))
# leads-here: rooms whose exits or commands reach the hub
INV_ROOMS += [
    room(9006, 1, "Portal", cmd=20302, E="9001/1"),
    room(9006, 2, "Portal Far", cmd=20303),
    room(9006, 3, "Portal None", cmd=20304),
]
INV_TB = [
    {"Number": 20301, "LinkTo": 0, "Action": "pull:message 1\n"},
    {"Number": 20302, "LinkTo": 0, "Action": "pray:teleport 1 9001\n"},
    {"Number": 20303, "LinkTo": 0, "Action": "pray:teleport 7 9001\n"},
    {"Number": 20304, "LinkTo": 0, "Action": "pray:teleport 1 9007\nsay hi\n"},
    {"Number": 20305, "LinkTo": 20306, "Action": "listen:20307\n"},
    {"Number": 20306, "LinkTo": 0, "Action": "go:random 20308\n"},
    {"Number": 20307, "LinkTo": 0, "Action": "teleport 1 9001\n"},
    {"Number": 20308, "LinkTo": 0, "Action": "teleport 3 9001\n"},
    {"Number": 20309, "LinkTo": 0, "Action": "go:20310\nlater:20311\n"},
    {"Number": 20310, "LinkTo": 20312, "Action": "say no\n"},
    {"Number": 20311, "LinkTo": 0, "Action": "say no\n"},
    {"Number": 20312, "LinkTo": 0, "Action": "teleport 3\n"},
    {"Number": 20313, "LinkTo": 0, "Action": "\x00"},
    {"Number": 20314, "LinkTo": 0, "Action": "port:0\nport:-3\n\n:20307\nbad\nport:20307\n"},
]
MON_COLS = None


def inv_monster(n, name, greet, in_game=1, summoned=""):
    return {"Number": n, "Name": name, "GreetTXT": greet, "In Game": in_game, "Summoned By": summoned}


INV_MON = [
    inv_monster(9001, "Test Guide", 20305, 1, "Room 9001/1, Room 9001/2, Room 9002/1"),
    inv_monster(9002, "Test Mute", 0),
    inv_monster(9003, "Test Greeter", 20309, 1, "Room 9006/1, Room 9001/1, Room 9006/2"),
    inv_monster(9004, "Test Hidden Guide", 20305, 0, "room 9001/5 ROOM 9001/6"),
    inv_monster(9005, "Test Blank", 20313),
    inv_monster(9006, "Test Walker", 20314, 1, "Room 7/7"),
    inv_monster(9007, "Test Lost", 29999),
]


def inv_spell(n, name, a0, v0, a1=0, v1=0, minb=0, maxb=0, learnable=0, casted="a caster"):
    d = {"Number": n, "Name": name, "Short": "t%d" % n, "Learnable": learnable, "Learned From": "", "Casted By": casted,
         "Classes": ""}
    for c in ("ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists", "Targets", "Dur", "AttType",
              "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc"):
        d[c] = 0
    d["MinBase"], d["MaxBase"] = minb, maxb
    for i in range(10):
        d["Abil-%d" % i] = 0
        d["AbilVal-%d" % i] = 0
    d["Abil-0"], d["AbilVal-0"], d["Abil-1"], d["AbilVal-1"] = a0, v0, a1, v1
    return d


INV_SPELLS = [
    inv_spell(9001, "Test Port One", 140, 1, 141, 9001),          # teleport to room 1 of map 9001
    inv_spell(9002, "Test Port Range", 140, 0, 141, 9001, minb=1, maxb=5),          # rooms 1 to 5
    inv_spell(9003, "Test Port Other Map", 140, 1, 141, 9002),
    inv_spell(9004, "Test Port No Map", 140, 1),
    inv_spell(9005, "Test Text Block", 148, 20307),
    inv_spell(9006, "Test Text Range", 148, 0, minb=20307, maxb=20308),
    inv_spell(9007, "Test Text Single", 148, 0, minb=20307, maxb=20307),
    inv_spell(9008, "Test Port Unlearnable", 140, 1, 141, 9001, casted="", learnable=0),
    inv_spell(9009, "Test Port Learnable", 140, 1, 141, 9001, casted="", learnable=1),
]


# ---------------------------------------------------------------------------------------- the oracle's cuts
MAP_DECLS = ["nMapLastFind", "nMapLastCellIndex", "bMapStillMapping", "nMapCellLength", "nMapCellDraw", "nMapCellDrawAdj",
             "nMapCellGapLength", "nMapCellGapDraw", "nMapCellGapDrawAdj", "sMapSECorner", "nMapRowLength", "nMapStartRoom",
             "nMapStartMap", "nMapCenterCell", "nMapLastRoom", "nMapLastMap", "nMap_iGoBack", "bMapSwapButtons", "nOverwritePasses",
             "StopBuild", "bUseZoomMap", "CellRoom", "UnchartedCells", "ALT_CellRoom", "ALT_UnchartedCells"]
YELLOW = "Call MapDrawOnRoom(lblRoomCell(Cell), drSquare, 4, BrightYellow)"
CTRL_STUBS = ["ConvertScale", "SetFrameCaption", "lblRoomCell_MouseDown", "UN_SubclassFormSizeRestriction"]
PROGRESS = {"Load frmProgressBar": "", "Unload frmProgressBar": "", "Set frmProgressBar.objFormOwner = Me": "",
            "frmProgressBar.Show vbModeless, Me": "", "Call frmResults.SetupResultsWindow(False, Me, nMapStartMap)": "",
            "frmResults.Show vbModeless, IIf(bNoAlwaysOnTop, Nothing, Me)": "", "Unload frmResults": ""}

# `Tag = number` between a Variant holding text and a number is false in VB6 (a number sorts below text); the oracle
# raises a type mismatch for it, so the comparison is made on text, which agrees for every tag these lists hold
TAGS = {
    'If Left(oLI.Text, Len("Spell:")) = "Spell:" And oLI.Tag = tabSpells.Fields("Number") Then GoTo skip_spell:':
        'If Left(oLI.Text, Len("Spell:")) = "Spell:" And CStr(oLI.Tag) = CStr(tabSpells.Fields("Number")) Then GoTo skip_spell:',
    'If Left(oLI.Text, Len("Monster:")) = "Monster:" And oLI.Tag = tabMonsters.Fields("Number") Then GoTo skip_monster:':
        'If Left(oLI.Text, Len("Monster:")) = "Monster:" And CStr(oLI.Tag) = CStr(tabMonsters.Fields("Number")) Then GoTo skip_monster:',
}

FRM_MAIN = {
    "file": "frmMain.frm",
    "decls": MAP_DECLS + ["bMapCancelFind", "bNoAlwaysOnTop"],
    "procs": ["MapStartMapping", "MapActivateCell", "MapMapExits", "MapGoDirection", "cmdMapLeadsHere_Click"],
    "enums": ["EnumDrawRoom"],
    "me_is_form": True, "tooltip_recorder": True, "map_recorder": True,
    "stub_procs": CTRL_STUBS,
    "stub_false": ["FormIsLoaded"],
    "record_procs": ["MapRecGo"],
    "subs": {
        "MapActivateCell": {"DrawWidth = 4": ""},
        "MapStartMapping": {"picMap.Cls": "Call MapRecClear"},
        "MapMapExits": {YELLOW: YELLOW + "\nCall MapRecWas(Cell, ALT_CellRoom(Cell, 1), ALT_CellRoom(Cell, 2))"},
        "MapGoDirection": {"Call MapStartMapping(RoomExits.Map, RoomExits.Room)": "Call MapRecGo(RoomExits.Map, RoomExits.Room)"},
    },
    "subs_all": {"cmdMapLeadsHere_Click": {**PROGRESS, **TAGS}},
}
FRM_WIN = {
    "file": "frmMap.frm",
    "decls": MAP_DECLS,
    "procs": ["MapStartMapping", "MapActivateCell", "MapMapExits", "ResizeMap", "cmdMove_Click"],
    "me_is_form": True, "map_recorder": True,
    "stub_procs": CTRL_STUBS,
    "record_procs": ["MapRecGo"],
    "cuts": {"ResizeMap": {"cut": "If bDPIAwareMode Then tWindowSize.DPI = GetDpiForWindow_Proxy(Me.hWnd)"}},
    "subs": {
        "ResizeMap": {"Call UN_SubclassFormSizeRestriction(Me)": ""},
        "MapActivateCell": {"DrawWidth = 4": ""},
        "MapStartMapping": {"picMap.Cls": "Call MapRecClear", "picZoomMap.Cls": ""},
        "MapMapExits": {YELLOW: YELLOW + "\nCall MapRecWas(Cell, ALT_CellRoom(Cell, 1), ALT_CellRoom(Cell, 2))"},
        "cmdMove_Click": {"Call MapStartMapping(RoomExit.Map, RoomExit.Room)": "Call MapRecGo(RoomExit.Map, RoomExit.Room)"},
    },
    "rename": {"prefix": "W", "names": MAP_DECLS + ["MapStartMapping", "MapActivateCell", "MapMapExits", "ResizeMap", "cmdMove_Click"]},
}

SIZES = ["20x20", "30x30", "30x50", "50x50", "zoom21", "zoom28"]


def opts_forms(name, size, opts):
    chk = {str(i): {"Value": 0} for i in range(13)}
    for k, v in opts.items():
        if k != "mark":
            chk[str(k)] = {"Value": v}
    mark = opts.get("mark", 0)
    return {"frmMain": {
        "name": name, "cmbMapSize": {"ListIndex": size}, "chkMapOptions": {"$array": chk},
        "optAlsoMark": {"$array": {str(i): {"Value": i == mark} for i in range(4)}}}}


# option numbers (chkMapOptions index)
FOLLOW, NOHIDDEN, NOLAIR, NONPC, NOCMD, DUPES, OVERWRITE, NORESTRICT = 0, 1, 2, 3, 4, 9, 10, 12


def layout_case(cid, surface, start, opts, center=None):
    if surface == "main":
        args = [start[0], start[1]] + ([center] if center else [])
        return {"id": cid, "forms": opts_forms("frmMain", 0, opts), "surface": "main", "steps": [{"call": "MapStartMapping", "args": args}],
                "read": {"m": "%map."}}
    k = SIZES.index(surface)
    args = [start[0], start[1]] + ([center] if center else [])
    return {"id": cid, "forms": opts_forms("frmMap", k, opts), "surface": surface, "steps": [{"call": "WMapStartMapping", "args": args}],
            "read": {"m": "%map.W"}}


def load_tables(con):
    con.row_factory = sqlite3.Row

    def rows(name):
        return [dict(r) for r in con.execute('select * from "%s" order by rowid' % name)]
    return rows


def nz(v):
    return 0 if v is None else v


def feature_rooms(rooms):
    def has(r, pat):
        return any(re.search(pat, str(r[d])) for d in DIRS)

    def map_change(r):
        for d in DIRS:
            m = re.match(r"(\d+)/", str(r[d]))
            if m and int(m.group(1)) != r["Map Number"]:
                return True
        return False
    return {
        "hidden": [k for k, r in rooms.items() if has(r, r"\(Hidd")],
        "class": [k for k, r in rooms.items() if has(r, r"\(Clas|\(Race|\(Leve")],
        "align": [k for k, r in rooms.items() if has(r, r"\(Alig")],
        "text": [k for k, r in rooms.items() if has(r, r"\(Text")],
        "trap": [k for k, r in rooms.items() if has(r, r"\(Trap")],
        "door": [k for k, r in rooms.items() if has(r, r"\(Door")],
        "mapchange": [k for k, r in rooms.items() if map_change(r)],
        "updown": [k for k, r in rooms.items() if str(r["U"]) != "0" and str(r["D"]) != "0" and not str(r["U"]).startswith("Action")],
        "up": [k for k, r in rooms.items() if str(r["U"]) not in ("0",) and str(r["D"]) == "0" and not str(r["U"]).startswith("Action")],
        "lair": [k for k, r in rooms.items() if len(r["Lair"] or "") > 1],
        "npc": [k for k, r in rooms.items() if nz(r["NPC"]) > 0],
        "shop": [k for k, r in rooms.items() if nz(r["Shop"]) > 2],
        "spell": [k for k, r in rooms.items() if nz(r["Spell"]) > 0],
        "cmd": [k for k, r in rooms.items() if nz(r["CMD"]) > 0],
        "action": [k for k, r in rooms.items() if any(str(r[d]).startswith("Action") for d in DIRS)],
    }


def build_cases(rooms):
    f = feature_rooms(rooms)
    small = lambda m: sorted(k for k in rooms if k[0] == m)
    cases = []
    A = cases.append
    town, newhaven, maze, dusty = (1, 224), (1, 2140), (1, 1917), (12, 5)
    # frmMain, the fixed grid
    A(layout_case("m-town", "main", town, {}))
    A(layout_case("m-town-follow", "main", town, {FOLLOW: 1, "mark": 1}))
    A(layout_case("m-newhaven-all", "main", newhaven, {FOLLOW: 1, NOHIDDEN: 1, DUPES: 1, OVERWRITE: 1}))
    A(layout_case("m-maze", "main", maze, {}))
    A(layout_case("m-maze-dupes", "main", maze, {DUPES: 1}))
    A(layout_case("m-maze-overwrite", "main", maze, {OVERWRITE: 1}))
    for i, k in enumerate(pick("mdungeon", [k for k in rooms if k[0] in (3, 7, 10)], 3)):
        A(layout_case("m-dungeon-%d" % i, "main", k, {FOLLOW: i % 2}))
    A(layout_case("m-hub", "main", (9001, 1), {}))
    A(layout_case("m-hub-follow", "main", (9001, 1), {FOLLOW: 1}))
    A(layout_case("m-hub-nohidden", "main", (9001, 1), {FOLLOW: 1, NOHIDDEN: 1, NORESTRICT: 1}))
    A(layout_case("m-hub-nomarks", "main", (9001, 1), {FOLLOW: 1, NOLAIR: 1, NONPC: 1, NOCMD: 1}))
    A(layout_case("m-hub-shop", "main", (9001, 1), {FOLLOW: 1, "mark": 1}))
    A(layout_case("m-hub-spell", "main", (9001, 1), {FOLLOW: 1, "mark": 2}))
    A(layout_case("m-hub-dupes", "main", (9001, 1), {FOLLOW: 1, DUPES: 1}))
    A(layout_case("m-hub-overwrite", "main", (9001, 1), {FOLLOW: 1, OVERWRITE: 1}))
    A(layout_case("m-hub-both", "main", (9001, 1), {FOLLOW: 1, DUPES: 1, OVERWRITE: 1}))
    A(layout_case("m-hub-east", "main", (9001, 3), {FOLLOW: 1, OVERWRITE: 1}))
    A(layout_case("m-hub-center", "main", (9001, 1), {FOLLOW: 1}, 100))
    A(layout_case("m-corridor", "main", (9003, 30), {}))
    A(layout_case("m-corridor-west", "main", (9003, 3), {}))
    A(layout_case("m-diagonal", "main", (9004, 20), {}))
    A(layout_case("m-shaft", "main", (9005, 30), {}))
    A(layout_case("m-annex", "main", (9002, 1), {}))
    A(layout_case("m-annex-follow", "main", (9002, 1), {FOLLOW: 1}))
    # frmMap, each size
    for size in SIZES:
        A(layout_case("w%s-hub" % size, size, (9001, 1), {FOLLOW: 1, DUPES: 1, OVERWRITE: 1}))
    for size in SIZES[:4]:
        A(layout_case("w%s-corridor" % size, size, (9003, 30), {}))
        A(layout_case("w%s-diagonal" % size, size, (9004, 20), {}))
    A(layout_case("w20x20-shaft", "20x20", (9005, 30), {}))
    A(layout_case("w30x50-shaft", "30x50", (9005, 30), {}))
    A(layout_case("wzoom21-corridor", "zoom21", (9003, 30), {}))
    A(layout_case("wzoom28-diagonal", "zoom28", (9004, 20), {}))
    A(layout_case("w20x20-hub-plain", "20x20", (9001, 1), {}))
    A(layout_case("w20x20-hub-center", "20x20", (9001, 1), {FOLLOW: 1}, 200))
    A(layout_case("w20x20-hub-badcenter", "20x20", (9001, 1), {FOLLOW: 1}, 25))
    A(layout_case("wzoom21-hub-badcenter", "zoom21", (9001, 1), {FOLLOW: 1}, 100))
    A(layout_case("w20x20-town", "20x20", town, {FOLLOW: 1}))
    A(layout_case("w30x30-newhaven", "30x30", newhaven, {NOHIDDEN: 1}))
    A(layout_case("w20x20-maze", "20x20", maze, {OVERWRITE: 1}))
    A(layout_case("w30x50-dusty", "30x50", dusty, {FOLLOW: 1}))
    A(layout_case("wzoom21-dusty", "zoom21", dusty, {"mark": 1}))
    for i, k in enumerate(pick("wfeat", f["updown"], 2) + pick("wfeat2", f["mapchange"], 2) + pick("wfeat3", f["hidden"], 1) +
                          pick("wfeat4", f["class"], 1) + pick("wfeat5", f["align"], 1) + pick("wfeat6", f["lair"], 1) +
                          pick("wfeat7", f["cmd"], 1) + pick("wfeat8", f["shop"], 1) + pick("wfeat9", f["text"], 1)):
        A(layout_case("w20x20-feat-%d" % i, "20x20", k, {FOLLOW: i % 2, NORESTRICT: i % 3 == 0, "mark": i % 3}))
    # a small map, wholly
    for i, k in enumerate(pick("smallmap", small(11), 2) + pick("smallmap2", small(13), 1)):
        A(layout_case("w30x30-small-%d" % i, "30x30", k, {FOLLOW: 1}))
    # a start room that does not exist
    A(layout_case("m-missing", "main", (9999, 1), {}))
    A(layout_case("w20x20-missing", "20x20", (1, 99999), {}))
    return cases


def go_cases(rooms):
    f = feature_rooms(rooms)
    out = []
    targets = [(9001, 1), (9001, 3), (9002, 1), (9002, 2)] + pick("goact", f["action"], 2) + pick("gomc", f["mapchange"], 2) + \
        pick("goplain", sorted(rooms), 2)
    main_steps, win_steps = [], []
    for k in targets:
        for d in DIRS:
            main_steps.append({"call": "MapGoDirection", "args": [k[0], k[1], d], "as": "g%d_%d_%s" % (k[0], k[1], d),
                               "capture": {"ev": "%evpop"}})
    main_steps.append({"call": "MapGoDirection", "args": [9999, 1, "N"], "as": "g-missing", "capture": {"ev": "%evpop"}})
    main_steps.append({"call": "MapGoDirection", "args": [9001, 1, "XX"], "as": "g-badcol", "capture": {"ev": "%evpop"}})
    out.append({"id": "go-main", "steps": main_steps})
    for k in targets:
        steps = []
        for i in range(10):
            steps.append({"call": "WcmdMove_Click", "args": [i], "as": "c%d" % i, "capture": {"ev": "%evpop"}})
        out.append({"id": "go-win-%d-%d" % k, "globals": {"WnMapStartMap": k[0], "WnMapStartRoom": k[1]}, "steps": steps})
    out.append({"id": "go-win-missing", "globals": {"WnMapStartMap": 9999, "WnMapStartRoom": 1},
                "steps": [{"call": "WcmdMove_Click", "args": [0], "as": "c0", "capture": {"ev": "%evpop"}}]})
    return out


def leads_case(cid, k, only_in_game=False):
    cap = {"rows": "%flv.frmResults.lvResults", "caption": "%f.frmResults.lblCaption.Text", "ev": "%evpop"}
    return {"id": cid, "globals": {"nMapStartMap": k[0], "nMapStartRoom": k[1], "bOnlyInGame": only_in_game},
            "forms": {"frmResults": {"lvResults": {"$recordlv": []}, "lblCaption": {"Text": ""}}},
            "steps": [{"call": "cmdMapLeadsHere_Click", "args": [], "as": "l", "capture": cap}]}


def preset_data(mme):
    text = open(os.path.join(mme, "frmMain.frm"), "rb").read().decode("latin-1")
    start = text.index("Public Sub LoadPresets")
    body = text[start:text.index("End Sub", start)]
    out = {}
    for m in re.finditer(r'^\s*Case (\d+): nMap = (\d+): nRoom = (\d+): sName = "([^"]*)"', body, re.M):
        out[int(m.group(1))] = [m.group(4), int(m.group(2)), int(m.group(3))]
    return [out[i] for i in sorted(out)]


# ---------------------------------------------------------------------------------------- scenario assembly
def scenario(cases, db_only, extra_rooms, extra_tb, extra_mon, extra_spells, data, mme_presets):
    return {
        "family": "map",
        "description": "Graphical map (stock): MapStartMapping, MapActivateCell and MapMapExits of frmMain and frmMap, "
                       "MapGoDirection, cmdMove_Click and cmdMapLeadsHere_Click.",
        "notes": [
            "Steps run MME's own procedures cut from frmMain.frm and frmMap.frm (frmMap's are renamed with the prefix W so both "
            "copies load). MapDrawOnRoom is a recorder; `m.draws` lists its calls since picMap.Cls as cell << 12 | shape << 8 | "
            "size << 4 | QBColor, `m.cells` is [cell, map, room, UnchartedCells, BackColor] for every cell that holds a room or "
            "has a colour, `m.alts` the ALT_ copies, `m.was` [cell, map, room] for each overwritten-room tooltip.",
            "ids: m-* run frmMain's grid, w<size>-* frmMap at that cmbMapSize entry. The surface is the case's `surface`; "
            "forms.frmMain.name is `frmMain` or `frmMap` (MapMapExits asks).",
            "go-main steps record where MapGoDirection would call MapStartMapping (event kind call MapRecGo) or its message box; "
            "go-win-* do the same for cmdMove_Click.",
            "leads-*: frmResults.lvResults is a recording list; the database holds the rooms, monsters, spells and textblocks "
            "in data (db_only).",
            "The Rooms table is the subset the cases reach (db_only pairs); data.rooms is the same rows for the Rust test. Rows "
            "numbered 9001 and up are invented.",
            "data.presets is the (name, map, room) list of LoadPresets' defaults, read from MME's source by pattern.",
        ],
        "database": True,
        "shims": ["lairs"],
        "db_only": db_only,
        "db_extra": {"Rooms": extra_rooms, "TBInfo": extra_tb, "Monsters": extra_mon, "Spells": extra_spells},
        "frm": [FRM_MAIN, FRM_WIN],
        "data": data,
        "globals": {"bHideRecordNumbers": False, "nNMRVer": 1.83, "bGreaterMUD": False, "bOnlyInGame": False},
        "forms": {"frmMain": {"name": "frmMain", "WindowState": 0, "lblRoomCell": {"$array": {}},
                              "chkMapOptions": {"$array": {str(i): {"Value": 0} for i in range(13)}},
                              "optAlsoMark": {"$array": {str(i): {"Value": i == 0} for i in range(4)}},
                              "lblInvenCharStat": {"$array": {"23": {"Caption": "0"}}}},
                  "frmResults": {"lvResults": {"$recordlv": []}, "lblCaption": {"Text": ""}}},
        "cases": cases,
    }


def run_scenario(scn, family="map"):
    import harness
    with tempfile.TemporaryDirectory() as d:
        with open(os.path.join(d, family + ".json"), "w") as f:
            json.dump(scn, f)
        raw = harness.build_fixture(family, sdir=d).decode()
    return json.loads("{" + raw[raw.index('"cases":['):])["cases"]


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    import harness
    con = sqlite3.connect(db)
    rows = load_tables(con)
    stock_rooms = {(r["Map Number"], r["Room Number"]): r for r in rows("Rooms")}
    all_rooms = dict(stock_rooms)
    for r in INV_ROOMS:
        all_rooms[(r["Map Number"], r["Room Number"])] = r
    tbs = {r["Number"]: r for r in rows("TBInfo")}
    mons = {r["Number"]: r for r in rows("Monsters")}
    spells = {r["Number"]: r for r in rows("Spells")}
    for t in INV_TB:
        tbs[t["Number"]] = t
    for m in INV_MON:
        mons[m["Number"]] = m
    for s in INV_SPELLS:
        spells[s["Number"]] = s

    layout = build_cases(all_rooms)
    mme = harness.mme_dir()
    presets = preset_data(mme)

    # ---- first pass: the whole Rooms table, to learn which rooms the layout cases reach
    base = scenario(layout, None, INV_ROOMS, INV_TB, INV_MON, INV_SPELLS, {"lairs": {}}, presets)
    base["db_only"] = {}
    reached = set()
    for c in run_scenario(base):
        m = c["outputs"]["m"] if "m" in c.get("outputs", {}) else None
        if m is None:
            continue
        for cell in m["cells"]:
            if cell[1]:
                reached.add((cell[1], cell[2]))
        reached.add(tuple(m["start"]))
    reached = {k for k in reached if k in all_rooms}

    # ---- leads-here targets: rooms that something teleports to or exits into, by kind
    f = feature_rooms(all_rooms)
    targets = [(9001, 1), (9001, 3), (9006, 1), (9009, 9), (9002, 1)]
    tele = {}
    for n, t in tbs.items():
        for m in re.finditer(r"teleport (\d+)(?: (\d+))?", t["Action"] or ""):
            tele.setdefault((int(m.group(2)) if m.group(2) else None, int(m.group(1))), []).append(n)
    cmd_tb = {r["CMD"] for r in all_rooms.values() if nz(r["CMD"]) > 0 and r["Map Number"] < 9000}
    greet_tb = {m["GreetTXT"] for m in mons.values() if nz(m["GreetTXT"]) > 0 and m["Number"] < 9000}
    cands = []
    for (mp, rm), tl in sorted(tele.items(), key=lambda kv: (kv[0][1], kv[0][0] or 0)):
        if mp is not None and (mp, rm) in all_rooms and any(x in cmd_tb for x in tl):
            cands.append((mp, rm))
    targets += pick("leadscmd", cands, 3)
    # spells that teleport to a fixed room
    sp_t = []
    for s in spells.values():
        if s["Number"] >= 9000:
            continue
        ab = {s["Abil-%d" % i]: s["AbilVal-%d" % i] for i in range(10)}
        if ab.get(140, 0) and ab.get(141, 0) and (ab[141], ab[140]) in all_rooms:
            sp_t.append((ab[141], ab[140]))
    targets += pick("leadsspell", sp_t, 2)
    # a room many rooms exit into
    into = {}
    for k, r in all_rooms.items():
        if k[0] >= 9000:
            continue
        for d in DIRS:
            mm = re.match(r"(\d+)/(\d+)", str(r[d]))
            if mm:
                into.setdefault((int(mm.group(1)), int(mm.group(2))), []).append(k)
    busy = [k for k, v in into.items() if k in all_rooms and len(v) >= 6]
    targets += pick("leadsbusy", busy, 2)
    targets += [(9999, 1)]
    targets = list(dict.fromkeys(targets))
    leads = [leads_case("leads-%d-%d" % k, k) for k in targets]
    leads.append(leads_case("leads-ingame-9001-1", (9001, 1), True))
    leads.append(leads_case("leads-none", (9007, 1)))
    leads.append(leads_case("leads-zero", (0, 1)))
    gos = go_cases(all_rooms)

    # leads-here first pass on the whole tables for the stock targets: which records matter
    base = scenario(leads, None, INV_ROOMS, INV_TB, INV_MON, INV_SPELLS, {"lairs": {}}, presets)
    base["db_only"] = {}
    keep_rooms, keep_mons, keep_spells = set(), set(), set()
    for k in targets:
        keep_rooms.add(k)
    # rooms that lead anywhere are found from the run's rows
    for c in run_scenario(base):
        l = c["outputs"].get("l.cap")
        for row in (l or {}).get("rows", []):
            text, tag = row[0], row[1]
            if text.startswith("Room:"):
                a, b = tag.split("/")
                keep_rooms.add((int(a), int(b)))
            elif text.startswith("Spell:"):
                keep_spells.add(int(tag))
            elif text.startswith("Monster:"):
                keep_mons.add(int(tag))
    keep_rooms = {k for k in keep_rooms if k in all_rooms}

    # ---- the subset
    subset = reached | keep_rooms | {k for k in all_rooms if k[0] >= 9000}
    # a few more rooms that have a teleporting command (but lead nowhere) and the monsters/spells that teleport at all
    for n in pick("noise", [k for k in all_rooms if nz(all_rooms[k]["CMD"]) > 0 and k[0] < 9000], 12):
        subset.add(n)
    tele_tb = {n for n, t in tbs.items() if "teleport" in (t["Action"] or "")}

    def chain(n, depth=0):
        """Textblocks a greet line, link or `random` can reach from n."""
        seen, work = set(), [n]
        while work:
            x = work.pop()
            if x in seen or x not in tbs or len(seen) > 40:
                continue
            seen.add(x)
            t = tbs[x]
            if nz(t["LinkTo"]):
                work.append(t["LinkTo"])
            for m in re.finditer(r"random (\d+)", t["Action"] or ""):
                work.append(int(m.group(1)))
        return seen

    def greet_reach(mon):
        out = set()
        g = nz(mon["GreetTXT"])
        if g <= 0 or g not in tbs:
            return out
        out |= chain(g)
        for line in (tbs[g]["Action"] or "").split("\n"):
            mm = re.match(r"\s*(\d+)", line.split(":", 1)[-1] if ":" in line else line)
            for part in re.findall(r"(?:^|:)(\d+)\s*$", line):
                out |= chain(int(part))
        return out
    for n, m in sorted(mons.items()):
        if n >= 9000 or n in keep_mons:
            continue
        if any(x in tele_tb for x in greet_reach(m)) and H("mon", n) % 4 == 0:
            keep_mons.add(n)
    for n in pick("noisemon", [n for n, m in mons.items() if n < 9000 and nz(m["GreetTXT"]) > 0], 8):
        keep_mons.add(n)
    for n, s in sorted(spells.items()):
        if n >= 9000 or n in keep_spells:
            continue
        ab = {s["Abil-%d" % i]: s["AbilVal-%d" % i] for i in range(10)}
        if (140 in ab or 148 in ab) and H("spell", n) % 6 == 0:
            keep_spells.add(n)
    keep_mons |= {m["Number"] for m in INV_MON}
    keep_spells |= {s["Number"] for s in INV_SPELLS}
    keep_tbs = {r["CMD"] for k, r in all_rooms.items() if k in subset and nz(r["CMD"]) > 0}
    for n in keep_mons:
        keep_tbs |= greet_reach(mons[n])
        g = nz(mons[n]["GreetTXT"])
        if g in tbs:
            keep_tbs.add(g)
            for line in (tbs[g]["Action"] or "").split("\n"):
                for part in re.findall(r"(?:^|:)(\d+)\s*$", line):
                    keep_tbs |= chain(int(part))
    for n in keep_spells:
        s = spells[n]
        for i in range(10):
            if s["Abil-%d" % i] == 148:
                v = s["AbilVal-%d" % i]
                for x in ([v] if v else range(nz(s["MinBase"]), nz(s["MaxBase"]) + 1)):
                    keep_tbs |= chain(x)
    for n in list(keep_tbs):
        keep_tbs |= chain(n)
    keep_tbs = {n for n in keep_tbs if n in tbs}

    stock_sub = sorted(k for k in subset if k[0] < 9000)
    db_only = {"Rooms": [[m, r] for m, r in stock_sub],
               "Monsters": sorted(n for n in keep_mons if n < 9000), "Spells": sorted(n for n in keep_spells if n < 9000),
               "TBInfo": sorted(n for n in keep_tbs if n < 20000)}

    def room_row(k):
        r = all_rooms[k]
        return [r[f] if f not in ("Name", "Lair") else (r[f] or "") for f in ROOM_F]
    data = {
        "rooms": {"fields": ROOM_F, "rows": [room_row(k) for k in sorted(subset)]},
        "monsters": {"fields": ["Number", "Name", "GreetTXT", "In Game", "Summoned By"],
                     "rows": [[n, mons[n]["Name"] or "", nz(mons[n]["GreetTXT"]), nz(mons[n]["In Game"]), mons[n]["Summoned By"] or ""]
                              for n in sorted(keep_mons)]},
        "spells": {"fields": ["Number", "Name", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists",
                              "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs",
                              "MinInc", "DurIncLVLs", "DurInc", "Abilities", "Learnable", "Learned From", "Casted By", "Classes"],
                   "rows": [[n, spells[n]["Name"] or ""] + [nz(spells[n][c]) for c in
                                                            ["ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff",
                                                             "TypeOfResists", "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap",
                                                             "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc", "DurIncLVLs", "DurInc"]] +
                            [[[nz(spells[n]["Abil-%d" % i]), nz(spells[n]["AbilVal-%d" % i])] for i in range(10)
                              if nz(spells[n]["Abil-%d" % i]) or nz(spells[n]["AbilVal-%d" % i])],
                             nz(spells[n]["Learnable"]), spells[n]["Learned From"] or "", spells[n]["Casted By"] or "", spells[n]["Classes"] or ""]
                            for n in sorted(keep_spells)]},
        "tbinfo": {"fields": ["Number", "LinkTo", "Action"],
                   "rows": [[n, nz(tbs[n]["LinkTo"]), tbs[n]["Action"] or ""] for n in sorted(keep_tbs)]},
        "presets": presets,
        "lairs": {},
    }
    cases = layout + gos + leads
    scn = scenario(cases, db_only, [r for r in INV_ROOMS], INV_TB, INV_MON, INV_SPELLS, data, presets)
    with open(OUT, "w") as fh:
        json.dump(scn, fh, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        fh.write("\n")
    print("%d cases, %d rooms (%d stock), %d monsters, %d spells, %d textblocks" % (
        len(cases), len(subset), len(stock_sub), len(keep_mons), len(keep_spells), len(keep_tbs)), file=sys.stderr)


if __name__ == "__main__":
    main()

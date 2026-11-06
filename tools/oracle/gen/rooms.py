#!/usr/bin/env python3
"""Writes fixtures/scenarios/rooms.json (FEAT-ROOMS, card P4-13).

    python3 -I tools/oracle/gen/rooms.py [DB.sqlite]

The oracle runs MME's own frmMain procedures for the Rooms tab against the stock database:

- `refs-*`, `opt-*`, `dmg-*`  MapGetRoomLoc (the room's reference list) and MapMapExits (the text of the room's
                              tooltip; its drawing and cell mapping are cut) for stock and invented rooms, with the
                              display options (hidden numbers, NPCs, lairs, commands, all exits) and data versions.
- `text-*`                    cmdMapFindText_Click: Find, then Find Next until the end of the list.
- `exits-*`                   FindRoomWithDirections: the dialog is replaced by the case's name, match mode and
                              direction buttons; the results are frmResults' list.
- `cmdrooms-*`                ShowRoomCmdRoomList.

The Rooms table does not fit a fixture, so the oracle database holds a subset (`db_only` Rooms pairs): the rooms the
cases name, their exit targets, and the rooms the finders look at. The same rows sit once under "data" for the Rust
test. GetLairInfo is answered by the lair shim from data.lairs (the real group indexes with fixed invented numbers).
Rows numbered 9001 and up (rooms of map 9001, a monster, textblocks) are invented to reach branches no stock room has.
Deterministic: selection by sha256 of fixed tags; the clock and Python's random are never used.
"""
import glob
import hashlib
import importlib.util
import json
import os
import re
import sqlite3
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "rooms.json")

spec = importlib.util.spec_from_file_location("misc_detail_gen", os.path.join(HERE, "misc_detail.py"))
md = importlib.util.module_from_spec(spec)
spec.loader.exec_module(md)

DIRS = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"]
ROOM_F = ["Map Number", "Room Number", "Name", "Light", "Shop", "NPC", "CMD", "Spell", "Lair", "Delay", "Placed"] + DIRS


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


def room_mask(room):
    """The exit mask FindRoomWithDirections builds for a room (a label for choosing the limit case)."""
    mask = 0
    for i, d in enumerate(DIRS):
        v = str(room[d])
        if v.startswith("Action") or v == "0":
            continue
        m = re.match(r"^(\d+)/(\d+)(?: (.*))?$", v)
        if not m:
            mask |= 1 << i
            continue
        et = m.group(3) or "0"
        kind = 0
        if len(et) > 2:
            kind = {"(Hidd": 6, "(Text": 10, "Actio": 12, "(Time": 16}.get(et[:5], 0)
        if int(m.group(1)) != room["Map Number"]:
            kind = 8
        if kind in (6, 10, 12, 16):
            continue
        mask |= 1 << i
    return mask


def exit_kind(room, d):
    """The exit-text kind MME's Select Case on Left(ExitType, 5) names (a label for choosing sample rooms)."""
    v = str(room[d])
    if v == "0":
        return None
    if v.startswith("Action"):
        return "action"
    m = re.match(r"^(\d+)/\d+(?: \((\w+))?", v)
    if not m:
        return "other"
    if int(m.group(1)) != room["Map Number"]:
        return "mapchange"
    return (m.group(2) or "plain").lower()


# ---------------------------------------------------------------------------------------- invented rows
# Rooms of map 9001: a hall with an NPC whose greet text teleports, a toll booth, odd exits, empty rooms.
INV_ROOMS = [
    {"Map Number": 9001, "Room Number": 1, "Name": "Test Hall", "Light": -120, "Shop": 3, "NPC": 9001, "CMD": 20201,
     "Spell": 22, "Lair": "(Max 3): 53,54,55,[10-10-15]", "Delay": 12, "Placed": "5,0,9999,12,",
     "N": "9001/2", "S": "9001/3 (Toll: 5)", "E": "9001/4 (Door [any picklocks/strength])", "W": "9001/5 (Toll: 150)",
     "NE": "9001/6 (Toll: 12345)", "NW": "9001/7 (Toll: 99)", "SE": "9001/8 (Toll: 10000)", "SW": "9001/9 (Toll: x)",
     "U": "9002/1", "D": "9001/10 (Hidden/Searchable)"},
    {"Map Number": 9001, "Room Number": 2, "Name": "Test Odd Exits", "Light": 150, "Shop": 1, "NPC": 0, "CMD": 0, "Spell": 0,
     "Lair": "(Max 12): 53,[10-10-10]", "Delay": 0, "Placed": "7,",
     "N": "9001/1 (Class: 0 OK, 0 NO)", "S": "9001/1 (Class: 4 OK, 5 NO)", "E": "9001/1 (Class: 0 OK, 5 NO)",
     "W": "9001/1 (Class: 4 OK, 0 NO)", "NE": "9001/1 (Race: 0 OK, 5 NO)", "NW": "9001/1 (Race: 3 OK, 4 NO)",
     "SE": "9001/1 (Race: 0 OK, 0 NO)", "SW": "9001/1 (Cast: pre-0, post-0)",
     "U": "9001/1 (Spell Trap: 0)", "D": "9001/1 (Item: 5)"},
    {"Map Number": 9001, "Room Number": 3, "Name": "", "Light": 0, "Shop": 2, "NPC": 0, "CMD": 20202, "Spell": 0,
     "Lair": "", "Delay": 5, "Placed": "",
     "N": "Action#1 [on the S exit of this room]: pull lever, push lever", "S": "0", "E": "0", "W": "9001/1 (Timed: 0*5 minutes)",
     "NE": "9001/1 (Text: say hello, go hello)", "NW": "Action [on the E exit of room 9001/2]: push stud (Item: 7)",
     "SE": "9001/1 (Alignment: Saint to Neutral)", "SW": "9001/1 (Level: 0 to 3)", "U": "9001/1 (Ticket/Item: 7)",
     "D": "9001/1 (Trap, 30 damage)"},
    {"Map Number": 9001, "Room Number": 4, "Name": "Test Pit", "Light": -300, "Shop": 0, "NPC": 9002, "CMD": 0, "Spell": 0,
     "Lair": "(Max 2): 9001,[99-99-99]", "Delay": 3, "Placed": "",
     "N": "0", "S": "0", "E": "0", "W": "0", "NE": "0", "NW": "0", "SE": "0", "SW": "0", "U": "0", "D": "0"},
    {"Map Number": 9002, "Room Number": 1, "Name": "Test Annex", "Light": 0, "Shop": 0, "NPC": 0, "CMD": 20203, "Spell": 0,
     "Lair": "", "Delay": 0, "Placed": "",
     "N": "9001/1", "S": "9002/2 (Key: 7 [or any picklocks])", "E": "9002/2 (Max 3 times)", "W": "9002/2 (Blocked)",
     "NE": "9002/2 (Delay: 3)", "NW": "9002/2 (Abilities)", "SE": "0", "SW": "0", "U": "0", "D": "0"},
    {"Map Number": 9002, "Room Number": 2, "Name": "Test Annex Two", "Light": 0, "Shop": 0, "NPC": 0, "CMD": 0, "Spell": 0,
     "Lair": "", "Delay": 0, "Placed": "",
     "N": "9002/1", "S": "0", "E": "0", "W": "0", "NE": "0", "NW": "0", "SE": "0", "SW": "0", "U": "0", "D": "0"},
]
# (rooms 9001/5..10 are not in the table: exits to unknown rooms read "m/r")
INV_TB = [
    # room commands: teleports with and without a map, a duplicate, a no-colon line, other commands
    {"Number": 20201, "LinkTo": 0, "Action": "knock:teleport 334 1\nopen|enter*:teleport 335\nknock:teleport 334 1\n"
                                             "say hi:cast 12\npry:teleport 99999 2\nplain line teleport 3 4\nbad:teleport x\n"},
    {"Number": 20202, "LinkTo": 0, "Action": "go:message 1\r\nrun:teleport 9002 1\r\n"},
    {"Number": 20203, "LinkTo": 0, "Action": "dig:teleport 9001 3\n"},
]
INV_MON = [{"Number": 9001, "Name": "Test Guide", "GreetTXT": 20101}, {"Number": 9002, "Name": "Test Mute", "GreetTXT": 0}]
INV_TB += [t for t in md.EXTRA_TB if t["Number"] in range(20101, 20109)]

# ---------------------------------------------------------------------------------------- the database
MAX_PICK = 2


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    con = sqlite3.connect(db)
    con.row_factory = sqlite3.Row
    t = md.Tables(con)
    # the invented rows join the tables the shared reader builds
    for tb in INV_TB:
        t.tb[tb["Number"]] = {"Number": tb["Number"], "LinkTo": tb["LinkTo"], "Action": tb["Action"]}
    for m in INV_MON:
        t.monsters[m["Number"]] = {**{k: None for k in next(iter(t.monsters.values())).keys()}, **m}

    full = [dict(r) for r in con.execute("select * from Rooms order by \"Map Number\", \"Room Number\"")]
    full_by = {(r["Map Number"], r["Room Number"]): r for r in full}
    for r in INV_ROOMS:
        full_by[(r["Map Number"], r["Room Number"])] = r
    for r in full:
        for k in ("Lair", "Placed", "Name"):
            r[k] = r[k] or ""
    inv = {(r["Map Number"], r["Room Number"]) for r in INV_ROOMS}

    # ---- sample rooms by feature
    def where(pred):
        return [k for k, r in sorted(full_by.items()) if k not in inv and pred(r)]

    groups = {}
    groups["npc"] = pick("npc", where(lambda r: r["NPC"] > 0), 5) + [(9001, 1), (9001, 4)]
    groups["cmd"] = pick("cmd", where(lambda r: r["CMD"] > 0), 4) + pick(
        "cmdtele", where(lambda r: r["CMD"] > 0 and "teleport " in (t.tb.get(r["CMD"], {}).get("Action") or "")), 4
    ) + [(9001, 1), (9001, 3), (9002, 1)]
    groups["shop-spell-placed"] = (pick("shop", where(lambda r: r["Shop"] > 2), 3) + pick("shop12", where(lambda r: r["Shop"] in (1, 2)), 2)
                                   + pick("rspell", where(lambda r: r["Spell"] > 0), 3)
                                   + pick("placed", where(lambda r: len(r["Placed"]) > 1), 4))
    groups["lair"] = pick("lair", where(lambda r: len(r["Lair"]) > 1), 9) + [(9001, 2), (9001, 4)]
    groups["light"] = [k for lt in (-300, -250, -200, -175, -150, -100, -50, 100, 175)
                       for k in pick("light%d" % lt, where(lambda r, lt=lt: r["Light"] == lt), 1)] + pick(
        "dark", where(lambda r: r["Light"] == 0), 1)
    kinds = ["plain", "door", "hidden", "key", "item", "toll", "trap", "text", "action", "class", "race", "level", "timed",
             "ticket", "alignment", "ability", "cast", "spell", "mapchange"]
    for kind in kinds:
        groups["exit-" + kind] = pick("exit" + kind, where(lambda r, kind=kind: any(exit_kind(r, d) == kind for d in DIRS)), MAX_PICK)
    groups["exit-invented"] = [(9001, 1), (9001, 2), (9001, 3), (9002, 1), (9002, 2)]

    # ---- finders: text queries and the room-find dialog
    text_queries = [("tomb", ["tomb"]), ("TOMB", ["tomb"]), ("xyzzy plugh", []), ("a", []), ("dirt", [])]
    ordered = sorted(full_by)
    text_rooms = set()
    for q, _ in text_queries:
        hits = [k for k in ordered if q.lower() in (full_by[k]["Name"] or "").lower()]
        text_rooms |= set(hits[:7])
    # the exits finder: name, exact, direction mask. The subset keeps up to FINDER_CAP rooms per name
    finders = [("Stone Tunnel", False, ["N", "S"]), ("Stone Tunnel", True, ["E", "W"]), ("Stone Tunnel", False, ["N", "S", "E", "W"]),
               ("Dirt Path", False, ["N", "S"]), ("Dirt Path", True, ["E", "W"]), ("tomb", False, ["N"]),
               ("Crystal Lake", False, ["N", "S", "E", "W"]), ("Dragon's Teeth Hills", True, ["N", "NE", "E"]),
               ("Gouged Tunnel", False, ["N", "W"])]
    finder_rooms = set()
    for name, exact, ds in finders:
        match = [k for k in ordered if (full_by[k]["Name"] == name if exact else name.lower() in (full_by[k]["Name"] or "").lower())]
        finder_rooms |= set(match[:40]) | set(pick("fr-" + name + ",".join(ds), match, 60))
    # a finder that must hit the 101-room limit: enough rooms named "Stone Tunnel" with exactly N and S exits
    stone = [k for k in ordered if "tunnel" in full_by[k]["Name"].lower()]
    by_mask = {}
    for k in stone:
        by_mask.setdefault(room_mask(full_by[k]), []).append(k)
    big_mask = max(by_mask, key=lambda m: (len(by_mask[m]), m))
    finder_rooms |= set(by_mask[big_mask][:125]) | set(stone[:30])
    big_dirs = [d for i, d in enumerate(DIRS) if big_mask >> i & 1]

    # ---- the subset the oracle database and the Rust data hold
    chosen = set()
    for g in groups.values():
        chosen |= set(g)

    def targets(k):
        r = full_by[k]
        out = set()
        for d in DIRS:
            m = re.search(r"(\d+)/(\d+)", str(r[d]))
            if m:
                out.add((int(m.group(1)), int(m.group(2))))
        return out

    subset = set(chosen) | text_rooms | finder_rooms
    for k in list(chosen):
        subset |= {x for x in targets(k) if x in full_by}
    # teleport targets of the chosen rooms' commands and NPC greets, and the room commands' own rooms
    need = md.Need(t)
    cases = []
    stock_subset = {k for k in subset if k not in inv}
    subset |= inv

    # ---- lair shim table for the group indexes in the subset
    lair_groups = set()
    for k in subset:
        lr = full_by[k]["Lair"]
        m = re.search(r"\[(\d+-\d+-\d+)-(\d+)\]\s*$", lr or "")
        if m:
            lair_groups.add(m.group(1))
    lair_table = {}
    for g in sorted(lair_groups):
        r = t.lairs.get(g)
        if not r or H("lairskip", g) % 7 == 0:
            continue                     # a group the table does not know: the shim answers an empty record
        d = {"sMobList": r["MobList"] or "", "nMobs": md.nz(r["Mobs"]), "nAvgExp": md.nz(r["AvgExp"]), "nAvgHP": md.nz(r["AvgHP"])}
        if H("dcr", g) % 3:
            d["nAvgDmgClearRound"] = H("dcrv", g) % 60 + [0, 0.25, 0.5, 0.75][H("dcrf", g) % 4]
            if H("dcl", g) % 3:
                d["nAvgDmgClear"] = H("dclv", g) % 900 + [0, 0.5][H("dclf", g) % 2]
        lair_table[g] = {k: v for k, v in d.items() if v != 0 and v != ""}

    # ---- cases
    def add(cid, steps, forms=None, globals_=None, loaded=None, inputbox=None, read=None):
        c = {"id": cid, "steps": steps}
        if forms:
            c["forms"] = {"frmMain": forms} if "frmResults" not in forms and "frmPopUpOptions" not in forms else forms
        if globals_:
            c["globals"] = globals_
        if loaded:
            c["loaded_forms"] = loaded
        if inputbox is not None:
            c["inputbox"] = inputbox
        if read is not None:
            c["read"] = read
        cases.append(c)

    CLEAR = {"call": "RoomsClearRefs"}

    def room_steps(keys, tag):
        steps = []
        for i, (m, r) in enumerate(keys):
            steps.append(CLEAR)
            steps.append({"call": "MapGetRoomLoc", "args": [m, r], "as": "%s%d" % (tag, i), "capture": {"refs": "%lv.lvMapLoc"}})
            steps.append({"call": "MapMapExits", "args": [345, r, m], "as": "%st%d" % (tag, i), "capture": {"tip": "%tip"}})
        return steps

    for name, keys in groups.items():
        if name.startswith("exit-"):
            continue
        add("refs-" + name, room_steps(keys, "r"))
    # exit kinds in two groups, each also with the "show all exits" box on (it appends the destination room names)
    ex_a = [k for kind in kinds[:7] for k in groups["exit-" + kind]]
    ex_b = [k for kind in kinds[7:] for k in groups["exit-" + kind]]
    for gid, keys in (("a", ex_a), ("b", ex_b), ("invented", groups["exit-invented"])):
        add("refs-exits-" + gid, room_steps(keys, "r"))
        add("refs-exits-%s-all" % gid, room_steps(keys, "r"), forms={"chkMapOptions": {"$array": {"6": {"Value": 1}}}})

    # display options and data versions over one mixed set
    mixed = (groups["npc"][:3] + groups["cmd"][:3] + groups["lair"][:4] + groups["shop-spell-placed"][:3] + groups["light"][:3]
             + [(9001, 1), (9001, 2), (9001, 3)])
    variants = [
        ("hide-numbers", {}, {"bHideRecordNumbers": True}),
        ("hide-npc", {"chkMapOptions": {"$array": {"3": {"Value": 1}}}}, {}),
        ("hide-lair", {"chkMapOptions": {"$array": {"2": {"Value": 1}}}}, {}),
        ("hide-cmd", {"chkMapOptions": {"$array": {"4": {"Value": 1}}}}, {}),
        ("nmr181", {}, {"nNMRVer": 1.81}),
        ("nmr182", {}, {"nNMRVer": 1.82}),
        ("illu-low", {"lblInvenCharStat": {"$array": {"23": {"Caption": "-100"}}}}, {}),
        ("illu-high", {"lblInvenCharStat": {"$array": {"23": {"Caption": "120"}}}}, {}),
    ]
    for vid, forms, glob_ in variants:
        add("opt-" + vid, room_steps(mixed, "r"), forms=forms, globals_=glob_)

    # pre-calculated damage: arrays are -1 (not calculated) except the monsters a case sets
    lair_mobs = sorted({int(x) for k in subset for x in re.findall(r"(\d+),", full_by[k]["Lair"].split(":", 1)[-1])
                        if not full_by[k]["Lair"].startswith("[")})
    dmg_sel = lair_mobs[:3]
    top = max(t.monsters)
    dmg_pos = [dmg_sel[0]]

    def arr(vals):
        a = [-1] * (top + 1)
        for k, v in vals.items():
            a[k] = v
        return a

    add("dmg-default", room_steps(groups["lair"][:5], "r"), globals_={"nMonsterDamageVsDefault": arr({n: 7 + i for i, n in enumerate(dmg_sel)})})
    add("dmg-char", room_steps(groups["lair"][:5], "r"),
        forms={"chkGlobalFilter": {"Value": 1}},
        globals_={"nMonsterDamageVsChar": arr({n: 11 + i for i, n in enumerate(dmg_sel)}),
                  "nMonsterDamageVsDefault": arr({n: 7 + i for i, n in enumerate(dmg_sel)})})
    add("dmg-party", room_steps(groups["lair"][:5], "r"),
        forms={"optMonsterFilter": {"$array": {"1": {"Value": True}}}, "txtMonsterLairFilter": {"$array": {"0": {"Text": "3"}}}},
        globals_={"nMonsterDamageVsParty": arr({n: 31 + i for i, n in enumerate(dmg_sel)})})
    add("room-missing", room_steps([(9999, 1), (1, 99999)], "r"))

    # ---- text find: Find then Find Next until the end (and a restart)
    FIND = {"call": "cmdMapFindText_Click", "args": [0]}
    NEXT = {"call": "cmdMapFindText_Click", "args": [1]}
    for i, (q, _) in enumerate(text_queries):
        hits = sum(1 for k in ordered if q.lower() in (full_by[k]["Name"] or "").lower() and k in subset)
        steps = [FIND] + [NEXT] * min(hits + 1, 12)
        add("text-%d" % i, steps, inputbox=[q], read={})
    add("text-restart", [FIND] + [NEXT] * 20, inputbox=["tomb", "tomb"], read={})
    add("text-next-first", [NEXT, NEXT, NEXT], inputbox=["dirt"], read={})
    add("text-end", [FIND, NEXT, NEXT, NEXT, NEXT], inputbox=["test annex", "annex"], read={})
    add("text-empty", [FIND, NEXT], inputbox=[""], read={})
    add("text-new-after-hit", [FIND, NEXT, FIND, NEXT], inputbox=["tomb", "dirt"], read={})

    # ---- exits finder
    mask_btn = lambda ds: {"$array": {str(i): {"Tag": "1" if d in ds else "0"} for i, d in enumerate(DIRS)}}
    fcap = {"rows": "%flv.frmResults.lvResults", "caption": "%f.frmResults.lblCaption.Text"}

    def finder_case(cid, name, exact, ds, ok=True):
        forms = {"frmPopUpOptions": {"cmdRoomFindDir": mask_btn(ds), "optRoomFindMatch": {"$array": {"0": {"Value": not exact}, "1": {"Value": exact}}},
                                     "txtRoomName": {"Text": name}},
                 "frmResults": {"lvResults": {"$recordlv": []}, "lblCaption": {"Text": ""}},
                 "frmMain": {"txtRoomFindOk": {"Text": "1" if ok else "-1"}}}
        add(cid, [{"call": "FindRoomWithDirections", "args": [], "as": "f", "capture": fcap}], forms=forms)

    for i, (name, exact, ds) in enumerate(finders):
        finder_case("exits-%d" % i, name, exact, ds)
    finder_case("exits-limit", "tunnel", False, big_dirs)
    finder_case("exits-limit-exact", "Stone Tunnel", True, big_dirs)
    finder_case("exits-short-name", "ab", False, ["N", "S"])
    finder_case("exits-no-dirs", "Stone Tunnel", False, [])
    finder_case("exits-cancel", "Stone Tunnel", False, ["N", "S"], ok=False)
    finder_case("exits-nothing", "Qqqq Zzzz", False, ["N"])
    finder_case("exits-trim", "  Dirt Path  ", True, ["E", "W"])
    finder_case("exits-case", "DIRT PATH", False, ["E", "W"])
    finder_case("exits-exact-case", "dirt path", True, ["E", "W"])
    finder_case("exits-invented", "Test", False, ["N", "U", "D"])
    finder_case("exits-invented-2", "Test", False, ["N"])
    finder_case("exits-invented-3", "Test Annex", False, ["N", "S", "E", "W", "NE", "NW"])

    # ---- room command room lists
    def cmdrooms(cid, room_list, row_text, percent):
        forms = {"frmResults": {"lvResults": {"$recordlv": []}, "lblCaption": {"Caption": ""}}}
        add(cid, [{"call": "ShowRoomCmdRoomList", "args": [room_list, row_text, percent, {"control": "frmResults"}], "as": "l",
                   "capture": {"rows": "%flv.frmResults.lvResults", "caption": "%f.frmResults.lblCaption.Caption"}}], forms=forms)

    a = sorted(stone[:3])
    cmdrooms("cmdrooms-list", "|".join("%d/%d" % k for k in a), 'Room cmd "pry coffin": Tomb (17/278) +2 more', "0.3%")
    cmdrooms("cmdrooms-range", "9001/1-4", 'Room cmd "dig": Test Hall (9001/1) +3 more', "")
    cmdrooms("cmdrooms-chest", "9001/1|9001/3|9002/1", 'Room cmd "knock": Test Hall (9001/1) -> chest (3)', "12%")
    cmdrooms("cmdrooms-empty", "", 'Room cmd "x": nowhere', "1%")

    # ---- data for the Rust test
    for (m, r) in sorted(subset):
        row = full_by[(m, r)]
        for lm in re.findall(r"(\d+),", (row["Lair"] or "").split(":", 1)[-1]):
            need.mons.add(int(lm))
        if row["NPC"]:
            need.mons.add(row["NPC"])
            mon = t.monsters.get(row["NPC"])
            if mon and mon.get("GreetTXT"):
                need.tb(mon["GreetTXT"])
        if row["Shop"]:
            need.shops.add(row["Shop"])
        if row["Spell"]:
            need.spell_refs(row["Spell"])
        if row["CMD"]:
            need.tbs.add(row["CMD"])
        for p in (row["Placed"] or "").split(","):
            if p.strip().isdigit():
                need.items.add(int(p))
        for d in DIRS:
            v = str(row[d])
            for key in re.findall(r"(?:Key: |Item: |Item\): )(\d+)", v):
                need.items.add(int(key))
            for sp in re.findall(r"(?:pre-|post-|Spell Trap: )(\d+)", v):
                if int(sp):
                    need.spell_refs(int(sp))
    md.tele_rooms(t, need, 0)
    # teleports of the room commands and NPC greet chains: the rooms they name must be in the subset (the name shows)
    for n in list(need.tbs):
        for tele in re.finditer(r"teleport (\d+)(?: (\d+))?", (t.tb.get(n, {}).get("Action") or "")):
            pass
    data = md.build_data(t, need, [], [])
    # items and monsters: every name is cheap, so the data carries them all (a lookup the closure missed would differ)
    data["items"] = md.table_json(["Number", "Name"], [[n, r["Name"] or ""] for n, r in sorted(t.items.items())])
    data["monsters"] = md.table_json(["Number", "Name", "GreetTXT"], [[n, r["Name"] or "", md.nz(r["GreetTXT"])]
                                                                       for n, r in sorted(t.monsters.items())])
    data["shops"] = md.table_json(["Number", "Name"], [[n, r["Name"] or ""] for n, r in sorted(t.shops.items())])
    data["rooms"] = md.table_json(ROOM_F, [[full_by[k][f] if f not in ("Name", "Lair", "Placed") else (full_by[k][f] or "")
                                            for f in ROOM_F] for k in sorted(subset)])
    data["lairs"] = lair_table
    data["tbinfo"] = md.table_json(["Number", "LinkTo", "Action"],
                                   [[n, md.nz(r["LinkTo"]), r["Action"] or ""] for n, r in sorted(t.tb.items())
                                    if n in need.tbs or n in (20201, 20202, 20203)])
    data["max_monster"] = top

    FRM = {
        "file": "frmMain.frm",
        "decls": ["nMapLastFind", "sMapSearch", "bMapCancelFind", "nMapStartMap", "nMapStartRoom", "CellRoom", "UnchartedCells",
                  "ALT_CellRoom", "ALT_UnchartedCells", "nOverwritePasses", "bUseZoomMap", "bNoAlwaysOnTop", "nMap_iGoBack"],
        "procs": ["MapGetRoomLoc", "MapMapExits", "cmdMapFindText_Click", "FindRoomWithDirections", "ShowRoomCmdRoomList"],
        "enums": ["EnumDrawRoom"],
        "me_is_form": True,
        "tooltip_recorder": True,
        "stub_procs": ["MapDrawOnRoom", "ConvertScale"],
        "record_procs": ["MapStartMapping"],
        # a helper of the scenario (not MME's): the caller empties the list before MapGetRoomLoc
        "extra": ["Public Sub RoomsClearRefs()", "lvMapLoc.ListItems.Clear", "End Sub"],
        "subs_all": {
            "cmdMapFindText_Click": {"Load frmProgressBar": "", "Unload frmProgressBar": "",
                                    "Set frmProgressBar.objFormOwner = Me": "", "frmProgressBar.Show vbModeless, Me": ""},
            "FindRoomWithDirections": {"Load frmProgressBar": "", "Load frmPopUpOptions": "", "Unload frmResults": "",
                                       "Set frmProgressBar.objFormOwner = Me": "", "frmProgressBar.Show vbModeless, Me": "",
                                       "Call frmResults.SetupResultsWindow(False, Me)": "",
                                       "frmResults.Show vbModeless, IIf(bNoAlwaysOnTop, Nothing, Me)": ""},
        },
        "subs": {
            "MapMapExits": {"ActivatedCell = MapActivateCell(Cell, x, nExitType)": "ActivatedCell = -1"},
            "FindRoomWithDirections": {"frmPopUpOptions.Show vbModal, Me": 'frmPopUpOptions.Tag = frmMain.txtRoomFindOk.Text'},
        },
    }

    scn = {
        "family": "rooms",
        "description": "Rooms tab (stock): MapGetRoomLoc and MapMapExits for the room's references and tooltip text, "
                       "cmdMapFindText_Click, FindRoomWithDirections and ShowRoomCmdRoomList.",
        "notes": [
            "Steps run MME's own frmMain procedures cut from frmMain.frm. The map drawing is not run: MapDrawOnRoom is "
            "blanked, MapMapExits' call of MapActivateCell answers -1 (no cell is activated), and the tooltip text is read "
            "from objToolTip.SetToolTipItem (read %tip).",
            "A references step captures lvMapLoc as [Text, Tag, cells] per row; the helper RoomsClearRefs empties it between rooms, "
            "as lblRoomCell_MouseDown does.",
            "The oracle database holds a subset of Rooms (db_only pairs); data.rooms is the same rows for the Rust test. "
            "Other tables are whole in the oracle; data holds the names and records the cases can reach.",
            "FindRoomWithDirections: frmPopUpOptions.Show is replaced by setting Tag from frmMain.txtRoomFindOk (1 = OK, -1 = cancel); "
            "the name, match mode and direction buttons are the case's frmPopUpOptions controls. Results are frmResults.lvResults.",
            "cmdMapFindText_Click: InputBox answers come from the case's inputbox list; MapStartMapping is recorded as an event "
            "(its arguments are the hit); MsgBox texts are recorded events.",
            "GetLairInfo answers from data.lairs; the damage arrays are -1 except where a case sets them.",
            "Rows numbered 9001 and up (rooms of maps 9001 and 9002, monster 9002, textblocks 20201 to 20203) are invented.",
        ],
        "database": True,
        "db_only": {"Rooms": [[m, r] for (m, r) in sorted(stock_subset)]},
        "db_extra": {"Rooms": INV_ROOMS, "TBInfo": INV_TB, "Monsters": INV_MON},
        "shims": ["lairs"],
        "frm": FRM,
        "data": data,
        "globals": {"bHideRecordNumbers": False, "nNMRVer": 1.83, "bGreaterMUD": False,
                    "nMonsterDamageVsDefault": [-1] * (top + 1), "nMonsterDamageVsChar": [-1] * (top + 1),
                    "nMonsterDamageVsParty": [-1] * (top + 1)},
        "forms": {"frmMain": {
            "name": "frmMain", "lvMapLoc": {"$recordlv": []},
            "chkMapOptions": {"$array": {str(i): {"Value": 0} for i in range(13)}},
            "optAlsoMark": {"$array": {str(i): {"Value": False} for i in range(4)}},
            "lblInvenCharStat": {"$array": {"23": {"Caption": "0"}}},
            "chkGlobalFilter": {"Value": 0}, "optMonsterFilter": {"$array": {"1": {"Value": False}}},
            "txtMonsterLairFilter": {"$array": {"0": {"Text": "1"}}},
            "txtRoomFindOk": {"Text": "1"}},
            "frmResults": {"lvResults": {"$recordlv": []}, "lblCaption": {"Text": ""}},
            "frmPopUpOptions": {}, "frmProgressBar": {}, "frmMap": {}},
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("%d cases, %d rooms (%d stock)" % (len(cases), len(subset), len(stock_subset)), file=sys.stderr)


if __name__ == "__main__":
    main()

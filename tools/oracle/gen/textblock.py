#!/usr/bin/env python3
"""Writes fixtures/scenarios/textblock.json (deterministic, stdlib only).

    python3 -I tools/oracle/gen/textblock.py [--check]

The scenario names textblock and room numbers taken from the stock database (cached by
tools/mdb-fetch.sh) and a few hand-written strings for the Room(..) Obtained From syntax that the
stock data does not contain. Only numbers and short exit texts go into the scenario; the
textblock text itself reaches the fixture as the oracle's own output of GetTextblockAction.
Selection is by sha256 of the record number, so it never depends on the Python version.
"""
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
SHA = "eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f"
OUT = os.path.join(ROOT, "fixtures", "scenarios", "textblock.json")

# how many of each kind (tuned so scenario + fixture stay under 1.5 MB)
N_RANDOM, N_GIVE, N_OTHER, N_PLAIN, N_EMPTY = 70, 90, 120, 25, 15
N_TRIGGER, N_LINE, N_TELE_CHAIN, N_CHEST = 90, 70, 40, 90
N_ROOMS, N_ROOMS_STR, N_EXITS = 150, 60, 90


def key(tag, n):
    return hashlib.sha256(("%s:%s" % (tag, n)).encode()).hexdigest()


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: key(tag, n))[:count])


def db():
    cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
    path = os.environ.get("ORACLE_DB") or os.path.join(cache, "mdb", SHA + ".sqlite")
    return sqlite3.connect(path)


def refs(action):
    return [int(m) for m in re.findall(r"random (\d+)", action)]


def main():
    con = db()
    tb = {n: (l, a or "") for n, l, a in con.execute("select Number, LinkTo, Action from TBInfo")}
    nonempty = [n for n in sorted(tb) if tb[n][1]]
    tele = [n for n in nonempty if "teleport " in tb[n][1]]
    rnd = [n for n in nonempty if "random " in tb[n][1] and n not in tele]
    give = [n for n in nonempty if "giveitem " in tb[n][1] and n not in tele and n not in rnd]
    other = [n for n in nonempty if ":" in tb[n][1] and n not in tele and n not in rnd and n not in give]
    plain = [n for n in nonempty if ":" not in tb[n][1] and n not in tele and n not in rnd]
    empty = [n for n in sorted(tb) if not tb[n][1]]

    base = set(tele) | set(pick("r", rnd, N_RANDOM)) | set(pick("g", give, N_GIVE))
    base |= set(pick("o", other, N_OTHER)) | set(pick("p", plain, N_PLAIN)) | set(pick("e", empty, N_EMPTY))
    base.add(5)  # the explicit-data cases pass number 5 with empty text
    # the fixture's textblock table is made of these cases, so recursion targets must be in it
    todo = list(base)
    while todo:
        n = todo.pop()
        for r in refs(tb[n][1]):
            if r in tb and r not in base:
                base.add(r)
                todo.append(r)
    base = sorted(base)

    cases = []
    grids = []

    def case(cid, steps):
        cases.append({"id": cid, "steps": steps})

    grids.append({
        "name": "tb", "axes": {"n": base},
        "case": {"steps": [
            {"call": "GetTextblockAction", "args": ["$n"], "as": "a"},
            {"call": "GetTextblockLinkTo", "args": ["$n"], "as": "l"},
            {"call": "GetTextblockCMDS", "args": ["$n", 0], "as": "c"},
            {"call": "GetTextblockCMDS", "args": ["$n", 14], "as": "m"}]}})

    # edge numbers
    for i, n in enumerate([0, 99999, -3, 32768]):
        case("e%d" % i, [
            {"call": "GetTextblockAction", "args": [n], "as": "a"},
            {"call": "GetTextblockLinkTo", "args": [n], "as": "l"},
            {"call": "GetTextblockCMDS", "args": [n, 0], "as": "c"},
            {"call": "GetTextblockTrigger", "args": [n, 5], "as": "t"},
            {"call": "GetTextblockCMDLine", "args": ["x", "", n], "as": "cl"},
            {"call": "GetTextblockCMDText", "args": ["x", "", n], "as": "ct"},
            {"call": "GetTextblockTeleport", "args": [n, {"ref": "long", "value": 7}, {"ref": "long", "value": 7}], "as": "tp"},
            {"call": "TextBlockHasTeleport", "args": [n, 5, 0, False, 0], "as": "h"}])

    # triggers
    cand = [n for n in base if ":" in tb[n][1]]
    for n in pick("t", cand, N_TRIGGER):
        l, a = tb[n]
        vals = []
        if l:
            vals.append(l)
        vals += [int(v) for v in re.findall(r"(?:random|text) (\d+)", a)[:2]]
        vals += [int(v) for v in re.findall(r":(\d+)", a)[:2]]
        vals += [99999, 0]
        steps = [{"call": "GetTextblockTrigger", "args": [n, v], "as": "t%d" % i}
                 for i, v in enumerate(vals[:5])]
        case("t%d" % n, steps)

    # command line / text
    for n in pick("c", cand, N_LINE):
        a = tb[n][1]
        lines = [x for x in a.split("\n") if ":" in x]
        words = []
        for x in (lines[0], lines[len(lines) // 2], lines[-1]):
            words.append(x.split(":")[0])
        words.append(words[0][:3])
        words.append("zzqq")
        steps = []
        for i, w in enumerate(words):
            steps.append({"call": "GetTextblockCMDLine", "args": [w, "", n], "as": "l%d" % i})
            steps.append({"call": "GetTextblockCMDText", "args": [w, "", n], "as": "x%d" % i})
        case("c%d" % n, steps)

    # explicit-data variants (the optional text argument)
    datas = ["", "a:b", "buy x:monsters:price 5\nsell y:z\n", "k:v\n", "k:", "k", "a:1\r\nb:2\r\n", "*x*|y:1\n:2\nlast"]
    cmds = ["", "a", "a:", "k", "k:", "b", "y", "x", ":", "last", "buy x", "sell"]
    i = 0
    for d in datas:
        steps = []
        for j, c in enumerate(cmds):
            steps.append({"call": "GetTextblockCMDLine", "args": [c, d, 5], "as": "l%d" % j})
            steps.append({"call": "GetTextblockCMDText", "args": [c, d, 5], "as": "x%d" % j})
        case("d%d" % i, steps)
        i += 1

    # teleports
    def first_tele(a):
        for line in a.split("\n"):
            m = re.search(r"teleport (\d+)(?: (\d+))?", line)
            if m:
                return int(m.group(1)), int(m.group(2) or 0)
        return None

    for n in tele:
        room, mp = first_tele(tb[n][1]) or (5, 0)
        steps = [
            {"call": "GetTextblockTeleport", "args": [n, {"ref": "long", "value": 7}, {"ref": "long", "value": 7}], "as": "tp"},
            {"call": "TextBlockHasTeleport", "args": [n, room, 0, False, 0], "as": "h0"},
            {"call": "TextBlockHasTeleport", "args": [n, room, mp, True, 0], "as": "h1"},
            {"call": "TextBlockHasTeleport", "args": [n, room, mp + 1, False, 0], "as": "h2"},
            {"call": "TextBlockHasTeleport", "args": [n, room + 1, mp, False, 0], "as": "h3"}]
        case("p%d" % n, steps)
    # chains: textblocks that reach a teleport only through "random"
    chain = [n for n in rnd if n in base and any(r in tele for r in refs(tb[n][1]))]
    for n in pick("h", chain, N_TELE_CHAIN):
        target = next(r for r in refs(tb[n][1]) if r in tele)
        room, mp = first_tele(tb[target][1]) or (5, 0)
        case("h%d" % n, [
            {"call": "TextBlockHasTeleport", "args": [n, room, 0, False, 0], "as": "h0"},
            {"call": "TextBlockHasTeleport", "args": [n, room, mp, True, 0], "as": "h1"},
            {"call": "TextBlockHasTeleport", "args": [n, room, 0, False, 4], "as": "h2"},
            {"call": "TextBlockHasTeleport", "args": [n, room, 0, False, 5], "as": "h3"},
            {"call": "GetTextblockTeleport", "args": [n, {"ref": "long", "value": 7}, {"ref": "long", "value": 7}], "as": "tp"}])

    # chests
    chestable = [n for n in base if "giveitem " in tb[n][1] or "random " in tb[n][1]]
    arr = {"ref": "array:currency", "bounds": [[1, 3], [0, 0]]}
    for n in pick("x", chestable, N_CHEST):
        case("x%d" % n, [{"call": "GetChestItems", "args": [arr, n, {"ref": "long", "value": 0}], "as": "g"}])
    some = pick("y", chestable, 12)
    for n in some:
        for nest in (4, 5, 6):
            case("y%d-%d" % (n, nest), [{"call": "GetChestItems", "args": [arr, n, {"ref": "long", "value": nest}], "as": "g"}])
        case("z%d" % n, [{"call": "GetChestItems",
                          "args": [arr, n, {"ref": "long", "value": 0}, {"t": "cur", "v": 0.35}], "as": "g"},
                         {"call": "GetChestItems", "args": [arr, n, {"ref": "long", "value": 0}], "as": "g2"}])
    case("ye", [{"call": "GetChestItems", "args": [arr, 99999, {"ref": "long", "value": 0}], "as": "g"},
                {"call": "GetChestItems", "args": [arr, 0, {"ref": "long", "value": 0}], "as": "g2"}])

    # rooms with a command
    rooms = [(m, r, c) for m, r, c in con.execute(
        'select "Map Number", "Room Number", CMD from Rooms where CMD > 0')]
    chosen = sorted(sorted(rooms, key=lambda t: key("rm", "%d/%d" % t[:2]))[:N_ROOMS])
    for m, r, c in chosen:
        cases.append({"id": "r%d.%d" % (m, r), "proc": "GetRoomCMDTB", "args": ["", m, r]})
    for m, r, c in sorted(sorted(chosen, key=lambda t: key("rs", "%d/%d" % t[:2]))[:N_ROOMS_STR]):
        suffix = ["", " (Door)", " x", "/5"][int(key("sf", "%d/%d" % (m, r)), 16) % 4]
        cases.append({"id": "s%d.%d" % (m, r), "proc": "GetRoomCMDTB", "args": ["%d/%d%s" % (m, r, suffix)]})
    none = [(m, r) for m, r in con.execute(
        'select "Map Number", "Room Number" from Rooms where CMD = 0 limit 400')]
    for m, r in pick("rn", none, 12):
        cases.append({"id": "n%d.%d" % (m, r), "proc": "GetRoomCMDTB", "args": ["", m, r]})
    for i, a in enumerate([["", 0, 5], ["", 5, 0], ["", 1, 99999999], ["", -1, 2], ["0/5"], ["5/0"], ["abc"],
                           ["/5"], ["99999999999/5"], ["1/99999999999"], [""]]):
        cases.append({"id": "q%d" % i, "proc": "GetRoomCMDTB", "args": a})

    # exit texts
    pats = {}
    for row in con.execute("select N,S,E,W,NE,NW,SE,SW,U,D from Rooms"):
        for v in row:
            v = v or ""
            pats.setdefault(re.sub(r"\d+", "#", v), v)
    exits = pick("ex", sorted(pats), N_EXITS)
    exits = [pats[p] for p in exits]
    synth = ["", "/", "5/", "/5", "a/5", "5/6", " 5/6", "5/6 ", "5 /6", "1/2 3/4", "x12/34", "12/34/56", "0/0",
             "-5/6", "5/-6", "99999999999/5", "5/99999999999", "5/6 (Door)", "5/6  two", "5/", "1/2 ", "7/8 a b c",
             "Action [on the N exit of this room]: pull lever", "10/20 (Text: go path, enter path)"]
    for i, s in enumerate(synth + exits):
        cases.append({"id": "m%d" % i, "proc": "ExtractMapRoom", "args": [s]})
    texts = [p.split(" ", 1)[1] if " " in p else p for p in exits] + [
        "", "a", "a b", "a b,c", "a , b", "a ,,b", ", ,x", "x ,y,z", "Text: go path, follow trail", "Text:,", "ab cd", " a",
        "Action [on the N exit]: x, y"]
    for i, s in enumerate(texts):
        cases.append({"id": "k%d" % i, "proc": "ExtractTextCommand", "args": [s]})

    # Room(...) Obtained From
    O = []
    base_entries = [
        "Room(pry coffin|push coffin) 17/278|17/2073-2096 (18,6%)",
        "Room(message 1421) 1/2",
        "Room(*pry*|message 5) 3/4|3/6-8 (50%)",
        "Room(giveitem 4) 2/9",
        "Room(a b) 5/6|bad|7/8 (?%)",
        "Room(a) 5/6 (12%)",
        "Room(a) 5/6 (12",
        "Room(a) 5/6 (",
        "Room(a) 5/6 (%)",
        "Room(a) 5/6 (-5%)",
        "Room(a) 5/6 (1.5 %)",
        "Room(a) 5/6|7/8|",
        "Room(a) 5/6|7/8|9/1",
        "Room(a) x/6",
        "Room(a) 0/6",
        "Room(a) 5/0",
        "Room(a) 5/9-3",
        "Room(a) 5/1-100002",
        "Room(a) 5/1-100001",
        "Room(a) 1234567890/5",
        "Room(a) 12/34567890",
        "Room(a)",
        "Room(a",
        "Room() 1/2",
        "Room(|) 1/2",
        "Room(teleport 5 6|text 3) 1/2",
        "Room(Message 7) 1/2",
        "Room(message  7) 1/2",
        "Room(message x) 1/2",
        "Room(random 12 13) 1/2",
        "Room(cast) 1/2",
        "Room(CLASS 3) 1/2",
        "Room(a)b) 1/2",
        "Room(a|b|c) 1/2|1/3-5|2/4",
        "Room(a) 1/2 | 3/4",
        "Room(a) 17/1-6000",
        "Room(a) 17/1-100001|18/1-100001|19/1-100001",
    ]
    singles = base_entries
    for s in singles:
        O.append(s)
        O.append("Shop 4, " + s)
        O.append(s + ", Monster 12(3%)")
        O.append("Shop 4, " + s + ", Textblock 3")
        O.append(s + "+")
        O.append(s + "+\0")
    O += [
        "Room(a) 5/6 (10%), Room(b) 7/8 (20%)",
        "Room(a) 5/6 (10%), Room(b) 7/8",
        "Room(a) 5/6 (10%), Room(b) 7/8|9/1",
        "Room(a) 5/6 (10%), Room(b) 7/8|9/1+",
        "Room(a) 5/6 (10%), Room(b) 7/8|9/1 (5+",
        "Room(a) 5/6 (10%), Room(b) 7/8 (5%)+",
        "Room(a) 5/6 (10%), Shop 3, Room(b) 7/8 (5%)",
        "Room(a) 5/6 (10%), Shop 3+",
        "Room(a) 5/6, Room(b) 7/8|9/+",
        "Shop 1, Room(a) 5/6, Shop 2, Room(b) 1/1",
        "Room 1/527, Room 6/1347",
        "Room(a) 1/2, ",
        "x, Room(a) 1/2",
        "Room(a) 5/6 (10%)+",
        "Room(a) 5/6|7/8|9/+\0",
        "+", "+\0", "", "Room(", "Room(+", ", Room(", ", Room(+",
        "NPC 12", "NPC #12", "npc #12", "Textblock #4", "Textblock 4", "textblock #4, Room 1/2",
        "Textblock #4, Room(a) 1/2", "Room(a) 1/2, Textblock #4", "Room(a) 1/2, NPC #3",
        "Room 1/2, NPC #4", "Shop 5, Monster 3(5%)", "TEXTBLOCK #9", "ROOM 3/4, textblock #1",
        "Monster 5, Room(a) 1/2, Textblock #4, room 2/3",
        "Shop(sell) 3, NPC #4", "Textblock #4(5%)",
    ]
    seen = set()
    for s in O:
        if s in seen:
            continue
        seen.add(s)
        i = len(seen)
        cases.append({"id": "o%d" % i, "steps": [
            {"call": "SplitRoomCmdSources", "args": [s, {"ref": "array:troomcmdsource"}, {"ref": "long", "value": 9}], "as": "s"},
            {"call": "ObtainedFromIsGettable", "args": [s], "as": "g"}]})
    lists = ["", "17/278", "17/278|17/2073-2076", "1/2|bad|3/4-3/5|5/-1|6/2-1", "1/1-5001", "1/1-3000|2/1-3000",
             "1/5000|2/1", "x", "|", "1/2|", " 1/2 | 3/4-5 ", "0/1", "1/1-100001", "1/100000-100000", "5/6-7|8/9"]
    for i, s in enumerate(lists):
        cases.append({"id": "w%d" % i, "steps": [
            {"call": "ExpandRoomCmdRooms", "args": [s, {"ref": "array:string"}], "as": "x"}]})

    scn = {
        "family": "textblock",
        "database": True,
        "description": "FAM-19 textblock and room-command parsing over the stock TBInfo and Rooms tables, plus hand-written Room(..) Obtained From strings.",
        "notes": [
            "Generated by tools/oracle/gen/textblock.py; the textblock text reaches the fixture as the output of GetTextblockAction in the 'tb' cases.",
            "Private procedures (ParseRoomCmdEntry, ParseRoomCmdToken, IsRoomCmd*) are exercised through SplitRoomCmdSources and ExpandRoomCmdRooms.",
            "ByRef array arguments use {\"ref\": \"array:<type>\", \"bounds\": [[lo, hi]..]}; their final contents land in outputs[<as>.refs] (multi-dimensional arrays row-major)."],
        "tolerance": {"default": 0},
        "grids": grids,
        "cases": cases,
    }
    text = '{\n"family":%s,\n"database":true,\n"description":%s,\n"notes":%s,\n"tolerance":{"default":0},\n"grids":%s,\n"cases":[\n%s\n]\n}\n' % (
        json.dumps(scn["family"]), json.dumps(scn["description"]), json.dumps(scn["notes"]),
        json.dumps(grids, separators=(",", ":")),
        ",\n".join(json.dumps(c, separators=(",", ":"), ensure_ascii=True) for c in cases))
    if "--check" in sys.argv:
        with open(OUT) as f:
            sys.exit(0 if f.read() == text else "scenario is stale")
    with open(OUT, "w", newline="\n") as f:
        f.write(text)
    print("wrote %s: %d bytes, %d explicit cases, %d base textblocks" % (OUT, len(text), len(cases), len(base)))


main()

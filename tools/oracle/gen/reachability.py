#!/usr/bin/env python3
"""Writes fixtures/scenarios/reachability.json (FEAT-REACH; card P3-45), deterministic, stdlib only.

    python3 -I tools/oracle/gen/reachability.py [DB.sqlite]

modReachability builds one graph of the whole map and answers every query from it, so the family computes it
once per process (`persist`, `setup_calls`) and every case only queries. The whole stock map (26,694 rooms) is
too big to ship in a fixture, so the family runs on a slice of the stock database chosen here (`db_only`):
the rooms within a short walk of Bank of Godfrey (1/297), the rooms of the unusual edges (room spells, room
commands, NPC conversations, item-gated teleports, random-teleport groups, tolls, waits) with their surroundings,
and the textblocks, spells, monsters and items those reach. The slice is chosen with a rough reading of the
data (exits and teleport numbers); MME's own build then decides what the slice's graph looks like. The rows
themselves reach the fixture header as "data", written by the harness from the same selection.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "reachability.json")
START = (1, 297)
BALL = int(os.environ.get("REACH_BALL", 24))        # walk length from Godfrey that is kept whole
EXITS = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"]
SUMMON_MAX = 700


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


def parse_exit(s):
    m = re.match(r"\s*(\d+)\s*/\s*(\d+)", s or "")
    return (int(m[1]), int(m[2])) if m else None


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    c = sqlite3.connect(sys.argv[1] if len(sys.argv) > 1 else dbs[0])
    c.row_factory = sqlite3.Row
    rooms = {(r["Map Number"], r["Room Number"]): dict(r) for r in c.execute("select * from Rooms")}
    tb = {r["Number"]: dict(r) for r in c.execute("select * from TBInfo")}
    spells = {r["Number"]: dict(r) for r in c.execute("select * from Spells")}
    mons = {r["Number"]: dict(r) for r in c.execute("select * from Monsters order by Number")}
    items = {r["Number"]: dict(r) for r in c.execute("select * from Items")}

    def spell_dests(n, depth=0, seen=None):
        """Teleport targets, textblocks and spells a spell leads to (a rough reading, for choosing the slice)."""
        seen = seen if seen is not None else set()
        s = spells.get(n)
        out = []
        if not s or ("s", n) in seen or depth > 4:
            return out
        seen.add(("s", n))
        room = mp = 0
        for i in range(10):
            a, v = s["Abil-%d" % i], s["AbilVal-%d" % i]
            if a == 140 and not room:
                room = v
            if a == 141 and not mp:
                mp = v
            if a == 148 and v:
                out += tb_dests(v, depth + 1, seen, mp_default=0)
            if a == 151 and v:
                out += spell_dests(v, depth + 1, seen)
        if room:
            out.append((mp, room))
        return out

    def tb_dests(n, depth=0, seen=None, mp_default=0):
        seen = seen if seen is not None else set()
        t = tb.get(n)
        out = []
        if not t or ("t", n) in seen or depth > 5:
            return out
        seen.add(("t", n))
        for m in re.finditer(r"(?i)\bteleport\s+(\d+)(?:\s+(\d+))?", t["Action"] or ""):
            out.append((int(m[2] or 0), int(m[1])))
        for m in re.finditer(r"(?i)\b(?:text|random)\s+(\d+)", t["Action"] or ""):
            out += tb_dests(int(m[1]), depth + 1, seen)
        for m in re.finditer(r"(?i)\bcast\s+(\d+)", t["Action"] or ""):
            out += spell_dests(int(m[1]), depth + 1, seen)
        if t["LinkTo"]:
            out += tb_dests(t["LinkTo"], depth + 1, seen)
        return out

    # rough adjacency (exits and teleports), directed
    adj = {}
    und = {}
    for k, r in rooms.items():
        a = adj.setdefault(k, [])
        for d in EXITS:
            e = parse_exit(r[d])
            if e and e in rooms:
                a.append(e)
                und.setdefault(e, []).append(k)
                und.setdefault(k, []).append(e)
        dests = []
        if r["CMD"]:
            dests += tb_dests(r["CMD"])
        if r["Spell"]:
            dests += spell_dests(r["Spell"])
        for mp, rm in dests:
            e = (mp or k[0], rm)
            if e in rooms:
                a.append(e)
                und.setdefault(e, []).append(k)
                und.setdefault(k, []).append(e)

    def bfs(seeds, graph, limit):
        dist = {s: 0 for s in seeds if s in rooms}
        q = list(dist)
        for u in q:
            if dist[u] >= limit:
                continue
            for v in graph.get(u, ()):
                if v not in dist:
                    dist[v] = dist[u] + 1
                    q.append(v)
        return dist

    keep = set(bfs([START], adj, BALL))
    # the unusual edges: room spells (maps 12 and 15, and a few random-teleport rooms of map 17), room commands and
    # NPC rooms away from the start
    seeds = [k for k, r in rooms.items() if r["Spell"] and k[0] in (12, 15)]
    seeds += pick("m17", [k for k, r in rooms.items() if r["Spell"] and k[0] == 17], 6)
    seeds += pick("cmd", [k for k, r in rooms.items() if r["CMD"] and k[0] in (3, 8, 10, 12, 15)], 20)
    seeds += pick("npc", [k for k, r in rooms.items() if r["NPC"] and k[0] in (1, 2, 3, 6, 15, 16)], 12)
    seeds = sorted(set(seeds))
    for s in seeds:
        keep.add(s)
        for mp, rm in (spell_dests(rooms[s]["Spell"]) if rooms[s]["Spell"] else []) + \
                (tb_dests(rooms[s]["CMD"]) if rooms[s]["CMD"] else []):
            keep.add((mp or s[0], rm))
    # their surroundings (so destinations connect), and the walk from Godfrey to the random-teleport rooms
    keep |= set(bfs(sorted(keep), und, 0)) | set(bfs(seeds, und, 1))
    dests = {k for s in seeds for k in ([(mp or s[0], rm) for mp, rm in
             (spell_dests(rooms[s]["Spell"]) if rooms[s]["Spell"] else [])])}
    keep |= set(bfs(sorted(d for d in dests if d in rooms), und, 3))
    par = {START: None}
    q = [START]
    for u in q:
        for v in adj.get(u, ()):
            if v not in par:
                par[v] = u
                q.append(v)
    for s in seeds:
        u = s if s in par else None
        while u is not None:
            keep.add(u)
            u = par[u]
    keep = sorted(k for k in keep if k in rooms)

    # textblocks: closure from the rooms' commands and the greeting texts of monsters, over text/random/cast/LinkTo
    # numbers; spells: those a kept room, textblock or monster names, plus every teleport/summon-chain spell
    kept_rooms = set(keep)
    tbs, sps = set(), set()
    for k in keep:
        if rooms[k]["CMD"]:
            tbs.add(rooms[k]["CMD"])
        if rooms[k]["Spell"]:
            sps.add(rooms[k]["Spell"])
    chain_spells = {n for n, s in spells.items()
                    if any(s["Abil-%d" % i] in (140, 141, 148, 151) and s["AbilVal-%d" % i] for i in range(10))}
    sps |= chain_spells

    def ref_nums(text, pat):
        return {int(x) for x in re.findall(pat, text or "")}

    def near_room(text):
        return any((int(a), int(b)) in kept_rooms for a, b in re.findall(r"(?:Room |Group(?:\(lair\))?: )(\d+)/(\d+)", text or ""))

    def mon_ok(n, m):
        sb = m["Summoned By"] or ""
        return len(sb) <= SUMMON_MAX
    # monsters: those placed in a kept room (directly, or through a kept room's NPC), and ones that name textblocks
    # or spells; plus a few located elsewhere (unreachable here), unplaced and out-of-game ones
    npc_mons = {rooms[k]["NPC"] for k in keep if rooms[k]["NPC"]}
    sel = set()
    for n, m in mons.items():
        if not m["Name"] or str(m["Name"]).startswith("sdf") or not mon_ok(n, m):
            continue
        sb = m["Summoned By"] or ""
        if near_room(sb) or n in npc_mons:
            sel.add(n)
    elsewhere = [n for n, m in mons.items() if n not in sel and mon_ok(n, m) and m["In Game"] and (m["Summoned By"] or "")
                 and re.search(r"Room \d+/\d+|Group", m["Summoned By"])]
    sel |= set(pick("elsewhere", elsewhere, 18))
    sel |= set(pick("nogame", [n for n, m in mons.items() if not m["In Game"] and mon_ok(n, m)], 4))
    sel |= set(pick("noloc", [n for n, m in mons.items() if m["In Game"] and not (m["Summoned By"] or "").strip()
                              and mon_ok(n, m)], 4))
    # summon sources of the selected monsters
    sources_sp = set()
    for n in sel:
        sb = mons[n]["Summoned By"] or ""
        tbs |= ref_nums(sb, r"Textblock(?:\(rndm\))? #(\d+)")
        sps |= ref_nums(sb, r"Spell #(\d+)")
        if mons[n]["GreetTXT"]:
            tbs.add(mons[n]["GreetTXT"])
    for n, m in mons.items():     # monsters other monsters name in "Monster #n"
        pass
    # close over textblocks and spells
    changed = True
    while changed:
        changed = False
        for t in sorted(tbs):
            row = tb.get(t)
            if not row:
                continue
            new = ref_nums(row["Action"], r"(?i)\b(?:text|random|cast)\s+(\d+)")
            new_t = set(re.findall(r"(?i)\btext\s+(\d+)", row["Action"] or "")) | set(re.findall(r"(?i)\brandom\s+(\d+)", row["Action"] or ""))
            new_s = set(re.findall(r"(?i)\bcast\s+(\d+)", row["Action"] or ""))
            add_t = {int(x) for x in new_t} | ({row["LinkTo"]} if row["LinkTo"] else set())
            add_s = {int(x) for x in new_s}
            # a bare number after a colon is a jump too
            add_t |= {int(x) for x in re.findall(r"(?:^|:)\s*(\d{1,6})\s*(?=:|$)", row["Action"] or "", re.M)}
            if not add_t <= tbs or not add_s <= sps:
                tbs |= add_t
                sps |= add_s
                changed = True
        for s in sorted(sps):
            row = spells.get(s)
            if not row:
                continue
            for i in range(10):
                if row["Abil-%d" % i] == 148 and row["AbilVal-%d" % i] and row["AbilVal-%d" % i] not in tbs:
                    tbs.add(row["AbilVal-%d" % i])
                    changed = True
                if row["Abil-%d" % i] == 151 and row["AbilVal-%d" % i] and row["AbilVal-%d" % i] not in sps:
                    sps.add(row["AbilVal-%d" % i])
                    changed = True
    tbs = {t for t in tbs if t in tb}
    sps = {s for s in sps if s in spells}
    # Called From / Casted By may name a room, textblock or spell that leads to a summon: pull those textblocks too
    # (and the monsters they name) so the summon resolution has something to follow
    for t in sorted(tbs):
        sps |= {s for s in ref_nums(tb[t]["Called From"], r"Spell #(\d+)") if s in spells}
    # items some kept spell needs in the room ('roomitem N')
    need_items = set()
    for t in tbs:
        need_items |= ref_nums(tb[t]["Action"], r"(?i)roomitem\s+(\d+)")
    item_sel = sorted(n for n in need_items if n in items)

    only = {"Rooms": [list(k) for k in keep], "TBInfo": sorted(tbs), "Spells": sorted(sps),
            "Monsters": sorted(sel), "Items": item_sel}
    write(only, keep, sorted(sel), c)


def lvl_forms(level):
    return {"frmMain": {"txtGlobalLevel": {"$array": {"0": {"Text": str(level)}}}}}


def write(only, keep, mon_nums, c):
    nm = len(mon_nums)
    sample_rooms = [list(k) for k in pick("rooms", [tuple(k) for k in keep], 90)]
    # 1/297 first (the start), and rooms that do not exist
    sample_rooms = [[1, 297]] + [r for r in sample_rooms if r != [1, 297]] + [[99, 99], [1, 0]]
    LEVEL_FORMS = {}
    cases = []

    def case(cid, **kw):
        cases.append({"id": cid, **kw})

    EDGE_COLS = "eFrom,eTo,eMin,eMax,eGroup,eKind,eDir,eLabel,eCoin,eWait"
    case("build", steps=[{"call": "BandsText", "each": list(range(1, nm + 1)), "as": "band_text"}],
         read={"rooms": "%m.modReachability.nRooms", "edges": "%m.modReachability.nEdges",
               "groups": "%m.modReachability.nGroups", "dropped": "%m.modReachability.nDroppedRandom",
               "bands": "%m.modReachability.nBP", "start": "%m.modReachability.nStartIdx",
               "bp_levels": "%m.modReachability.bpLevel:nBP",
               "digest": "%digest.modReachability.nEdges:" + EDGE_COLS,
               "room_digest": "%digest.modReachability.nRooms:rMap,rRoom",
               "mon_num": "%m.modReachability.mNum:nMons", "mon_status": "%m.modReachability.mStatus:nMons",
               "poi": "%m.modReachability.nPOIIdx"})
    for lv in (1, 8, 20, 45, 70, 120):
        case("ok_%d" % lv, steps=[{"call": "ReachIsMonsterOK", "each": [[n, lv] for n in mon_nums + [99999]], "as": "ok"}])
    ref = lambda t, v: {"ref": t, "value": v}
    info_tail = [ref("long", 0)] * 6 + [ref("boolean", False), ref("long", 0)]
    for lv in (1, 10, 25, 50, 90, 200):
        case("info_%d" % lv, steps=[{"call": "ReachMonsterInfo", "each": [[n, lv, False] for n in mon_nums + [99999]],
                                    "args": info_tail, "as": "info"}])
    for lv in (1, 20, 100):
        case("room_status_%d" % lv, steps=[{"call": "ReachRoomStatus", "each": [[m, r, lv] for m, r in sample_rooms],
                                           "as": "status"}])
    route_tail = [ref("long", 0), ref("long", 0), ref("string", ""), {"ref": "array:long"}, {"ref": "array:long"},
                  {"ref": "array:long"}, {"ref": "array:string"}, ref("long", 0), ref("long", 0), ref("long", 0),
                  ref("long", 0), ref("string", "")]
    route_mons = pick("routes", mon_nums, 16)
    for cid, lv, gate, ign, alt in (("route_30", 30, False, False, False), ("route_gate_12", 12, True, False, False),
                                    ("route_ignore_5", 5, False, True, False), ("route_alt_60", 60, False, False, True)):
        case(cid, steps=[{"call": "ReachRoute", "each": [[n, lv, gate, ign, alt] for n in route_mons],
                          "args": route_tail, "as": "route"}])
    room_routes = [r for r in sample_rooms[:1] + pick("rr", [tuple(r) for r in sample_rooms[1:-2]], 9)]
    room_routes = [list(r) for r in room_routes] + [[14, 10], [14, 1]]
    for cid, lv, ign, alt in (("room_route_30", 30, False, False), ("room_route_ignore_5", 5, True, False),
                              ("room_route_alt_80", 80, False, True)):
        case(cid, steps=[{"call": "ReachRoomRoute", "each": [[m, r, lv, ign, alt] for m, r in room_routes],
                          "args": route_tail, "as": "route"}])
    row_mons = pick("rows", mon_nums, 36)
    for cid, lv, hide, interval in (("rows_1", 1, False, 0), ("rows_30", 30, False, 0), ("rows_100", 100, False, 0),
                                    ("rows_30_hide_5", 30, True, 5)):
        case(cid, forms=lvl_forms(lv), globals={"bHideRecordNumbers": hide, "nRouteWaypointInterval": interval},
             steps=[{"rows": {"table": "tabMonsters", "index": "pkMonsters", "keys": row_mons, "lv": "list"},
                     "call": "AddMonsterReachRows", "args": [{"key": True}, {"lv": True}], "as": "rows"}])
    scn = {
        "family": "reachability",
        "description": "modReachability on a slice of the stock database: the graph build, level bands, monster and "
                       "room status, gate tracing, routes and the monster pane's reach rows (modMain AddMonsterReachRows).",
        "notes": ["The map is built once per process (persist, setup_calls) and every case queries it.",
                  "db_only is the slice (rooms by [map, room]; other tables by number); header data holds its rows.",
                  "digest: FNV-1a 64 of each edge column, values joined by 0x1f."],
        "database": True, "db_open": True, "persist": True, "modules": ["modReachability.bas"],
        "setup_calls": ["ReachEnsureComputed"], "data_from_db": True, "db_only": only,
        "forms": {"frmMain": {"Enabled": True}, "frmProgressBar": {}},
        "globals": {"sCurrentDatabaseFile": "stock.mdb"},
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("%d cases, %d bytes" % (len(cases), os.path.getsize(OUT)), file=sys.stderr)


if __name__ == "__main__":
    main()

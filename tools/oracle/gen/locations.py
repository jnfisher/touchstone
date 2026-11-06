#!/usr/bin/env python3
"""Writes fixtures/scenarios/locations.json (FAM-19 reference rows, card P3-31).

    python3 -I tools/oracle/gen/locations.py [DB.sqlite]

GetLocations turns the "where is this" text columns (an item's Obtained From and References, a
monster's Summoned By, a spell's Learned From and Casted By, a shop's Assigned To) into ListView rows;
the oracle runs MME's own GetLocations, AddRoomCmdLocationRow, SplitRoomCmdSources and the name lookups
against the stock database and records the rows (Text, Tag, sub-items) and the SortListView requests.

Cases:
- `item-*`  ~200 items' Obtained From as the item pane asks (percent column, the item as aux value), a third
  also as a plain one-column list and a fifth as two columns; References for some.
- `mon-*`   ~150 monsters' Summoned By as the monster pane asks (one column or two, don't sort, a limit).
- `spell-*` ~100 spells: Casted By alone, or Learned From (header "(learn) ") followed by Casted By.
- `shop-*`   shops' Assigned To.
- `hand-*`   strings written to reach each branch: every keyword, truncation marks, limits, headers and
  footers, room commands, the lair regex for each data version, errors that end the procedure.

GetLairInfo is answered by the oracle's lair shim from the table under "data"/"lairs" (invented but fixed
numbers on the real group indexes). The rows the Rust test needs (items, shops, monsters, spells, rooms)
sit once under "data": the closure of every record the cases can reach.
Deterministic: selection by sha256 of fixed tags; the clock and Python's random are never used.
"""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "locations.json")

N_ITEMS, N_MONS, N_SPELLS = 170, 120, 90
LV = {"control": "lvLoc"}


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def pick(tag, items, count):
    return sorted(sorted(items, key=lambda n: H(tag, n))[:count])


def call(loc, dont_clear=False, header="", aux=0, two=False, dont_sort=False, percent=False, footer="", limit=0):
    """One GetLocations call with every optional argument written out (an omitted one is its default)."""
    return {"call": "GetLocations",
            "args": [loc, LV, dont_clear, header, aux, two, dont_sort, percent, footer, limit]}


def kinds(s):
    """The reference kinds a source string holds (used to spread the sample over every kind)."""
    k = set()
    for key, pat in (("room", r"Room \d"), ("monster", "Monster #"), ("textblock", "Textblock #"), ("item", "Item #"),
                     ("spell", "Spell #"), ("shop", r"Shop #"), ("sell", r"Shop\(sell\)"), ("nogen", r"Shop\(nogen\)"),
                     ("lair", r"Group\(lair\)"), ("group", "Group: "), ("npc", "NPC #"), ("cmd", r"Room\(")):
        if re.search(pat, s):
            k.add(key)
    return k


def stratified(rows, count, tag, maxlen):
    """rows: [(number, text)]. Every kind-signature first (rarest first), then a hash-ordered fill."""
    rows = [(n, s) for n, s in rows if s and len(s) <= maxlen]
    sig = {}
    for n, s in rows:
        sig.setdefault(tuple(sorted(kinds(s))), []).append(n)
    chosen = []
    for key in sorted(sig, key=lambda k: (len(sig[k]), k)):
        for n in sorted(sig[key], key=lambda n: H(tag, "sig", n))[:3]:
            if n not in chosen:
                chosen.append(n)
    rest = [n for n, _ in rows if n not in chosen]
    for n in sorted(rest, key=lambda n: H(tag, "fill", n)):
        if len(chosen) >= count:
            break
        chosen.append(n)
    return sorted(chosen[:count])


KEY = re.compile(r"(?i)(room |monster #|textblock #|textblock\(rndm\) #|item #|spell #|shop #|shop\(sell\) #|shop\(nogen\) #|"
                 r"group\(lair\): |group: |npc #)(\d+(?:/\d+)?)")
PAIR = re.compile(r"(\d+)/(\d+)")


def main():
    dbs = sorted(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/*.sqlite")))
    db = sys.argv[1] if len(sys.argv) > 1 else dbs[0]
    c = sqlite3.connect(db)

    items = {n: (nm, ty, pr, cu, ob or "", rf or "") for n, nm, ty, pr, cu, ob, rf in c.execute(
        'select Number, Name, ItemType, Price, Currency, "Obtained From", "References" from Items order by Number')}
    mons = {n: (nm or "", sb or "") for n, nm, sb in c.execute('select Number, Name, "Summoned By" from Monsters order by Number')}
    spells = {n: (nm or "", lf or "", cb or "") for n, nm, lf, cb in c.execute(
        'select Number, Name, "Learned From", "Casted By" from Spells order by Number')}
    shop_cols = [r[1] for r in c.execute("pragma table_info(Shops)")]
    shops = {}
    for r in c.execute("select * from Shops order by Number"):
        d = dict(zip(shop_cols, r))
        shops[d["Number"]] = {"type": d["ShopType"], "markup": d["Markup%"], "assigned": d["Assigned To"] or "",
                              "stock": [[d["Item-%d" % x], d["Max-%d" % x], d["Time-%d" % x], d["Amount-%d" % x], d["%%-%d" % x]]
                                        for x in range(20) if d["Item-%d" % x]]}
    lairs = {g: (mobs, exp) for g, mobs, exp in c.execute("select GroupIndex, Mobs, AvgExp from Lairs")}
    rooms = {(m, r): (nm or "", dl) for m, r, nm, dl in c.execute('select "Map Number", "Room Number", Name, Delay from Rooms')}

    cases = []

    def add(cid, steps, forms=None, globals_=None):
        d = {"id": cid, "steps": steps}
        if forms:
            d["forms"] = {"frmMain": forms}
        if globals_:
            d["globals"] = globals_
        cases.append(d)

    def view(k, charm_on=None):
        """Per-case frontend state: global filter and charm, record numbers shown or hidden."""
        f = {}
        if charm_on is not None:
            f = {"chkGlobalFilter": {"Value": 1 if charm_on else 0},
                 "txtCharStats": {"$array": {"5": {"Tag": str((0, 25, 50, 100, 37)[k % 5])}}}}
        return f

    # ---- items: Obtained From as the item pane asks (percent column, aux = the item)
    item_rows = [(n, v[4]) for n, v in items.items()]
    chosen_items = stratified(item_rows, N_ITEMS, "item", 700)
    for i, n in enumerate(chosen_items):
        ob = items[n][4]
        g = {"bHideRecordNumbers": H("hide", n) % 7 == 0, "nGlobalAttackTypeMME": H("att", n) % 6}
        add("item-%d" % n, [call(ob, True, "", n, False, False, True)], view(H("ch", n), H("gf", n) % 4 != 0), g)
        if i % 3 == 0:
            add("item1-%d" % n, [call(ob)], view(i, i % 2 == 0), g)
        if i % 5 == 0:
            add("item2-%d" % n, [call(ob, False, "", 0, True)], view(i + 1, True), g)
        if items[n][5] and i % 4 == 0:
            add("itemref-%d" % n, [call(ob, True, "", n, False, False, True), call(items[n][5], True, "", 0, False, False, True)],
                view(i, True), g)
    # a few long ones, with a limit
    longs = [n for n, v in items.items() if len(v[4]) > 700]
    for n in pick("itemlong", longs, 6):
        add("itemlong-%d" % n, [call(items[n][4], True, "", n, False, False, True, "", (5, 20)[n % 2])], view(n, True))

    # ---- monsters: Summoned By as the monster pane asks
    mon_rows = [(n, v[1]) for n, v in mons.items()]
    lair_mons = [(n, s) for n, s in mon_rows if "(lair)" in s]
    other_mons = [(n, s) for n, s in mon_rows if "(lair)" not in s]
    chosen_mons = stratified(lair_mons, 60, "monlair", 1500) + stratified(other_mons, N_MONS - 60, "mon", 1500)
    for i, n in enumerate(sorted(chosen_mons)):
        sb = mons[n][1]
        g = {"nGlobalAttackTypeMME": H("att", n) % 6, "bHideRecordNumbers": H("hide", n) % 6 == 0}
        lim = (0, 5, 10, 30)[H("lim", n) % 4]
        if i % 2 == 0:
            add("mon1-%d" % n, [call(sb, True, "", 0, False, True, False, "", lim)], view(i, None), g)
        else:
            add("mon2-%d" % n, [call(sb, True, "", 0, True, True, False, "", lim)], view(i, None), g)
        if i % 6 == 0:
            add("monpct-%d" % n, [call(sb, True, "", 0, False, True, True, "", lim)], view(i, None), g)
    for n in pick("monlong", [n for n, s in mon_rows if len(s) > 4000], 2):
        add("monlong-%d" % n, [call(mons[n][1], True, "", 0, True, True, False, "", 12)])

    # ---- spells: Casted By alone, or Learned From (header) then Casted By
    spell_rows = [(n, v[2] or v[1]) for n, v in spells.items()]
    chosen_spells = stratified(spell_rows, N_SPELLS, "spell", 400)
    for i, n in enumerate(chosen_spells):
        nm, lf, cb = spells[n]
        g = {"bHideRecordNumbers": H("hide", n) % 8 == 0}
        if lf and i % 2 == 0:
            add("spell2-%d" % n, [call(lf, True, "(learn) "), call(cb, True)], None, g)
        else:
            add("spell-%d" % n, [call(cb)], None, g)
        if i % 7 == 0 and cb:
            add("spellpct-%d" % n, [call(cb, False, "", 0, False, False, True)], None, g)

    # ---- shops: Assigned To
    for n in pick("shop", [n for n in shops if shops[n]["assigned"]], 14):
        add("shop-%d" % n, [call(shops[n]["assigned"])], None, {"bHideRecordNumbers": n % 5 == 0})

    # ---- hand-written
    someroom = sorted(rooms)[:1][0]
    r1, r2, r3 = ("%d/%d" % k for k in sorted(rooms)[100:103])
    shop_n = sorted(n for n in shops if shops[n]["stock"] and shops[n]["type"] != 8)[3]
    stocked = shops[shop_n]["stock"][0][0]
    selling = sorted(n for n in shops if shops[n]["stock"])[1]
    chest = sorted(n for n, v in items.items() if v[1] == 8 and v[4])[:6]
    lair_g = sorted(g for g in lairs)[:6]
    hand = []

    def h(cid, steps, forms=None, globals_=None):
        hand.append((cid, steps, forms, globals_))

    # every keyword, all three layouts
    every = ("Room %s, Monster #%d, Textblock #12, Textblock(rndm) #13, Item #%d, Spell #%d, Shop #%d, Shop(sell) #%d, "
             "Shop(nogen) #%d, Group: %s, NPC #%d" % (r1, sorted(mons)[5], sorted(items)[7], sorted(spells)[3], shop_n,
                                                    selling, selling, r2, sorted(mons)[9]))
    h("every-1", [call(every)])
    h("every-2", [call(every, False, "", 0, True)])
    h("every-pct", [call(every, False, "", 0, False, False, True)])
    h("every-header", [call(every, False, "Found: ")])
    h("every-header-colon", [call(every, False, "Where: ", 0, True)])
    h("every-hide", [call(every, False, "", 0, True)], None, {"bHideRecordNumbers": True})
    h("every-footer", [call(every, False, "", 0, False, False, False, " (note)")])
    h("every-footer-pct", [call(every, False, "", 0, False, False, True, " -> chest (40%)")])
    h("every-case", [call(every.upper(), False, "", 0, True)])
    # percentages in the text
    h("pct-text-1", [call("Monster #%d(10%%), Monster #%d(0.5%%), Monster #%d(0.004%%), Item #%d(100%%), Room %s(x3)" % (
        sorted(mons)[1], sorted(mons)[2], sorted(mons)[3], sorted(items)[2], r1))])
    h("pct-text-pct", [call("Monster #%d(10%%), Monster #%d(0.5%%), Monster #%d(0.004%%), Item #%d(100%%), Room %s(x3), Room %s(?%%)" % (
        sorted(mons)[1], sorted(mons)[2], sorted(mons)[3], sorted(items)[2], r1, r2), False, "", 0, False, False, True)])
    h("pct-text-2", [call("Monster #%d(10%%), Monster #%d(0.5%%), Textblock #7(33.3%%)" % (sorted(mons)[1], sorted(mons)[2]),
                          False, "", 0, True)])
    # footer percent multiplied into the row percent (a chest's contents)
    for k, ft in enumerate((" -> chest (5%)", " -> chest (0%)", " (12.5%)", " - Value: 5g (10%) (20%)", " x%)", "%)")):
        h("footer-%d" % k, [call("Monster #%d(10%%), Item #%d, Room %s" % (sorted(mons)[1], sorted(items)[2], r1),
                                 False, "", 0, False, False, True, ft)])
    # chests: an item that holds others opens its own Obtained From below it
    for n in chest:
        h("chest-%d" % n, [call("Item #%d" % n)])
        h("chest-pct-%d" % n, [call("Item #%d(50%%)" % n, False, "", 0, False, False, True)])
        h("chest-2-%d" % n, [call("Item #%d" % n, False, "", 0, True)])
    h("chest-header", [call("Item #%d" % chest[0], False, "From: ")])
    h("chest-footer", [call("Item #%d" % chest[0], False, "", 0, False, False, False, " more")])
    h("chest-limit", [call("Item #%d, Item #%d, Item #%d" % tuple(chest[:3]), False, "", 0, False, False, False, "", 2)])
    # shops
    h("shop-value", [call("Shop #%d" % shop_n, False, "", stocked, False, False, False)], view(0, True))
    h("shop-value-nofilter", [call("Shop #%d" % shop_n, False, "", stocked, False, False, False)], view(1, False))
    h("shop-value-pct", [call("Shop #%d, Shop(sell) #%d, Shop(nogen) #%d" % (shop_n, shop_n, shop_n), False, "", stocked, False,
                              False, True)], view(2, True))
    h("shop-sell-value", [call("Shop(sell) #%d, Shop(nogen) #%d" % (shop_n, shop_n), False, "", stocked)], view(3, True))
    h("shop-nogen-2", [call("Shop(nogen) #%d" % shop_n, False, "", stocked, True)], view(4, True))
    h("shop-noaux", [call("Shop #%d, Shop #%d" % (shop_n, selling))])
    h("shop-none", [call("Shop #0, Shop #99999")])
    h("shop-limit", [call("Shop #%d, Shop #%d, Shop #%d" % (shop_n, selling, sorted(shops)[7]), False, "", stocked, False, False, False, "", 1)])
    # lairs for each data version
    for g in lair_g[:4]:
        h("lair-183-%s" % g, [call("Monster #1, [%s][3]Group(lair): %s, Group: %s" % (g, r1, r2), False, "", 0, True)],
          None, {"nGlobalAttackTypeMME": 1})
    h("lair-183-pct", [call("[%s][3]Group(lair): %s,[%s][5]Group(lair): %s" % (lair_g[0], r1, lair_g[1], r2), False, "", 0, False, False, True)])
    h("lair-183-1col", [call("[%s][3]Group(lair): %s,[%s][5]Group(lair): %s" % (lair_g[0], r1, lair_g[1], r2))])
    for att in (0, 1, 2, 5):
        h("lair-attack-%d" % att, [call("[%s][4]Group(lair): %s" % (lair_g[2], r1), False, "", 0, True)], None, {"nGlobalAttackTypeMME": att})
    h("lair-unknown", [call("[99-99-99][4]Group(lair): %s" % r1, False, "", 0, True)])
    h("lair-regen0", [call("[%s][0]Group(lair): %s" % (lair_g[0], r1), False, "", 0, True)])
    h("lair-regen-big", [call("[%s][32768]Group(lair): %s" % (lair_g[0], r1), False, "", 0, True)])
    h("lair-regen-32767", [call("[%s][32767]Group(lair): %s" % (lair_g[0], r1), False, "", 0, True)])
    h("lair-182", [call("[6]Group(lair): %s,[1]Group(lair): %s" % (r1, r2), False, "", 0, True)], None, {"nNMRVer": 1.82})
    h("lair-182-1col", [call("[6]Group(lair): %s" % r1)], None, {"nNMRVer": 1.82})
    h("lair-17", [call("Group(lair): %s,Group(lair): %s" % (r1, r2), False, "", 0, True)], None, {"nNMRVer": 1.7})
    h("lair-17-pct", [call("Group(lair): %s" % r1, False, "", 0, False, False, True)], None, {"nNMRVer": 1.7})
    h("lair-nomatch", [call("Group(lair): %s,Group: %s" % (r1, r2), False, "", 0, True)])
    h("lair-first", [call("Group(lair): %s" % r1, False, "", 0, True)])
    h("lair-second", [call("a,Group(lair): %s" % r1, False, "", 0, True)])
    h("lair-third", [call("ab,Group(lair): %s" % r1, False, "", 0, True)])
    h("lair-upper", [call("[%s][3]GROUP(LAIR): %s" % (lair_g[0], r1), False, "", 0, True)])
    h("lair-nonum", [call("Group(lair): , Group(lair): x")])
    h("lair-delay-old", [call("[%s][3]Group(lair): %s" % (lair_g[0], r1), False, "", 0, True)], None, {"nNMRVer": 1.83})
    # room commands
    cmd = "Room(pry coffin) 17/278|17/279 (20%%), Room(giveitem 5|pry coffin) 17/2073-2096 (?%%), Room(message 3) 1/2-5, Room(open door) 1/2|1/3"
    h("cmd-1", [call("Room %s, " % r1 + cmd)])
    h("cmd-2", [call("Room %s, " % r1 + cmd, False, "", 0, True)])
    h("cmd-pct", [call("Room %s, " % r1 + cmd, False, "", 0, False, False, True)])
    h("cmd-pct-footer", [call(cmd, False, "", 0, False, False, True, " -> chest (50%)")])
    h("cmd-pct-footer-unknown", [call(cmd, False, "", 0, False, False, True, " (25%)")])
    h("cmd-pct-footer-2", [call(cmd, False, "", 0, True, False, False, " (25%)")])
    h("cmd-only-first", [call("Room(pry coffin) 17/278")])
    h("cmd-hide", [call(cmd, False, "", 0, True)], None, {"bHideRecordNumbers": True})
    h("cmd-cut", [call("Room %s, Room(pry coffin) 17/278|17/279|17/280+" % r1)])
    h("cmd-cut-nul", [call("Room %s, Room(pry coffin) 17/278|17/279|17/280+\u0000" % r1)])
    h("cmd-bad", [call("Room(pry coffin 17/278, Room(x) , Room() 1/2, Room(a) zz")])
    h("cmd-limit", [call("Room %s, " % r1 + cmd, False, "", 0, False, False, False, "", 2)])
    h("cmd-limit-pct", [call(cmd, False, "", 0, False, False, True, "", 1)])
    h("cmd-lower", [call("room(Pry Coffin) 17/278")])
    # truncated and cut strings
    h("plus-more", [call("Monster #%d, Monster #%d+\u0000" % (sorted(mons)[1], sorted(mons)[2]))])
    h("plus-more-footer", [call("Monster #%d, Monster #%d+\u0000" % (sorted(mons)[1], sorted(mons)[2]), False, "", 0, False, False, False, " -> x")])
    h("plus-more-2", [call("Monster #%d, Monster #%d+\u0000" % (sorted(mons)[1], sorted(mons)[2]), False, "", 0, True)])
    h("plus-more-single", [call("Monster #%d+\u0000" % sorted(mons)[1])])
    h("plus-ends", [call("Monster #%d, Monster #%d+" % (sorted(mons)[1], sorted(mons)[2]))])
    h("plus-after-number", [call("Monster #%d, Item #%d+, Room %s" % (sorted(mons)[1], sorted(items)[2], r1))])
    h("plus-pct-col", [call("Monster #%d, Monster #%d+\u0000" % (sorted(mons)[1], sorted(mons)[2]), False, "", 0, False, False, True)])
    # limits
    many = ", ".join("Monster #%d" % m for m in sorted(mons)[:14])
    for lim in (1, 3, 13, 14, 15, 40):
        h("limit-%d" % lim, [call(many, False, "", 0, False, False, False, "", lim)])
    h("limit-2col", [call(many, False, "", 0, True, False, False, "", 4)])
    h("limit-pct", [call(many, False, "", 0, False, False, True, "", 4)])
    h("limit-header", [call(many, False, "x ", 0, False, False, False, "", 4)])
    h("limit-footer", [call(many, False, "", 0, False, False, False, " f", 4)])
    h("limit-nested", [call("Shop #%d, Monster #1, Monster #2, Monster #3" % shop_n, False, "", stocked, False, False, False, "", 2)])
    h("limit-dontsort", [call(many, False, "", 0, False, True, False, "", 3)])
    h("sort-many", [call(many)])
    h("nosort-many", [call(many, False, "", 0, False, True)])
    # headers and the room row
    h("room-header-plain", [call("Room %s, Room %s" % (r1, r2), False, "Hdr")])
    h("room-header-colon", [call("Room %s, Room %s" % (r1, r2), False, "Hdr: ")])
    h("room-aux-header", [call("Room %s" % r1, False, "Shop: ", 7)])
    h("room-aux-noheader", [call("Room %s" % r1, False, "", 7)])
    h("room-aux-pct", [call("Room %s" % r1, False, "Shop: ", 7, False, False, True)])
    h("room-aux-2", [call("Room %s" % r1, False, "Shop: ", 7, True)])
    h("monster-header", [call("Monster #%d" % sorted(mons)[1], False, "(learn) ", 0, True)])
    h("spell-header", [call("Spell #%d, Item #%d, NPC #%d, Textblock #3" % (sorted(spells)[1], sorted(items)[3], sorted(mons)[3]), False, "(learn) ")])
    h("spell-header-pct", [call("Spell #%d, Item #%d, NPC #%d, Textblock #3" % (sorted(spells)[1], sorted(items)[3], sorted(mons)[3]), False, "(learn) ", 0, False, False, True)])
    # unknown records and odd numbers
    h("missing-1", [call("Room 99/99999, Monster #99999, Item #99999, Spell #99999, Textblock #0, NPC #99999")])
    h("missing-2", [call("Room 99/99999, Monster #99999, Item #99999, Spell #99999, NPC #99999", False, "", 0, True)])
    h("zero", [call("Room 0/0, Room 1/0, Room 0/5, Monster #0, Item #0, Spell #0, NPC #0")])
    h("zero-hide", [call("Room 0/0, Monster #0, Item #0")], None, {"bHideRecordNumbers": True})
    h("fraction", [call("Item #3.5, Monster #2.5, Spell #4.49, Textblock #6.5, Room %s." % r1)])
    h("fraction-2", [call("Item #3.5, Monster #2.5", False, "", 0, True)])
    h("overflow", [call("Monster #1, Monster #99999999999, Item #1")])
    h("overflow-room", [call("Room 1/334, Room 99999999999/1, Item #1")])
    h("no-number", [call("Monster #, Item # x, Room x, Spell #(5%)")])
    h("number-then-paren", [call("Monster #5(abc), Item #5(12, Spell #5)")])
    h("paren-open", [call("Monster #%d(50%%" % sorted(mons)[1])])
    h("short", [call("Item")])
    h("short-4", [call("a1/2")])
    h("short-5", [call("Room1")])
    h("empty", [call("")])
    h("clear", [call("Item #1")], {"lvLoc": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}, {"Text": "older", "Tag": "y", "Sub": []}]}})
    h("noclear", [call("Item #1", True)], {"lvLoc": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}, {"Text": "older", "Tag": "y", "Sub": []}]}})
    h("noclear-short", [call("x", True)], {"lvLoc": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}, {"Text": "older", "Tag": "y", "Sub": []}]}})
    h("noclear-pct", [call("Item #1", True, "", 0, False, False, True)], {"lvLoc": {"$recordlv": [{"Text": "old", "Tag": "x", "Sub": []}, {"Text": "older", "Tag": "y", "Sub": []}]}})
    h("repeat", [call("Item #1, Item #1, Item #2")])
    h("two-calls", [call("Item #1, Monster #2", True), call("Room %s, Spell #3" % r1, True)])
    h("two-calls-clear", [call("Item #1, Monster #2", True), call("Room %s, Spell #3" % r1)])
    h("nmr-old-room", [call("Group(lair): %s" % r1, False, "", 0, True)], None, {"nNMRVer": 1.0})
    h("spaces", [call("  Room %s ,  Monster #%d  " % (r1, sorted(mons)[1]))])
    h("same-line-keywords", [call("monster #%d item #%d room %s" % (sorted(mons)[1], sorted(items)[2], r1))])
    h("dot-number", [call("Room 1.5/2, Item #1.2.3")])
    h("one-left-char", [call("xroom %s" % r1)])
    h("room-in-word", [call("bedroom %s" % r1)])
    h("shop-in-room", [call("Shop #%d" % shop_n, False, "", 0, False, False, True)])
    for cid, steps, forms, glb in hand:
        add("hand-" + cid, steps, forms, glb)

    # ---- the table of lair answers: real group indexes, invented fixed numbers
    wanted = set()
    for cs in cases:
        for st in cs["steps"]:
            for m in re.finditer(r"\[([\d\-]+)\]", st["args"][0]):
                wanted.add(m.group(1))
    # also the groups of the lair-bearing Obtained From strings reached through chests and shops: the
    # closure below adds the strings, this adds their groups
    lair_table = {}

    def lair_row(g):
        mobs, exp = lairs.get(g, (0, 0))
        r = {"nMobs": mobs, "nAvgExp": exp}
        if H("dc", g) % 3:
            r["nAvgDmgClearRound"] = H("dcv", g) % 60 + [0, 0.25, 0.5][H("dcf", g) % 3]
        if H("do", g) % 3:
            r["nDamageOut"] = H("dov", g) % 300
        return {k: v for k, v in r.items() if v != 0}

    # ---- closure of the records the cases can reach
    texts = []
    for cs in cases:
        for st in cs["steps"]:
            texts.append(st["args"][0])
            if st["args"][4]:
                texts.append("item #%d" % st["args"][4])
    need = {"room": set(), "mon": set(), "item": set(), "spell": set(), "shop": set()}
    seen_text = set()
    work = list(texts)
    while work:
        t = work.pop().lower()
        if t in seen_text:
            continue
        seen_text.add(t)
        for m in re.finditer(r"\[([\d\-]+)\]", t):
            wanted.add(m.group(1))
        for p in PAIR.finditer(t):
            need["room"].add((int(p.group(1)), int(p.group(2))))
        for m in KEY.finditer(t):
            k, v = m.group(1).lower(), m.group(2)
            if "/" in v:
                continue
            v = int(v)
            if k.startswith("monster") or k.startswith("npc"):
                need["mon"].add(v)
            elif k.startswith("item"):
                if v not in need["item"]:
                    need["item"].add(v)
                    if v in items and items[v][1] == 8:
                        work.append(items[v][4])
            elif k.startswith("spell"):
                need["spell"].add(v)
            elif k.startswith("shop"):
                if v not in need["shop"]:
                    need["shop"].add(v)
                    if v in shops:
                        work.append(shops[v]["assigned"])
    for st_text in list(texts):
        pass
    for t in texts:                    # an aux item is looked up for its value
        pass
    for cs in cases:
        for st in cs["steps"]:
            if st["args"][4]:
                need["item"].add(st["args"][4])
    for g in sorted(wanted):
        lair_table[g] = lair_row(g)

    data = {
        "lairs": lair_table,
        "items": [[n, items[n][0], items[n][1], items[n][2], items[n][3], items[n][4]] for n in sorted(need["item"]) if n in items],
        "monsters": [[n, mons[n][0]] for n in sorted(need["mon"]) if n in mons],
        "spells": [[n, spells[n][0]] for n in sorted(need["spell"]) if n in spells],
        "shops": [[n, shops[n]["type"], shops[n]["markup"], shops[n]["assigned"], shops[n]["stock"]]
                  for n in sorted(need["shop"]) if n in shops],
        "rooms": [[m, r, rooms[(m, r)][0], rooms[(m, r)][1]] for (m, r) in sorted(need["room"]) if (m, r) in rooms],
        "fields": {
            "items": ["Number", "Name", "ItemType", "Price", "Currency", "Obtained From"],
            "monsters": ["Number", "Name"], "spells": ["Number", "Name"],
            "shops": ["Number", "ShopType", "Markup%", "Assigned To", "stock[[Item, Max, Time, Amount, %]]"],
            "rooms": ["Map Number", "Room Number", "Name", "Delay"],
        },
    }

    scn = {
        "family": "locations",
        "description": "Reference rows (FAM-19, stock): GetLocations, AddRoomCmdLocationRow and ExtractRoomActions "
                       "over the stock Obtained From, References, Summoned By, Learned From, Casted By and Assigned To texts.",
        "notes": [
            "Each case is a list of GetLocations calls (arguments written out in order: text, list, bDontClear, sHeader, "
            "nAuxValue, bTwoColumns, bDontSort, bPercentColumn, sFooter, nLimit) on one recording ListView; output 'rows' is "
            "[Text, Tag, cells] per row, a cell [Text, Tag, ForeColor] or its Text alone.",
            "ListView sorting is a skipped feature: SortListView is bound to a recorder, so rows are in the order MME adds "
            "them and the events 'sort_listview' give the row count at each sort request.",
            "GetLairInfo answers from data.lairs (invented numbers on the real group indexes), as in the lairs family.",
            "data.items/monsters/spells/shops/rooms are the stock rows the cases can reach (chests and shops open further "
            "texts, so it is a closure); the Rust test answers its lookups from them.",
        ],
        "database": True,
        "shims": ["lairs"],
        "tolerance": {"default": 0},
        "globals": {"nNMRVer": 1.83, "bHideRecordNumbers": False, "nGlobalAttackTypeMME": 0},
        "forms": {"frmMain": {"lvLoc": {"$recordlv": []}, "chkGlobalFilter": {"Value": 0},
                              "txtCharStats": {"$array": {"5": {"Tag": "0"}}}}},
        "read": {"rows": "%lv.lvLoc"},
        "data": data,
        "cases": cases,
    }
    with open(OUT, "w") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"))
        f.write("\n")
    print("cases", len(cases), "bytes", os.path.getsize(OUT), file=sys.stderr)


if __name__ == "__main__":
    main()

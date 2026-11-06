"""Writes fixtures/scenarios/item_detail.json: python3 -I tools/oracle/gen/item_detail.py fixtures/scenarios/item_detail.json
Item detail (card P3-35): modMain PullItemDetail with CompareArmor and CompareWeapons. A `rows` step seeks each item, runs
PullItemDetail against the detail text box (control txtDetail) and a recording ListView, and captures the text and the events
(ListView sort requests, handled errors) per item. Cases: no character (plain, full details, combat options, every attack kind,
display options, error edges), two characters (the pipeline scenario's ladder warrior and mage, with a weapon, rings and wrists
equipped) and compare loadouts (nEquippedItem set, global filter off). Deterministic (sha256 selection)."""
import glob
import hashlib
import json
import os
import re
import sqlite3
import sys

OUT = sys.argv[1]
PIPE = json.load(open(os.path.join(os.path.dirname(OUT), "pipeline.json")))
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
shops = {r["Number"]: dict(r) for r in con.execute("select * from Shops order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}
monsters = {r["Number"]: r["Name"] or "" for r in con.execute("select Number, Name from Monsters")}
tbinfo = {r["Number"]: r["Action"] or "" for r in con.execute("select Number, Action from TBInfo")}
lairs = {g: (m, e) for g, m, e in con.execute("select GroupIndex, Mobs, AvgExp from Lairs")}
rooms = {(m, r): (n or "", d) for m, r, n, d in con.execute('select "Map Number","Room Number",Name,Delay from Rooms')}


def ab(row, n):
    return [(row["Abil-%d" % i], row["AbilVal-%d" % i]) for i in range(n) if row["Abil-%d" % i] != 0]


NAMED = [n for n, r in items.items() if r["Name"]]
WEAPONS = [n for n in NAMED if items[n]["ItemType"] == 1]
ARMOURS = [n for n in NAMED if items[n]["ItemType"] == 0]


def sig(n):
    r = items[n]
    s = {("type", r["ItemType"])}
    if r["ItemType"] == 0:
        s.add(("worn", r["Worn"]))
    if r["ItemType"] == 1:
        s.add(("wt", r["WeaponType"]))
    for a, v in ab(r, 20):
        s.add(("abil", a))
    s.add(("neg", any(r["NegateSpell-%d" % i] for i in range(10))))
    s.add(("race", any(r["RaceRest-%d" % i] for i in range(10))))
    s.add(("cls", any(r["ClassRest-%d" % i] for i in range(10))))
    s.add(("uses", r["UseCount"] > 0, r["Retain After Uses"]))
    s.add(("flags", r["Gettable"], r["Not Droppable"], r["Destroy On Death"]))
    s.add(("nab", min(len(ab(r, 20)), 6)))
    s.add(("ob", bool(r["Obtained From"]), bool(r["References"])))
    return s


def choose(count):
    """Greedy cover of the signatures (rarest first), then a hash-ordered fill."""
    cover = {}
    for n in NAMED:
        for s in sig(n):
            cover.setdefault(s, []).append(n)
    chosen = []
    for s in sorted(cover, key=lambda s: (len(cover[s]), repr(s))):
        if any(s in sig(n) for n in chosen):
            continue
        chosen.append(sorted(cover[s], key=lambda n: H("pick", s, n))[0])
    for n in sorted(NAMED, key=lambda n: H("fill", n)):
        if len(chosen) >= count:
            break
        if n not in chosen:
            chosen.append(n)
    return sorted(chosen)


SEL = choose(150)


def spell_effects(sn, depth=0):
    """Ability numbers a cast spell reaches (its own, and through EndCast / learn / give-spell abilities)."""
    sp = spells.get(sn)
    if sp is None or depth > 3:
        return set()
    out = set()
    for a, v in ab(sp, 10):
        out.add(a)
        if a in (42, 43, 151, 153, 160) and v:
            out |= spell_effects(v, depth + 1)
    return out


# items whose spells add reference rows of every kind (teleport, summon, textblock, nested casts)
for want in (140, 12, 148, 151, 160, 42, 122):
    hits = [n for n in NAMED if any(a == 43 and want in spell_effects(v) for a, v in ab(items[n], 20)) or
            any(a == want for a, v in ab(items[n], 20))]
    for n in sorted(hits, key=lambda n: H("eff", want, n))[:4]:
        if n not in SEL:
            SEL.append(n)
SEL = sorted(SEL)
SEL_W = [n for n in SEL if items[n]["ItemType"] == 1]
EXTRA_W = sorted(sorted(WEAPONS, key=lambda n: H("w", n))[:30])
ALL_W = sorted(set(SEL_W) | set(EXTRA_W))


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


SETUP_FORMS = json.loads(json.dumps(PIPE["forms"]))
SETUP_FORMS["frmMain"].update({
    "chkGlobalFilter": {"Value": 0},
    "chkWeaponOptions": arr({0: {"Value": 5}, 3: {"Value": 0}, 4: {"Value": 0}}),
    "txtWeaponExtras": arr({2: {"Text": "0"}, 3: {"Text": "0"}, 4: {"Text": "0"}}),
    "cmbWeaponCombos": arr({1: {"ListIndex": 0, "ItemData": [5]}}),
    "txtCharStats": arr({0: {"Tag": "0"}, 5: {"Tag": "0"}}),
    "txtDetail": {"Text": ""},
    "lblInvenCharStat": arr({10: {"Tag": "0"}, 46: {}}),
})
READ = {"charm_tag": "%c.txtCharStats(5).Tag", "strength_tag": "%c.txtCharStats(0).Tag",
        "accuracy_tag": "%c.lblInvenCharStat(10).Tag", "level_text": "%c.txtGlobalLevel(0).Text"}
cases = []


def rows_step(keys, mode=0, full=False, as_="rows"):
    return {"rows": {"table": "tabItems", "index": "pkItems", "keys": list(keys), "lv": "each",
                     "capture": {"detail": "%c.txtDetail.Text"}, "events": True, "all_rows": True},
            "call": "PullItemDetail", "args": [{"control": "txtDetail"}, {"lv": True}, mode, full], "as": as_}


def case(cid, steps, forms=None, globals_=None):
    c = {"id": cid, "steps": steps}
    if forms:
        c["forms"] = {"frmMain": forms}
    if globals_:
        c["globals"] = globals_
    cases.append(c)


def combat(speed=0, vs=(40, 5, 20), mode=5, idx=0):
    return {"chkWeaponOptions": arr({0: {"Value": 5}, 3: {"Value": 1}, 4: {"Value": speed}}),
            "txtWeaponExtras": arr({2: {"Text": str(vs[0])}, 3: {"Text": str(vs[1])}, 4: {"Text": str(vs[2])}}),
            "cmbWeaponCombos": arr({1: {"ListIndex": idx, "ItemData": [mode], "StrictItemData": True}})}


def stride(seq, n, off=0):
    seq = list(seq)
    return seq if n >= len(seq) else [seq[(off + i * len(seq) // n) % len(seq)] for i in range(n)]


# ---- A. no character
case("nochar-plain", [rows_step(stride(SEL, 40))])
case("nochar-full", [rows_step(SEL, 0, True)])
case("nochar-combat", [rows_step(ALL_W, 0, True)], forms=combat(1))
case("nochar-combat-mode", [rows_step(stride(ALL_W, 12), 7, True)], forms=combat(0, (10, 3, 10), 3))
case("nochar-modes", [rows_step(stride(ALL_W, 7), m, True, "m%d" % m) for m in (1, 2, 3, 4, 6, 7)])
case("nochar-modes-short", [rows_step(stride(ALL_W, 12), m, False, "m%d" % m) for m in (2, 4, 5)])
case("nochar-hide", [rows_step(stride(SEL, 25), 0, True)], globals_={"bHideRecordNumbers": True})
case("nochar-old-nmr", [rows_step(stride(SEL, 25), 0, True)], globals_={"nNMRVer": 1.6})
case("nochar-quick-spell", [rows_step(stride(SEL, 40, 3), 0, True)], globals_={"bQuickSpell": True})
case("nochar-shops-first", [rows_step(stride(SEL, 20), 0, False)], forms={"mnuShopsFirst": {"Checked": True}})
case("nochar-level-filter", [rows_step(stride(SEL, 30), 0, True)],
     forms={"txtGlobalLevel": arr({0: {"Text": "27"}, 1: {"Text": "27"}})})
case("nochar-combat-no-selection", [rows_step(stride(ALL_W, 4), 0, True)], forms=combat(0, (1, 1, 1), 5, -1))
for tag, vs in (("overflow", ("3000000000", "0", "0")), ("round", ("2.5", "3.5", "-0.5"))):
    case("nochar-extras-" + tag, [rows_step(stride(ALL_W, 5), 0, True)], forms=combat(0, vs, 5))

# ---- B. compare loadouts, global filter off
ARM_BY_WORN = {}
for n in ARMOURS:
    ARM_BY_WORN.setdefault(items[n]["Worn"], []).append(n)


def slot_pick(worn, k):
    lst = ARM_BY_WORN.get(worn) or ARMOURS
    return sorted(lst, key=lambda n: H("slot", worn, k, n))[0]


def loadout(k):
    eq = [0] * 20
    for slot, worn in ((0, 2), (1, 15), (2, 8), (3, 7), (4, 11), (5, 6), (6, 14), (7, 14), (8, 3), (9, 4), (10, 13),
                       (11, 10), (12, 9), (13, 5), (14, 16), (15, 12), (17, 18), (18, 19), (19, 1)):
        eq[slot] = slot_pick(worn, (k, slot))
    eq[16] = sorted(WEAPONS, key=lambda n: H("lw", k, n))[0]
    return eq


def cmp_keys(k, per=3):
    keys = set()
    for w, lst in ARM_BY_WORN.items():
        keys |= set(sorted(lst, key=lambda n: H("ck", n))[:per])
    keys |= set(sorted(WEAPONS, key=lambda n: H("ckw", n))[:14])
    return sorted(keys)


for k in range(3):
    eq = loadout(k)
    keys = cmp_keys(k)
    case("compare-%d" % k, [rows_step(keys if k == 0 else keys[::3], 0, True)], globals_={"nEquippedItem": eq})
    case("compare-short-%d" % k, [rows_step(keys[::6], 5, False)], globals_={"nEquippedItem": eq})
# the shown item is the equipped one; a half-empty ring/wrist pair; a missing record
eq = loadout(0)
case("compare-self", [rows_step([eq[9], eq[10], eq[6], eq[7], eq[16], eq[0]], 0, True)], globals_={"nEquippedItem": eq})
one = list(eq)
one[9] = 0
one[7] = 0
RINGS = ARM_BY_WORN[4] + ARM_BY_WORN.get(13, [])
WRISTS = ARM_BY_WORN[14]
case("compare-one-ring", [rows_step(stride(RINGS, 10) + stride(WRISTS, 8), 0, True)], globals_={"nEquippedItem": one})
bad = list(eq)
bad[9] = 99999
case("compare-missing-both", [rows_step(stride(RINGS, 4), 0, True)], globals_={"nEquippedItem": bad})
bad2 = list(eq)
bad2[9] = 99999
bad2[10] = 0
case("compare-missing-one", [rows_step(stride(RINGS, 4), 0, True)], globals_={"nEquippedItem": bad2})
bad3 = list(eq)
bad3[16] = 99999
case("compare-missing-weapon", [rows_step(stride(WEAPONS, 4), 0, True)], globals_={"nEquippedItem": bad3})

# ---- C. two characters
PC = {c["id"]: c for c in PIPE["cases"]}
CHARS = [("warrior", "ladder-c1-l50", 0), ("mage", "ladder-c12-l50", 1)]
for who, base, k in CHARS:
    f = json.loads(json.dumps(PC[base]["forms"]["frmMain"]))
    f["txtGlobalLevel"]["$array"]["1"] = {"Text": f["txtGlobalLevel"]["$array"]["0"]["Text"]}
    for combo in ("cmbGlobalClass", "cmbGlobalRace"):      # a loaded combo has entries (GetClassStealth checks ListCount)
        f[combo]["$array"]["0"]["List"] = ["None", "selected"]
    g = json.loads(json.dumps(PC[base]["globals"]))
    eq = list(g["nEquippedItem"])
    mine = loadout(k)
    for slot in (6, 7, 9, 10, 16):
        eq[slot] = mine[slot]
        f["cmbEquip"]["$array"][str(slot)] = {"ItemData": [mine[slot]], "ListIndex": 0}
    g["nEquippedItem"] = eq
    keys = sorted(set(SEL) | set(cmp_keys(k, 2)))

    def char_case(suffix, steps, **more):
        ff = json.loads(json.dumps(f))
        ff.update(more)
        case("%s-%s" % (who, suffix), [{"call": "RefreshAll"}] + steps, forms=ff, globals_=g)
    char_case("full", [rows_step(keys[::3], 0, True)])
    char_case("short", [rows_step(stride(keys, 20), 0, False)])
    char_case("modes", [rows_step(stride(ALL_W, 5, m), m, True, "m%d" % m) for m in (1, 2, 3, 4, 6, 7)])
    char_case("combat", [rows_step(stride(ALL_W, 20), 0, True)], **combat(1, (30, 6, 15), 5))
    char_case("stealth-weapons", [rows_step(stride(ALL_W, 12), 4, True)])

# ---- data for the Rust test: the records the cases can reach
KEY = re.compile(r"(?i)(room |monster #|textblock #|textblock\(rndm\) #|item #|spell #|shop #|shop\(sell\) #|shop\(nogen\) #|"
                 r"group\(lair\): |group: |npc #)(\d+(?:/\d+)?)")
PAIR = re.compile(r"(\d+)/(\d+)")
shown = set()
for c in cases:
    for st in c["steps"]:
        if "rows" in st:
            shown |= set(st["rows"]["keys"])
    for e in c.get("globals", {}).get("nEquippedItem", []):
        shown.add(e)
shown.discard(0)
shown.discard(99999)
need = {"room": set(), "item": set(shown), "shop": set()}
work = []
for n in sorted(shown):
    work += [items[n]["Obtained From"] or "", items[n]["References"] or ""]
seen = set()
wanted_groups = set()
while work:
    t = work.pop().lower()
    if t in seen:
        continue
    seen.add(t)
    for m in re.finditer(r"\[([\d\-]+)\]", t):
        wanted_groups.add(m.group(1))
    for p in PAIR.finditer(t):
        need["room"].add((int(p.group(1)), int(p.group(2))))
    for m in KEY.finditer(t):
        k, v = m.group(1).lower(), m.group(2)
        if "/" in v:
            continue
        v = int(v)
        if k.startswith("item"):
            if v not in need["item"]:
                need["item"].add(v)
                if v in items:
                    work += [items[v]["Obtained From"] or ""]
        elif k.startswith("shop"):
            if v not in need["shop"]:
                need["shop"].add(v)
                if v in shops:
                    work.append(shops[v]["Assigned To"] or "")
                    for i in range(20):
                        it = shops[v]["Item-%d" % i]
                        if it:
                            need["item"].add(it)
# teleport rooms of every spell
for sp in spells.values():
    if any(sp["Abil-%d" % i] == 140 for i in range(10)):
        mp = [sp["AbilVal-%d" % i] for i in range(10) if sp["Abil-%d" % i] == 141]
        if mp:
            for r in set([sp["MinBase"], sp["MaxBase"]] + [sp["AbilVal-%d" % i] for i in range(10) if sp["Abil-%d" % i] == 140]):
                need["room"].add((mp[-1], r))
need["item"] &= set(items)

SPELL_VAL = {42, 43, 122, 153, 151, 160}
need_sp, need_mon = set(), set()
for n in need["item"]:
    r = items[n]
    for a, v in ab(r, 20):
        if a in SPELL_VAL:
            need_sp.add(v)
        if a in (12, 146):
            need_mon.add(v)
    for i in range(10):
        if r["NegateSpell-%d" % i]:
            need_sp.add(r["NegateSpell-%d" % i])
for t in seen:
    for m in KEY.finditer(t):
        v = m.group(2)
        if "/" not in v:
            (need_sp if m.group(1).startswith("spell") else need_mon if m.group(1).startswith(("monster", "npc")) else set()).add(int(v))


def spell_closure():
    work = list(need_sp)
    while work:
        n = work.pop()
        sp = spells.get(n)
        if sp is None:
            continue
        for i in range(10):
            a, v = sp["Abil-%d" % i], sp["AbilVal-%d" % i]
            if a == 0:
                continue
            vals = [v] if v else list(range(sp["MinBase"], min(sp["MaxBase"], sp["MinBase"] + 40) + 1))
            if a in SPELL_VAL:
                for x in vals:
                    if x not in need_sp:
                        need_sp.add(x)
                        work.append(x)
            if a == 12:
                need_mon.update(vals)
            if a == 148:
                for tb in vals:
                    for part in (tbinfo.get(tb) or "").split(":"):
                        if part.startswith("cast "):
                            m = re.match(r"cast (\d+)", part)
                            if m and int(m.group(1)) not in need_sp:
                                need_sp.add(int(m.group(1)))
                                work.append(int(m.group(1)))
                    need_tb.add(tb)


need_tb = set()
spell_closure()


def lair_row(g):
    mobs, exp = lairs.get(g, (0, 0))
    r = {"nMobs": mobs, "nAvgExp": exp}
    if H("dc", g) % 3:
        r["nAvgDmgClearRound"] = H("dcv", g) % 60 + [0, 0.25, 0.5][H("dcf", g) % 3]
    if H("do", g) % 3:
        r["nDamageOut"] = H("dov", g) % 300
    return {k: v for k, v in r.items() if v != 0}


ITEM_COLS = ["Number", "Name", "Limit", "Encum", "ItemType", "UseCount", "Price", "Currency", "Min", "Max", "ArmourClass",
             "DamageResist", "WeaponType", "ArmourType", "Worn", "Accy", "Gettable", "StrReq", "Speed", "Not Droppable",
             "Destroy On Death", "Retain After Uses", "Obtained From", "References"]


def item_row(n):
    r = items[n]
    nz = lambda k: [r["%s-%d" % (k, i)] for i in range(10) if r["%s-%d" % (k, i)]]
    if n not in shown:        # only reached through a reference text: what the reference rows read
        return [r[c] if r[c] is not None else "" for c in ITEM_COLS[:2] + ITEM_COLS[4:5] + ITEM_COLS[6:8] + ITEM_COLS[22:23]]
    return [r[c] if r[c] is not None else "" for c in ITEM_COLS] + [nz("ClassRest"), nz("RaceRest"), nz("NegateSpell"),
                                                                  [list(p) for p in ab(r, 20)]]


SPELL_COLS = ["Number", "Name", "Short", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists",
              "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc",
              "DurIncLVLs", "DurInc", "Learnable"]
CLASS_COLS = ["Number", "Name", "MinHits", "MaxHits", "ExpTable", "MageryType", "MageryLVL", "WeaponType", "ArmourType", "CombatLVL"]
RACE_COLS = ["Number", "Name", "mSTR", "mINT", "mWIL", "mAGL", "mHEA", "mCHM", "xSTR", "xINT", "xWIL", "xAGL", "xHEA", "xCHM",
             "HPPerLVL", "ExpTable", "BaseCP"]
tb_need = sorted({sp["AbilVal-%d" % i] for sp in spells.values() for i in range(10)
                  if sp["Abil-%d" % i] == 148 and sp["AbilVal-%d" % i] in tbinfo})
DATA = {
    "item_fields": ITEM_COLS + ["ClassRest(nonzero)", "RaceRest(nonzero)", "NegateSpell(nonzero)", "Abilities(ability,value)"],
    "item_fields_short": ["Number", "Name", "ItemType", "Price", "Currency", "Obtained From"],
    "items": [item_row(n) for n in sorted(need["item"])],
    "spell_fields": SPELL_COLS + ["Abilities(ability,value)"],
    "spells": [[spells[n][c] if spells[n][c] is not None else "" for c in SPELL_COLS] + [[list(p) for p in ab(spells[n], 10)]]
               for n in sorted(need_sp) if n in spells],
    "class_fields": CLASS_COLS + ["Abilities(ability,value)"],
    "classes": [[classes[n][c] for c in CLASS_COLS] + [[list(p) for p in ab(classes[n], 10)]] for n in sorted(classes)],
    "race_fields": RACE_COLS + ["Abilities(ability,value)"],
    "races": [[races[n][c] for c in RACE_COLS] + [[list(p) for p in ab(races[n], 10)]] for n in sorted(races)],
    "monster_names": [[n, monsters[n]] for n in sorted(need_mon) if n in monsters],
    "textblocks": [[n, tbinfo[n]] for n in sorted(need_tb) if n in tbinfo],
    "shops": [[n, shops[n]["ShopType"], shops[n]["Markup%"], shops[n]["Assigned To"] or "",
               [[shops[n]["Item-%d" % x], shops[n]["Max-%d" % x], shops[n]["Time-%d" % x], shops[n]["Amount-%d" % x],
                 shops[n]["%%-%d" % x]] for x in range(20) if shops[n]["Item-%d" % x]]] for n in sorted(need["shop"]) if n in shops],
    "rooms": [[m, r, rooms[(m, r)][0], rooms[(m, r)][1]] for (m, r) in sorted(need["room"]) if (m, r) in rooms],
    "lairs": {g: lair_row(g) for g in sorted(wanted_groups)},
}

scn = {
    "family": "item_detail",
    "description": "Item detail (FAM-20): PullItemDetail with CompareArmor and CompareWeapons: the detail text, the compare "
                   "blocks and the reference list rows of ~150 stratified stock items.",
    "notes": [
        "Each `rows` step seeks every key in tabItems and runs PullItemDetail(txtDetail, recording ListView, attack kind, full "
        "details); outputs '<as>' (the list rows [Text, Tag, cells]) and '<as>.capture' (per key: detail = the text box, events = "
        "the ListView sort requests and handled errors of that call).",
        "ListView sorting is a skipped feature: SortListView, LV_RefreshSort and LV_RefreshSort_ShopsFirst are recorders "
        "(events sort_listview and lv_refresh_sort).",
        "Characters are the pipeline scenario's ladder warrior and mage with a weapon, rings and wrists added to the gear; "
        "RefreshAll runs once per case. Compare cases set nEquippedItem with the global filter off.",
        "GetLairInfo answers from data.lairs (invented numbers on real group indexes), as in the locations family.",
        "data holds the records the cases can reach (items, every spell, class, race and monster name, shops, rooms, textblocks).",
    ],
    "database": True,
    "shims": ["lairs"],
    "frm": PIPE["frm"],
    "data": DATA,
    "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0, "bHideRecordNumbers": False, "bQuickSpell": False},
    "forms": SETUP_FORMS,
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d bytes, %d selected items" % (len(cases), os.path.getsize(OUT), len(SEL)), file=sys.stderr)

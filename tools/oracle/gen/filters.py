"""Writes fixtures/scenarios/filters.json: python3 -I tools/oracle/gen/filters.py fixtures/scenarios/filters.json
List filters (card P3-29): frmMain FilterWeapons, FilterArmour, FilterSundry, FilterInvenItems and FilterAll, cut out of the
form with the helpers they call (ItemIsUsableByChar, InvenAddEquip, InvenSetupEquip, LoadInvenItems, ResetFilterOptions).
The lists they fill (`lvWeapons`, `lvArmour`, `lvOtherItems`) are recording ListViews and the inventory combos (`cmbEquip`)
record what InvenAddEquip adds; the sort and the click handlers that follow are stubbed, so a list is read in the order MME
added the rows ([record number, attack kind] per row). Characters are the pipeline scenario's (RefreshAll runs first); the
cases without one have the global filter's controls set by hand. Deterministic (splitmix64). The tables the Rust test needs
are written once into the scenario's "data" as field lists."""
import copy
import glob
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
PIPE = json.load(open(os.path.join(os.path.dirname(OUT), "pipeline.json")))
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

items = {r["Number"]: dict(r) for r in con.execute("select * from Items order by Number")}
spells = {r["Number"]: dict(r) for r in con.execute("select * from Spells order by Number")}
classes = {r["Number"]: dict(r) for r in con.execute("select * from Classes order by Number")}
races = {r["Number"]: dict(r) for r in con.execute("select * from Races order by Number")}
monsters = {r["Number"]: r["Name"] for r in con.execute("select Number, Name from Monsters order by Number")}
tbinfo = {r["Number"]: r["Action"] for r in con.execute("select Number, Action from TBInfo order by Number")}


def ab(row, n):
    return [[row["Abil-%d" % i], row["AbilVal-%d" % i]] for i in range(n) if row["Abil-%d" % i] != 0]


def arr(d):
    return {"$array": {str(k): v for k, v in d.items()}}


def stride(seq, n, offset=0):
    """About n members spread evenly over seq."""
    seq = list(seq)
    if n >= len(seq):
        return seq
    return [seq[(offset + (i * len(seq)) // n) % len(seq)] for i in range(n)]


WEAPONS = [n for n, r in items.items() if r["ItemType"] == 1]
ARMOURS = [n for n, r in items.items() if r["ItemType"] == 0]
ABIL_WEAPONS = lambda a: [n for n in WEAPONS if any(items[n]["Abil-%d" % x] == a for x in range(20))]

# ---------------------------------------------------------------------------------- the form code under test
FRM = copy.deepcopy(PIPE["frm"])
FRM["procs"] = FRM["procs"] + ["FilterWeapons", "FilterArmour", "FilterSundry", "FilterInvenItems", "FilterAll", "ResetFilterOptions",
                               "InvenAddEquip", "InvenSetupEquip", "LoadInvenItems"]
# ResetFilterOptions is the real one here (FilterAll calls it); the rest of the pipeline's display helpers stay stubs
FRM["stub_procs"] = [p for p in FRM["stub_procs"] if p != "ResetFilterOptions"] + [
    "LV_RefreshSort", "LV_Sort_ColumnClick", "lvWeapons_ItemClick", "lvArmour_ItemClick", "lvOtherItems_ItemClick",
    "ExpandCombo", "LoadResPicture", "LockWindowUpdate", "FilterSpells", "cmbGlobalClass_Click"]
FRM["stub_false"] = ["FormIsLoaded"]
FRM["me_is_form"] = True
FRM["extra"] = ["Public Enum ListDataType", "ldtstring = 0", "ldtnumber = 1", "ldtdate = 2", "End Enum", "Public Const vbResBitmap = 0", "Public Const vbDefaultButton1 = 0", "Public Const vbDefaultButton2 = 256"]
FRM["decls"] = FRM["decls"] + ["bAllowForceWeaponCombatSort", "bMapCancelFind"]
# the pipeline's own RefreshAll is the real one: the cases that need a character call it as the first step, and FilterAll's
# call is stubbed through a rename (see `RefreshAll` in the cuts below)

ABIL_IDS = [43, 22, 58, 135, 136, 59, 116, 105, 28, 142, 114, 1114, 4, 2, 106, 98, 97, 112, 110, 111, 113, 68, 51]
ABIL_DATA = [0, -1] + ABIL_IDS          # cmbXAbilityList.ItemData: "Any", "Negates (= Spell#)", then abilities
ABIL_TEXT = ["Any", "Negates (= Spell#)"] + ["abil%d" % a for a in ABIL_IDS]
WORN_DATA = list(range(0, 19))           # cmbArmourWorn.ItemData(i): "Anywhere" is 0, then 1..18 (Nowhere .. Everywhere)
WORN_TEXT = ["Anywhere"] + ["worn%d" % k for k in range(1, 19)]
COMBO_DATA = [5, 4, 6, 7, -1, -2]        # cmbWeaponCombos(1).ItemData: normal, backstab, bash, smash, martial arts, max effort


def abil_combo(choice):
    """cmbXAbilityList with the entry whose ItemData is `choice` selected (0 = Any, -1 = Negates)."""
    k = ABIL_DATA.index(choice)
    return {"ItemData": ABIL_DATA, "ListIndex": k}


def base_forms():
    f = copy.deepcopy(PIPE["forms"]["frmMain"])
    f.update({
        "chkGlobalFilter": {"Value": 0},
        "chkHanded": arr({k: {"Value": 1} for k in range(4)}),
        "chkWeaponOptions": arr({0: {"Value": 0}, 1: {"Value": 0}, 2: {"Value": 1}, 3: {"Value": 0}, 4: {"Value": 0}}),
        "txtWeaponSpeed": {"Text": "999999"},
        "cmbWeaponCombos": arr({0: {"ListIndex": 0}, 1: {"ListIndex": 0, "ItemData": COMBO_DATA}}),
        "cmbWeaponAbilityList": abil_combo(0),
        "cmbWeaponAbilityOp": {"ListIndex": 1},
        "txtWeaponAbilityVal": {"Text": "1"},
        "txtWeaponExtras": arr({0: {"Text": "999"}, 1: {"Text": "0"}, 2: {"Text": "0"}, 3: {"Text": "0"}, 4: {"Text": "0"}}),
        "chkArmourType": arr({k: {"Value": 1} for k in range(7)}),
        "chkArmourNoLimit": {"Value": 0},
        "chkArmourNonMagic": {"Value": 0},
        "cmbArmourWorn": {"ItemData": WORN_DATA, "ListIndex": 0},
        "cmbArmorAbilityList": abil_combo(0),
        "cmbArmorAbilityOp": {"ListIndex": 1},
        "txtArmorAbilityVal": {"Text": "1"},
        "cmbSundryAbilityList": abil_combo(0),
        "cmbSundryAbilityOp": {"ListIndex": 1},
        "txtSundryAbilityVal": {"Text": "1"},
        "txtGlobalLevel": arr({0: {"Text": "999"}, 1: {"Text": "999"}}),
        "cmbGlobalClass": arr({0: {"ListIndex": 0, "ItemData": [0]}}),
        "cmbGlobalRace": arr({0: {"ListIndex": 0, "ItemData": [0]}}),
        "cmbGlobalAlignment": {"ListIndex": 0},
        "txtGlobalMinLVL": {"Text": "0"},
        "lvWeapons": {"$recordlv": []},
        "lvArmour": {"$recordlv": []},
        "lvOtherItems": {"$recordlv": []},
        "cmbEquip": arr({k: {} for k in range(20)}),
        "cmdNav": arr({k: {} for k in range(8)}),
        "cmdFilter": arr({k: {} for k in range(8)}),
    })
    return f


READ = {
    "weapons": "%lvn.lvWeapons", "armour": "%lvn.lvArmour", "sundry": "%lvn.lvOtherItems",
    "equip": "%list.cmbEquip[0..19]",
    "nav": "%c.cmdNav(0).Caption", "nav_armour": "%c.cmdNav(1).Caption", "nav_sundry": "%c.cmdNav(7).Caption",
    "nav_eq": "%c.cmdNav(4).Caption", "prompt_save": "%g.bPromptSave",
}

cases = []


def case(cid, steps, forms=None, globals_=None, msgbox=None, read=None):
    c = {"id": cid, "steps": steps}
    if forms:
        c["forms"] = {"frmMain": forms}
    g = {"bOnlyInGame": True}
    g.update(globals_ or {})
    c["globals"] = g
    if msgbox:
        c["msgbox"] = msgbox
    if read:
        c["read"] = read
    cases.append(c)


def call(proc, *args):
    return {"call": proc, "args": list(args)}


# ---------------------------------------------------------------------------------- scenario helpers
NEGATES = []
for n in items:
    for k in range(10):
        v = items[n]["NegateSpell-%d" % k]
        if v and v not in NEGATES:
            NEGATES.append(v)
NEGATES = NEGATES[:6]
CLASS_NUMS = sorted(classes)
PC = {c["id"]: c for c in PIPE["cases"]}


def combo0(ds, k):
    return arr({0: {"ListIndex": k, "ItemData": ds}})


def glob_forms(cls=0, level=999, align=0, minlvl=0, on=0):
    """The global filter's controls set by hand (no loaded character)."""
    ds = [0] + CLASS_NUMS
    return {"chkGlobalFilter": {"Value": on},
            "cmbGlobalClass": combo0(ds, 0 if cls == 0 else ds.index(cls)),
            "txtGlobalLevel": arr({0: {"Text": str(level)}, 1: {"Text": str(level)}}),
            "cmbGlobalAlignment": {"ListIndex": align}, "txtGlobalMinLVL": {"Text": str(minlvl)}}


def wopt(**kw):
    """Weapon controls: limit, speed, handed(list of 4 0/1), str, mindmg, magic, nonmagic, bsonly, ability, op, val, calc, slow, mode, vs."""
    f = {}
    if "limit" in kw or "nonmagic" in kw or "bsonly" in kw or "calc" in kw or "slow" in kw:
        f["chkWeaponOptions"] = arr({0: {"Value": kw.get("nonmagic", 0)}, 1: {"Value": kw.get("bsonly", 0)},
                                     2: {"Value": kw.get("limit", 1)}, 3: {"Value": kw.get("calc", 0)}, 4: {"Value": kw.get("slow", 0)}})
    if "speed" in kw:
        f["txtWeaponSpeed"] = {"Text": str(kw["speed"])}
    if "handed" in kw:
        f["chkHanded"] = arr({k: {"Value": v} for k, v in enumerate(kw["handed"])})
    if "str" in kw or "mindmg" in kw:
        f["txtWeaponExtras"] = arr({0: {"Text": str(kw.get("str", 999))}, 1: {"Text": str(kw.get("mindmg", 0))},
                                    2: {"Text": str(kw.get("vs", (0, 0, 0))[0])}, 3: {"Text": str(kw.get("vs", (0, 0, 0))[1])},
                                    4: {"Text": str(kw.get("vs", (0, 0, 0))[2])}})
    elif "vs" in kw:
        f["txtWeaponExtras"] = arr({0: {"Text": "999"}, 1: {"Text": "0"}, 2: {"Text": str(kw["vs"][0])}, 3: {"Text": str(kw["vs"][1])},
                                    4: {"Text": str(kw["vs"][2])}})
    if "magic" in kw:
        w = f.get("cmbWeaponCombos", arr({0: {"ListIndex": 0}, 1: {"ListIndex": 0, "ItemData": COMBO_DATA}}))
        w["$array"]["0"]["ListIndex"] = kw["magic"]
        f["cmbWeaponCombos"] = w
    if "mode" in kw:
        w = f.get("cmbWeaponCombos", arr({0: {"ListIndex": 0}, 1: {"ListIndex": 0, "ItemData": COMBO_DATA}}))
        w["$array"]["1"]["ListIndex"] = COMBO_DATA.index(kw["mode"])
        f["cmbWeaponCombos"] = w
    if "ability" in kw:
        f["cmbWeaponAbilityList"] = abil_combo(kw["ability"])
    if "op" in kw:
        f["cmbWeaponAbilityOp"] = {"ListIndex": kw["op"]}
    if "val" in kw:
        f["txtWeaponAbilityVal"] = {"Text": str(kw["val"])}
    return f


def aopt(**kw):
    f = {}
    if "types" in kw:
        f["chkArmourType"] = arr({k: {"Value": v} for k, v in enumerate(kw["types"])})
    if "nolimit" in kw:
        f["chkArmourNoLimit"] = {"Value": kw["nolimit"]}
    if "nonmagic" in kw:
        f["chkArmourNonMagic"] = {"Value": kw["nonmagic"]}
    if "worn" in kw:
        f["cmbArmourWorn"] = {"ItemData": WORN_DATA, "ListIndex": kw["worn"]}
    if "ability" in kw:
        f["cmbArmorAbilityList"] = abil_combo(kw["ability"])
    if "op" in kw:
        f["cmbArmorAbilityOp"] = {"ListIndex": kw["op"]}
    if "val" in kw:
        f["txtArmorAbilityVal"] = {"Text": str(kw["val"])}
    return f


def sopt(**kw):
    f = {}
    if "ability" in kw:
        f["cmbSundryAbilityList"] = abil_combo(kw["ability"])
    if "op" in kw:
        f["cmbSundryAbilityOp"] = {"ListIndex": kw["op"]}
    if "val" in kw:
        f["txtSundryAbilityVal"] = {"Text": str(kw["val"])}
    return f


def merged(*parts):
    f = {}
    for p in parts:
        f.update(p)
    return f


R_W = {"weapons": READ["weapons"], "nav": READ["nav"]}
R_A = {"armour": READ["armour"], "nav_armour": READ["nav_armour"]}
R_S = {"sundry": READ["sundry"], "nav_sundry": READ["nav_sundry"]}
R_E = {"equip": READ["equip"], "nav_eq": READ["nav_eq"], "prompt_save": READ["prompt_save"]}


def wcase(cid, forms=None, **kw):
    case(cid, [call("FilterWeapons", *kw.pop("args", []))], forms=forms, read=R_W, **kw)


# ---------------------------------------------------------------------------------- A. weapons, no character
wcase("w-default")
wcase("w-default-all-items", globals_={"bOnlyInGame": False})
wcase("w-remove-filter", wopt(limit=0, speed=1000, handed=[0, 0, 0, 0]), args=[True])
wcase("w-nolimit", wopt(limit=0))
for sp in (1200, 1500, 2000, 0):
    wcase("w-speed-%d" % sp, wopt(speed=sp))
for k in range(4):
    h = [1, 1, 1, 1]
    h[k] = 0
    wcase("w-handed-off-%d" % k, wopt(handed=h))
wcase("w-handed-only-0", wopt(handed=[1, 0, 0, 0]))
wcase("w-handed-none", wopt(handed=[0, 0, 0, 0]))
for st in (0, 30, 60, "abc", "1e2"):
    wcase("w-str-%s" % st, wopt(str=st))
for mg in (1, 3, 5, 8, 15):
    wcase("w-magic-%d" % mg, wopt(magic=mg))
wcase("w-nonmagic", wopt(nonmagic=1))
wcase("w-nonmagic-magic3", wopt(nonmagic=1, magic=3))
wcase("w-bsonly", wopt(bsonly=1))
for a in (43, 22, 58, 135, 116, 105, 28, 142, 114, 1114, 68, 4):
    wcase("w-abil-%d" % a, wopt(ability=a))
for op, v in ((0, 0), (0, 3), (1, 5), (1, 100), (1, 0)):
    wcase("w-abil-135-op%d-%d" % (op, v), wopt(ability=135, op=op, val=v))
wcase("w-abil-58-val-text", wopt(ability=58, val="2x"))
wcase("w-abil-42-on-bsonly", wopt(ability=116, bsonly=1))
for sp in NEGATES[:4] + [0]:
    wcase("w-negate-%d" % sp, wopt(ability=-1, val=sp))
for md in (10, 40, 80, 400):
    wcase("w-mindmg-%d" % md, wopt(mindmg=md))
for mode in (5, 4, 6, 7):
    wcase("w-combat-%d" % mode, wopt(calc=1, mode=mode, vs=(20, 3, 5)))
    wcase("w-combat-%d-mindmg" % mode, wopt(calc=1, mode=mode, mindmg=60, vs=(20, 3, 5), slow=1))
wcase("w-mindmg-combat-off-vs", wopt(calc=0, mindmg=40, vs=(30, 5, 10)))
wcase("w-combat-bs-ability", wopt(calc=1, mode=4, ability=135))
wcase("w-combat-bs-negate", wopt(calc=1, mode=4, ability=-1, val=NEGATES[0]))
wcase("w-combat-bs-gt", wopt(calc=1, mode=4, magic=0, ability=0, mindmg=30))
wcase("w-combat-bs-ability-43", wopt(calc=1, mode=4, ability=43))
# martial arts and maximum effort (dialog answers: 6 yes, 7 no, 2 cancel)
wcase("w-martial-nochar", wopt(calc=1, mode=-1, vs=(10, 2, 3)))
wcase("w-martial-nochar-slow", wopt(calc=1, mode=-1, slow=1))
for i, ans in enumerate(([6, 6, 6], [7, 7, 7], [6, 7, 6], [2], [7, 2], [6, 6, 2])):
    wcase("w-maxeffort-nochar-%d" % i, wopt(calc=1, mode=-2, mindmg=150 if i != 3 else 0, vs=(10, 2, 3)), msgbox=ans)
wcase("w-maxeffort-mindmg", wopt(calc=1, mode=-2, mindmg=80, vs=(10, 2, 3)), msgbox=[6, 6])
wcase("w-maxeffort-bsonly", wopt(calc=1, mode=-2, bsonly=1), msgbox=[7, 6])
wcase("w-maxeffort-filtered", wopt(calc=1, mode=-2, ability=116), msgbox=[6, 6])

# ---------------------------------------------------------------------------------- B. armour and sundry, no character
def acase(cid, forms=None, use=False, **kw):
    case(cid, [call("FilterArmour", use)], forms=forms, read=R_A, **kw)


acase("a-default")
acase("a-default-all-items", globals_={"bOnlyInGame": False})
acase("a-remove-filter", aopt(types=[0] * 7, nolimit=1), use=False)
acase("a-nolimit", aopt(nolimit=1))
acase("a-nonmagic", aopt(nonmagic=1))
for w in range(1, 19):
    acase("a-worn-%d" % w, aopt(worn=w))
for k in range(7):
    t = [1] * 7
    t[k] = 0
    acase("a-type-off-%d" % k, aopt(types=t))
acase("a-type-only-3", aopt(types=[0, 0, 0, 1, 0, 0, 0]))
acase("a-type-none", aopt(types=[0] * 7))
for a in (28, 22, 58, 135, 136, 59, 105, 106, 4, 2, 98, 97, 112, 110, 111, 113, 51):
    acase("a-abil-%d" % a, aopt(ability=a))
for op, v in ((0, 0), (0, 10), (1, 10), (1, 50), (1, 0)):
    acase("a-abil-135-op%d-%d" % (op, v), aopt(ability=135, op=op, val=v))
for sp in NEGATES[:4] + [0]:
    acase("a-negate-%d" % sp, aopt(ability=-1, val=sp))
acase("a-combined", merged(aopt(worn=11, types=[1, 1, 1, 1, 1, 0, 0], nolimit=1, nonmagic=1)))


def scase(cid, forms=None, **kw):
    case(cid, [call("FilterSundry")], forms=forms, read=R_S, **kw)


scase("s-default")
scase("s-default-all-items", globals_={"bOnlyInGame": False})
for a in (43, 22, 135, 28, 4, 2, 68, 114):
    scase("s-abil-%d" % a, sopt(ability=a))
for op, v in ((0, 0), (0, 5), (1, 5), (1, 100)):
    scase("s-abil-4-op%d-%d" % (op, v), sopt(ability=4, op=op, val=v))
for sp in NEGATES[:4] + [0]:
    scase("s-negate-%d" % sp, sopt(ability=-1, val=sp))

# ---------------------------------------------------------------------------------- C. the global filter without a character
combos = []
for cls in [0, 1, 2, 7, 12]:
    for level, align, mn in ((1, 0, 0), (15, 1, 0), (50, 2, 0), (50, 3, 20), (255, 0, 0), (999, 0, 0), (30, 1, 10), (0, 0, 0)):
        combos.append((cls, level, align, mn))
for cls, level, align, mn in combos[::2] + combos[1::6]:
    case("armour-g-c%d-l%d-a%d-m%d" % (cls, level, align, mn), [call("FilterArmour", True)], forms=glob_forms(cls, level, align, mn),
         read=R_A)
for cls, level, align, mn in combos[::3]:
    case("inven-g-c%d-l%d-a%d-m%d" % (cls, level, align, mn), [call("FilterInvenItems", False)],
         forms=glob_forms(cls, level, align, mn), read=R_E)
for cls, level, align, mn in combos[1::13]:
    case("inven-remove-g-c%d-l%d" % (cls, level), [call("FilterInvenItems", False), call("FilterInvenItems", True)],
         forms=glob_forms(cls, level, align, mn), read=R_E)
case("g-all-items-armour", [call("FilterArmour", True)], forms=glob_forms(2, 20, 1, 0), globals_={"bOnlyInGame": False}, read=R_A)
case("g-armour-abilities", [call("FilterArmour", True)], forms=merged(glob_forms(1, 30, 0, 0), aopt(ability=135, op=0, val=20)), read=R_A)
case("g-armour-worn-nonmagic", [call("FilterArmour", True)], forms=merged(glob_forms(12, 50, 3, 0), aopt(worn=11, nonmagic=1)), read=R_A)

# ---------------------------------------------------------------------------------- D. loaded characters
CHAR_IDS = ["ladder-c1-l50", "ladder-c7-l50", "ladder-c12-l50", "ladder-c15-l50", "grid-c2-r9-l50", "grid-c3-r7-l50",
            "grid-c5-r10-l50", "grid-c9-r9-l50", "ladder-c1-l15", "ladder-c12-l255", "ladder-c7-l1"]


def char_forms(base, **more):
    f = base_forms()
    cf = copy.deepcopy(PC[base]["forms"]["frmMain"])
    for k, v in cf.items():
        f[k] = v
    ce = f["cmbEquip"]["$array"]
    for k in range(20):
        ce.setdefault(str(k), {})
    lv = f["txtGlobalLevel"]["$array"]
    f["txtGlobalLevel"] = arr({0: lv["0"], 1: {"Text": lv["0"]["Text"]}})
    f.update(more)
    return f


def char_case(base, suffix, steps, read, msgbox=None, **more):
    case("%s-%s" % (base, suffix), [call("RefreshAll")] + steps, forms=char_forms(base, **more), globals_=PC[base]["globals"],
         read=read, msgbox=msgbox)


ALL3 = dict(R_W, **dict(R_A, **R_E))
STEPS3 = [call("FilterWeapons"), call("FilterArmour", True), call("FilterInvenItems", False)]
for base in CHAR_IDS:
    char_case(base, "plain", STEPS3, ALL3)
for base, full in (("ladder-c1-l50", True), ("ladder-c12-l50", True), ("ladder-c7-l50", False), ("ladder-c15-l50", False)):
    fullread = dict(R_W, full="%lv.lvWeapons") if full else R_W
    char_case(base, "mindlvl", STEPS3, ALL3, txtGlobalMinLVL={"Text": "20"})
    for mode in (5, 4, 6, 7):
        if not full and mode in (6, 7):
            continue
        if base == "ladder-c12-l50" and mode in (6, 7):
            continue
        char_case(base, "weapons-combat-%d" % mode, [call("FilterWeapons")], fullread if mode == 4 and base.endswith("c1-l50") else R_W,
                  **wopt(calc=1, mode=mode, mindmg={5: 120, 4: 60, 6: 160, 7: 200}[mode], vs=(25, 4, 8), slow=int(mode == 6)))
    char_case(base, "martial", [call("FilterWeapons")], fullread if base.endswith("c1-l50") else R_W, msgbox=[6],
              **wopt(calc=1, mode=-1, vs=(10, 2, 3)))
    char_case(base, "maxeffort-yes", [call("FilterWeapons")], R_W, msgbox=[6, 6, 6], **wopt(calc=1, mode=-2, mindmg=45, vs=(10, 2, 3)))
    if base != "ladder-c1-l50":
        continue
    char_case(base, "weapons-align-evil", [call("FilterWeapons")], R_W, cmbGlobalAlignment={"ListIndex": 3})
    char_case(base, "weapons-limit-off-mindmg", [call("FilterWeapons")], R_W, **wopt(limit=0, mindmg=30))
    char_case(base, "weapons-abil-43", [call("FilterWeapons")], fullread, **wopt(ability=43))
    char_case(base, "weapons-abil-135", [call("FilterWeapons")], R_W, **wopt(ability=135, op=0, val=30))
    char_case(base, "martial-no", [call("FilterWeapons")], R_W, msgbox=[7], **wopt(calc=1, mode=-1))
    for i, ans in enumerate(([7, 7, 7], [6, 7, 6], [2], [7, 2])):
        char_case(base, "maxeffort-%d" % i, [call("FilterWeapons")], R_W, msgbox=ans,
                  **wopt(calc=1, mode=-2, mindmg=45 if i != 2 else 0, vs=(10, 2, 3)))
    char_case(base, "armour-worn", [call("FilterArmour", True)], R_A, **aopt(worn=11))
    char_case(base, "armour-nolimit-nonmagic", [call("FilterArmour", True)], R_A, **aopt(nolimit=1, nonmagic=1))
    char_case(base, "armour-no-global", [call("FilterArmour", False)], R_A)
    char_case(base, "inven-remove", [call("FilterInvenItems", True)], R_E)
    char_case(base, "all-on", [call("FilterAll", True)], ALL3)
    char_case(base, "all-off", [call("FilterAll", False)], ALL3,
              **merged(wopt(limit=0, speed=1200, handed=[0, 0, 1, 1]), aopt(nolimit=1, worn=5)))

# ---------------------------------------------------------------------------------- data for the Rust test
# Every item with the fields the filters read; the weapons also with what CalculateAttack and the weapon row read. The
# other tables (spells, classes, races, names, textblocks) the Rust test takes from the rows fixture's data.
ITEM_FIELDS = ["Number", "Name", "Limit", "ItemType", "WeaponType", "ArmourType", "Worn", "StrReq", "Speed", "In Game"]
WEAPON_FIELDS = ["Encum", "Min", "Max", "ArmourClass", "DamageResist", "Accy"]


def item_row(n):
    r = items[n]
    row = [r[k] for k in ITEM_FIELDS]
    if r["ItemType"] != 1:
        row[1] = "x" if r["Name"] else ""      # armour and sundry rows only need to know whether the name is empty
    row += [[r["ClassRest-%d" % k] for k in range(10) if r["ClassRest-%d" % k]],
            [r["NegateSpell-%d" % k] for k in range(10) if r["NegateSpell-%d" % k]], ab(r, 20)]
    row.append([r[k] for k in WEAPON_FIELDS] if r["ItemType"] == 1 else None)
    return row


DATA = {
    "item_fields": ITEM_FIELDS + ["ClassRest(nonzero)", "NegateSpell(nonzero)", "Abilities(ability,value)", "WeaponExtra" + str(WEAPON_FIELDS)],
    "items": [item_row(n) for n in sorted(items)],
}

scn = {
    "family": "filters",
    "description": "List filters: FilterWeapons, FilterArmour, FilterSundry, FilterInvenItems, FilterAll",
    "notes": [
        "Each case runs frmMain's own Filter* procedures (cut out by tools/oracle/frm.py) against the stock database. The lists "
        "(lvWeapons, lvArmour, lvOtherItems) are recording ListViews read before the sort that follows: 'number' or "
        "'number:tag' per row in the order MME added it (weapon tag = attack kind; martial-arts rows are 0 with tags -1..-3, "
        "the surprise punch 0 with tag 4). 'full' holds whole weapon rows. 'equip' is the ItemData of each cmbEquip(0..19) "
        "entry (entry 0 is '(none)').",
        "Controls not named by a case have MME's ResetFilterOptions defaults (the family-wide forms). Characters are the "
        "pipeline scenario's: RefreshAll runs first. Cases without one set the global filter's controls by hand. bOnlyInGame "
        "is true unless a case says otherwise. msgbox lists the answers to MME's MsgBox calls in order (6 yes, 7 no, 2 cancel).",
        "Stubbed: LV_RefreshSort / LV_Sort_ColumnClick and the *_ItemClick handlers (UI after the list is built), FilterSpells "
        "and cmbGlobalClass_Click (FilterAll's other callees, P3-30 and the UI), ExpandCombo and the picture loads.",
        "data holds every item with the fields the filters read (weapons in full); the spells, classes, races, names and "
        "textblocks the weapon rows need are the rows fixture's data.",
    ],
    "database": True,
    "data": DATA,
    "frm": FRM,
    "globals": {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0.0},
    "forms": {"frmMain": base_forms()},
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

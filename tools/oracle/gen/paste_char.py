"""Writes fixtures/scenarios/paste_char.json (FAM-25, character half: frmMain PasteCharacter and friends):
    python3 -I tools/oracle/gen/paste_char.py

Runs frmMain PasteCharacter, PasteInventoryManager, PasteSpells, ApplyPastedStat, PastedStatIsBuffed and
InvenAddEquip (cut from frmMain.frm), the combo and text-box events they rely on, and modMMudFunc
TestPasteChar / TestAlphaChar over the stock database. All pasted texts are invented MajorMUD-style
"stat" / "inventory" / "spells" outputs built from stock item and spell names, never real logs.
Deterministic (splitmix64). Needs the converted stock SQLite (tools/mdb-fetch.sh)."""
import glob
import json
import os
import sqlite3
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "paste_char.json")
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

    def sample(self, xs, n):
        xs = list(xs)
        out = []
        for _ in range(min(n, len(xs))):
            out.append(xs.pop(self.int(0, len(xs) - 1)))
        return out


# ------------------------------------------------------------------------------ stock rows
ITEMS = {r["Number"]: dict(r) for r in con.execute("select * from Items order by rowid")}
SHOPS = {r["Number"]: dict(r) for r in con.execute("select * from Shops order by rowid")}
CLASSES = [dict(r) for r in con.execute("select * from Classes order by rowid")]
RACES = [dict(r) for r in con.execute("select * from Races order by rowid")]
SPELLS = [dict(r) for r in con.execute("select * from Spells order by rowid")]
ROOMS = {(r["Map Number"], r["Room Number"]): r["Name"]
         for r in con.execute('select "Map Number", "Room Number", Name from Rooms')}


def nm(s):
    return (s or "")


def strip_sp(s):
    return s.replace(" ", "")


def ab(r, n=20):
    return [[r["Abil-%d" % i], r["AbilVal-%d" % i]] for i in range(n) if r.get("Abil-%d" % i)]


ALLOWED = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -_:'\".`")


def plain(name):
    """A name the paste scanner passes through whole: letters, digits and a few marks, no brackets or commas."""
    return bool(name) and name == name.strip() and "  " not in name and all(c in ALLOWED for c in name)


# ------------------------------------------------------------------------------ invented rows
def item(n, name, **kw):
    r = {"Number": n, "Name": name, "Encum": 5, "ItemType": 0, "Worn": 0, "Price": 100, "Currency": 0,
         "Gettable": 0, "In Game": 1, "Obtained From": "", "WeaponType": 0, "ArmourType": 0}
    r.update(kw)
    return r


EXTRA_ITEMS = [
    item(9001, "tst eye patch", Worn=18), item(9002, "tst face mask", Worn=19),
    item(9003, "tst all cloak", Worn=1, Encum=11), item(9004, "tst twin", Worn=1, Encum=7),
    item(9005, "tst twin", Worn=16, Encum=3), item(9006, "tst twin2", Worn=16, Encum=2),
    item(9007, "tst twin2", Worn=1, Encum=9), item(9008, "tst glove", Worn=3, Encum=4),
    item(9009, "tst glove", Worn=3, Encum=6), item(9010, "tst ghost", Worn=3, **{"In Game": 0}),
    item(9011, "tst sundry", ItemType=3, Gettable=1, Price=40, **{"Obtained From": "Shop #5"}),
]
for r in EXTRA_ITEMS:
    ITEMS[r["Number"]] = dict(r)


# ------------------------------------------------------------------------------ equipment pools
# text slot -> (cmbEquip index the item lands in, Items.Worn values that belong there)
SLOT_TEXT = {0: "Head", 1: "Ears", 2: "Neck", 3: "Back", 4: "Torso", 5: "Arms", 6: "Wrist", 8: "Hands", 9: "Finger",
             11: "Waist", 12: "Legs", 13: "Feet", 15: "Off-Hand", 16: "Weapon Hand", 17: "Eyes", 18: "Face", 14: "Worn"}
WORN_FOR = {0: 2, 1: 15, 2: 8, 3: 7, 4: 11, 5: 6, 6: 14, 8: 3, 9: 4, 11: 10, 12: 9, 13: 5, 15: 12, 17: 18, 18: 19, 14: 16}


def pool(pred, limit=14, unique=True):
    out, seen = [], set()
    for n, r in ITEMS.items():
        nmv = nm(r["Name"])
        if r["In Game"] and plain(nmv) and pred(r) and strip_sp(nmv) not in seen and n < 9000:
            # only names with exactly one stock row keep the case reading simple
            if not unique or sum(1 for q in ITEMS.values() if strip_sp(nm(q["Name"])) == strip_sp(nmv)) == 1:
                out.append(n)
                seen.add(strip_sp(nmv))
    step = max(1, len(out) // limit)
    return out[::step][:limit]


EQ = {}
for slot, worn in WORN_FOR.items():
    EQ[slot] = pool(lambda r, w=worn: r["ItemType"] == 0 and r["Worn"] == w)
EQ[16] = pool(lambda r: r["ItemType"] == 1, 24)
EQ[17] = [9001]
EQ[18] = [9002]
EVERY = [9003, 9004, 9006]
WORNPOOL = EQ[14]
EQ[14] = EQ[14] + [9005, 9007]
EQ[10] = EQ[9]
EQ[7] = EQ[6]
EQ[19] = EVERY
NOEQ = pool(lambda r: r["ItemType"] not in (0, 1), 24)       # sundries
JUNK = ["wooden spoon of doom", "xyzzy thing", "mystery meat", "bag of holding", "a thing"]
for j in JUNK:
    assert not any(strip_sp(nm(r["Name"])).lower() == strip_sp(j).lower() for r in ITEMS.values())

# an item whose name appears twice with different Worn values (the Worn = 1 / 16 branches)
DUP = {}
for n, r in ITEMS.items():
    if plain(nm(r["Name"])) and r["In Game"]:
        DUP.setdefault(strip_sp(nm(r["Name"])), []).append(n)
DUP = {k: v for k, v in DUP.items() if len(v) > 1}


def real_dup_pair():
    for k, v in sorted(DUP.items()):
        ws = sorted(ITEMS[n]["Worn"] for n in v)
        if 1 in ws or 16 in ws:
            return k, v
    return None


# ------------------------------------------------------------------------------ the combos MME builds
CLASS_LIST = [("Any", 0)] + [(c["Name"], c["Number"]) for c in CLASSES]
RACE_LIST = [("Any", 0)] + [(r["Name"], r["Number"]) for r in RACES]


def combo_spec(entries, index, names=False):
    if names:
        return {"List": [e[0] for e in entries], "ItemData": {str(i): e[1] for i, e in enumerate(entries)},
                "ListIndex": index}
    return {"Items": [e[1] for e in entries], "ListIndex": index}


def class_combo(number):
    idx = next(i for i, e in enumerate(CLASS_LIST) if e[1] == number)
    return {"$array+": {"0": {"ListIndex": idx}, "1": {"ListIndex": idx}}}


def race_combo(number):
    idx = next(i for i, e in enumerate(RACE_LIST) if e[1] == number)
    return {"$array+": {"0": {"ListIndex": idx}}}


def equip_combo(items):
    """One cmbEquip(x): the None entry, then the given item numbers (the first one selected when `sel`)."""
    return [("None", 0)] + [(str(n), n) for n in items]


# ------------------------------------------------------------------------------ text builders
def stat_block(rng, name="Brogan Ironhand", race="Dwarf", cls="Warrior", level=12, stats=(80, 40, 35, 60, 70, 45),
               buffed=(), enc=None, ac=(24, 3), extra=()):
    labels = ["Strength", "Intellect", "Willpower", "Agility", "Health", "Charm"]
    cells = []
    for i, lab in enumerate(labels):
        mark = "*" if i in buffed else ""
        cells.append("%s: %s%s" % (lab, " " * rng.int(0, 3) + str(stats[i]), mark))
    lines = ["Name: %s%sLives/CP: %d/%d" % (name, " " * rng.int(1, 12), rng.int(1, 9), rng.int(0, 40)),
             "Race: %s%sExp: %d" % (race, " " * rng.int(1, 12), rng.int(0, 99999999)),
             "Class: %s%sLevel: %s" % (cls, " " * rng.int(1, 10), level),
             "%s%s%s" % (cells[0], " " * rng.int(2, 6), cells[3]),
             "%s%s%s" % (cells[1], " " * rng.int(2, 6), cells[4]),
             "%s%s%s" % (cells[2], " " * rng.int(2, 6), cells[5]),
             "Hits: %d/%d%sMana: %d/%d" % (rng.int(1, 300), rng.int(1, 300), "   ", rng.int(0, 100), rng.int(0, 100))]
    if ac is not None:
        acs = str(ac[0])
        lines.append("Armour Class:%s%s/%d%sAccuracy: %d" % (" " * max(0, 4 - len(acs)), acs, ac[1], "    ", rng.int(1, 200)))
    lines.append("Perception: %d%sStealth: %d" % (rng.int(1, 90), "   ", rng.int(1, 90)))
    if enc is not None:
        lines.append("Encumbrance: %d/%d - Light [%d%%]" % (enc, enc * 3 + 50, rng.int(1, 99)))
    lines += list(extra)
    return "\n".join(lines)


def equip_line(entries):
    return "You are currently equipped with: " + ", ".join(entries)


def carry_line(entries):
    return "You are carrying " + (", ".join(entries) if entries else "nothing")


# ------------------------------------------------------------------------------ the oracle spec
FRM = [
    {"file": "frmMain.frm",
     "procs": ["PasteCharacter", "PasteInventoryManager", "ApplyPastedStat", "PastedStatIsBuffed", "PasteSpells",
               "InvenAddEquip", "cmbEquip_Click", "cmbGlobalRace_Click", "cmbGlobalClass_Click",
               "txtCharStats_Change", "ItemIsUsableByChar"],
     "decls": ["bDontRefresh", "bAutoSave", "nGlobalRefreshDelay", "sNormalCaption"],
     "subs": {
         "PasteCharacter": {
             "Load frmPasteChar": "'",
             "frmPasteChar.txtText = Trim(Clipboard.GetText)": "frmPasteChar.txtText.Text = Trim(frmMain.txtClip.Text)",
             'If frmPasteChar.txtText = "" Then': 'If frmPasteChar.txtText.Text = "" Then',
             'frmPasteChar.txtText = "Paste the commands below into your game to get your stats." _':
                 'frmPasteChar.txtText.Text = "Paste the commands below into your game to get your stats." _',
             "frmPasteChar.Show vbModal, Me": 'frmPasteChar.Tag = "1"',
             "If chkEquipHold(x).Value = 0 Then cmbEquip(x).ListIndex = 0":
                 "If chkEquipHold(x).Value = 0 Then\ncmbEquip(x).ListIndex = 0\nCall cmbEquip_Click(x)\nEnd If",
             "cmbEquip(x).ListIndex = y":
                 "If cmbEquip(x).ListIndex <> y Then\ncmbEquip(x).ListIndex = y\nCall cmbEquip_Click(x)\nEnd If",
             "cmbGlobalRace(0).ListIndex = x":
                 "If cmbGlobalRace(0).ListIndex <> x Then\ncmbGlobalRace(0).ListIndex = x\nCall cmbGlobalRace_Click(0)\nEnd If",
             "cmbGlobalClass(0).ListIndex = x":
                 "If cmbGlobalClass(0).ListIndex <> x Then\ncmbGlobalClass(0).ListIndex = x\nCall cmbGlobalClass_Click(0)\n"
                 "Call cmbGlobalClass_Click(1)\nEnd If",
             "txtGlobalLevel(0).Text = nStat": "txtGlobalLevel(0).Text = nStat\ntxtGlobalLevel(1).Text = nStat",
         },
         "PasteInventoryManager": {
             "Load frmPasteChar": "'",
             "Call LV_Sort_ColumnClickOrSticky(lvItemManager, lvItemManager.ColumnHeaders(11), ldtnumber, True, False, True)": "'",
             "frmPasteChar.txtText = Clipboard.GetText": "frmPasteChar.txtText.Text = frmMain.txtClip.Text",
             "frmPasteChar.Show vbModal, Me": 'frmPasteChar.Tag = "1"',
         },
         "ApplyPastedStat": {
             "txtCharStats(nStatIndex).Text = nBase":
                 "If txtCharStats(nStatIndex).Text <> CStr(nBase) Then\ntxtCharStats(nStatIndex).Text = nBase\n"
                 "Call txtCharStats_Change(nStatIndex)\nEnd If",
             "txtCharStats(nStatIndex).Text = val(sManualStat)":
                 "If txtCharStats(nStatIndex).Text <> CStr(val(sManualStat)) Then\n"
                 "txtCharStats(nStatIndex).Text = val(sManualStat)\nCall txtCharStats_Change(nStatIndex)\nEnd If",
         },
         "cmbGlobalRace_Click": {
             "If val(txtCharStats(x)) < val(txtCharMaxStats(x).Tag) Then": "If val(txtCharStats(x).Text) < val(txtCharMaxStats(x).Tag) Then",
             "txtCharStats(x) = val(txtCharMaxStats(x).Tag)":
                 "txtCharStats(x).Text = val(txtCharMaxStats(x).Tag)\nCall txtCharStats_Change(x)",
         },
     },
     "subs_all": {
         "PasteCharacter": {"Me.Enabled = False": "'", "Me.Enabled = True": "'", "Me.Caption = sNormalCaption": "'"},
         "PasteInventoryManager": {"Me.Enabled = False": "'", "Me.Enabled = True": "'",
                                   "If Not bExtraInventoryOnly Then Me.Enabled = True": "'"},
         "cmbEquip_Click": {"timRefreshDelay.Enabled = False": "'", "timRefreshDelay.Enabled = True": "'"},
     },
     "stub_procs": ["CalcCharacterStats", "RefreshCPs", "RefreshMagic", "RefreshPicklocks", "RefreshMagicRes",
                    "RefreshHitPoints", "RefreshShops"]},
    {"file": "modListViewExt.bas", "procs": ["ParseActionAndQty"]},
]


# ------------------------------------------------------------------------------ cases
cases = []
used_eq = set()      # space-stripped names the equipment scan may match
used_inv = set()     # lower-case trimmed names the inventory matcher looks up
used_spell = set()
used_numbers = set()
_n = [0]


def add(prefix, **kw):
    kw["id"] = "%s%03d" % (prefix, _n[0])
    _n[0] += 1
    cases.append(kw)
    return kw


def iname(n):
    used_eq.add(strip_sp(ITEMS[n]["Name"]))
    used_inv.add(ITEMS[n]["Name"].strip().lower())
    used_numbers.add(n)
    return ITEMS[n]["Name"]


def spell_lines(names, rng, header=True, levels=None):
    lines = []
    if header:
        lines.append("You have the following spells:")
        lines.append(rng.pick(["Level Mana Short Spell name", "Level Mana  Short  Spell Name"]))
    for nmv in names:
        sp = rng.pick([" ", "  ", "   "])
        lines.append("%s%d%s%d%s%s%s%s" % (sp, rng.int(1, 60), sp, rng.int(0, 30), sp, "abcd"[: rng.int(2, 4)], sp + sp, nmv))
    return "\n".join(lines)


def pick_spells(rng, n, cls=None):
    out = []
    pool_ = [s for s in SPELLS if s["Name"] and s["Short"] and plain(s["Name"]) and len(s["Name"]) > 3]
    for s in rng.sample(pool_, n):
        out.append(s["Name"])
        used_spell.add(s["Name"].strip())
    return out


def forms_for(st):
    """frmMain controls for a character state dict."""
    f = {
        "cmbGlobalClass": class_combo(st.get("cls", 0)), "cmbGlobalRace": race_combo(st.get("race", 0)),
        "txtGlobalLevel": {"$array": {"0": {"Text": str(st.get("lvl", 1))}, "1": {"Text": str(st.get("lvl", 1))}}},
        "chkGlobalFilter": {"Value": 1 if st.get("filter") else 0},
        "txtCharName": {"Text": st.get("name", "")},
        "txtCharStats": {"$array": {str(i): ({"Text": str(v), "Tag": str(v)} if i == 5 else {"Text": str(v)})
                                    for i, v in enumerate(st.get("stats", [10] * 6))}},
        "cmbGlobalAlignment": {"ListIndex": st.get("align", 0)},
        "txtGlobalMinLVL": {"Text": str(st.get("minlvl", 0))},
        "chkUnequipMissing": {"Value": 1 if st.get("unequip") else 0},
        "txtInvenAddWeight": {"Text": str(st.get("addw", 0))},
        "chkInvenAddWeight": {"Value": 1 if st.get("addw_on") else 0},
        "lvItemManager": {"$listview": st.get("rows", [])},
        "txtClip": {"Text": st.get("clip", "")},
        "lblLabelArray": {"$array+": {"3": {"Tag": str(st.get("charm_bonus", 0))}}},
    }
    elems = {}
    for slot, items in st.get("combos", {}).items():
        sel = st.get("cur", {}).get(slot, 0)
        entries = equip_combo(items)
        idx = 0
        if sel:
            idx = 1 + items.index(sel)
        elems[str(slot)] = combo_spec(entries, idx)
    if elems:
        f["cmbEquip"] = {"$array+": elems}
    if st.get("hold"):
        f["chkEquipHold"] = {"$array": {str(s): {"Value": 1} for s in st["hold"]}}
    return f


def globals_for(st):
    eq = [0] * 20
    for slot, n in st.get("cur", {}).items():
        eq[slot] = n
    g = {}
    for key, val in (("nEquippedItem", eq if any(eq) else None), ("bOnlyInGame", st.get("only_in_game")),
                     ("bInvenUse2ndWrist", st.get("wrist2")), ("bCharLoaded", st.get("loaded")),
                     ("bAutoSave", st.get("autosave")), ("bPromptSave", st.get("prompt_save")),
                     ("bHideRecordNumbers", st.get("hide")), ("sSessionLastCharFile", st.get("session_file")),
                     ("nLearnedSpellClass", st.get("lclass")),
                     ("nLearnedSpells", (st["learned"] + [0] * 100)[:100] if st.get("learned") else None)):
        if val:
            g[key] = bool(val) if key[0] == "b" else val      # unset globals start at their VB default
    return g


READS = {
    "eq": "%g.nEquippedItem", "learned": "%gt.nLearnedSpells", "lclass": "%g.nLearnedSpellClass",
    "stats": "%c.txtCharStats.Text[0..5]", "tags": "%c.txtCharStats.Tag[0..5]",
    "lvl": "%c.txtGlobalLevel(0).Text", "lvl1": "%c.txtGlobalLevel(1).Text", "filter": "%c.chkGlobalFilter.Value",
    "name": "%c.txtCharName.Text", "cls": "%c.cmbGlobalClass(0).ListIndex", "cls1": "%c.cmbGlobalClass(1).ListIndex",
    "race": "%c.cmbGlobalRace(0).ListIndex", "addw": "%c.txtInvenAddWeight.Text", "addc": "%c.chkInvenAddWeight.Value",
    "rows": "%lv.lvItemManager", "combos": "%combo.cmbEquip", "session": "%g.sSessionLastCharFile",
    "loaded": "%g.bCharLoaded", "delay": "%g.nGlobalRefreshDelay", "dontrefresh": "%g.bDontRefresh",
    "simac": "%f.frmMonsterAttackSim.txtUserAC.Text", "simdr": "%f.frmMonsterAttackSim.txtUserDR.Text",
}


def run_case(prefix, st, text, steps=None, **kw):
    """One paste: character state `st`, pasted text `text`, then the steps (default PasteCharacter)."""
    st = dict(st)
    st["clip"] = text
    c = {"forms": {"frmMain": forms_for(st)}, "globals": globals_for(st),
         "steps": steps or [{"call": "PasteCharacter"}]}
    if st.get("sim"):
        c["loaded_forms"] = ["frmMonsterAttackSim"]
    c.update(kw)
    return add(prefix, **c)


def reads(*names):
    return {n: READS[n] for n in names}


def build_pure():
    chars = [chr(i) for i in range(32, 127)] + ["\t", "\r", "\n", "\xe9", "ß", "K", "ı"]
    for ch in chars:
        add("t", proc="TestPasteChar", args=[ch], read={})
        add("a", proc="TestAlphaChar", args=[ch], read={})
    labels = ["Strength:", "Intellect:", "Charm:"]
    texts = ["Strength: 80*", "Strength:  80  *", "Strength: 80", "strength:80*", "Strength:*", "Strength:", "Intellect: 12 *x",
             "Charm: 9 9*", "no label", "", "Strength:   ", "Strength: 80\n*", "STRENGTH: 80*  Strength: 70",
             "Strength: * Strength: 5*", "Intellect:\t*"]
    for t in texts:
        for lab in labels:
            add("b", proc="PastedStatIsBuffed", args=[t, lab], read={})


def build_apply():
    rng = Rng(0xA991)
    for i in range(32):
        idx = rng.int(0, 5)
        pasted = rng.pick([0, -3, 1, 2, 45, 80, 120, 32767, 100000])
        bonus = rng.pick([0, 0, 0, 5, -4, 200, 100000])
        buffed = rng.chance(55)
        cur = rng.pick([0, 1, 40, 80, 75, 120])
        src = rng.pick(["", "", "Ring of X (+5)", "Ring (+5), Cloak (+2)"])
        manual = rng.pick(["", "", "SKIP", "OTHER"])
        stats = [10] * 6
        stats[idx] = cur
        answers = rng.pick([[], [], ["77"], ["0"], ["abc"], ["12.7"], [""], ["-4"]])
        st = {"stats": stats, "loaded": False}
        add("p", forms={"frmMain": forms_for(st)}, globals=globals_for(st), inputbox=answers,
            read=reads("stats", "tags"),
            steps=[{"call": "ApplyPastedStat", "args": [idx, "Strength Name", pasted, buffed, bonus, src,
                                                       {"ref": "string", "value": manual}], "as": "x"}])



SLOT_ORDER = [0, 1, 17, 18, 2, 3, 4, 5, 6, 7, 11, 9, 10, 8, 12, 13, 15, 16]
TEXT_OF = {0: "Head", 1: "Ears", 17: "Eyes", 18: "Face", 2: "Neck", 3: "Back", 4: "Torso", 5: "Arms", 6: "Wrist",
           7: "Wrist", 11: "Waist", 9: "Finger", 10: "Finger", 8: "Hands", 12: "Legs", 13: "Feet", 15: "Off-Hand",
           16: "Weapon Hand"}
STAT_RANGE = {1: (35, 90), 2: (40, 110), 3: (30, 100)}


def gen_equipment(rng, n_slots):
    """slot -> item for the equipped line, plus the 'Worn' items."""
    eq = {}
    for slot in rng.sample(SLOT_ORDER, n_slots):
        if slot == 7 and 6 not in eq:
            eq[6] = rng.pick(EQ[6])
        if slot == 10 and 9 not in eq:
            eq[9] = rng.pick(EQ[9])
        if EQ.get(slot):
            eq[slot] = rng.pick(EQ[slot])
    worn = []
    if rng.chance(30):
        worn.append(rng.pick(EQ[14]))
    if rng.chance(12):
        worn.append(rng.pick(EQ[14] + EVERY))
    return eq, worn


def equip_entries(rng, eq, worn, junk=True):
    ents = []
    for slot in SLOT_ORDER:
        if slot in eq:
            sl = TEXT_OF[slot]
            if slot == 16 and rng.chance(15):
                sl = rng.pick(["Two Handed", "Two-Handed", "TwoHanded"])
            if rng.chance(8):
                sl = sl.upper()
            ents.append("%s (%s)" % (iname(eq[slot]), sl))
    for w in worn:
        ents.append("%s (Worn)" % iname(w))
    if junk and rng.chance(25):
        ents.insert(rng.int(0, len(ents)), "%s (%s)" % (rng.pick(JUNK), rng.pick(["Back", "Neck", "Head"])))
    return ents


def random_state(rng, eq, worn):
    st = {"cls": rng.pick([0] + [c["Number"] for c in CLASSES]), "race": rng.pick([0] + [r["Number"] for r in RACES]),
          "lvl": rng.pick([1, 5, 12, 40, 100]), "stats": [rng.int(10, 120) for _ in range(6)],
          "align": rng.pick([0, 1, 2, 3]), "filter": rng.chance(50), "name": rng.pick(["", "Old Name"]),
          "loaded": False, "only_in_game": rng.chance(25), "wrist2": rng.chance(60), "hide": rng.chance(30),
          "unequip": rng.chance(35), "charm_bonus": rng.pick([0, 0, 3, 12])}
    combos, cur = {}, {}
    for slot, n in eq.items():
        lst = [m for m in rng.sample(EQ.get(slot, []), 2) if m != n]
        if rng.chance(55):
            lst.insert(rng.int(0, len(lst)), n)
        if lst:
            combos[slot] = lst
        if lst and rng.chance(30):
            cur[slot] = rng.pick(lst)
        if slot == 6 and 7 in eq:
            pass
    for slot in rng.sample(SLOT_ORDER, rng.int(0, 3)):      # slots the paste does not mention
        if slot not in eq and EQ.get(slot):
            lst = rng.sample(EQ[slot], 2)
            combos[slot] = lst
            if rng.chance(70):
                cur[slot] = lst[0]
    st["combos"], st["cur"] = combos, cur
    if rng.chance(25):
        st["hold"] = rng.sample(sorted(set(list(eq) + list(cur))) or [0], 1)
    return st


def existing_rows(rng, pool_):
    rows = []
    for n in rng.sample(pool_, rng.int(0, 4)):
        r = ITEMS[n]
        iname(n)
        flag = rng.pick(["", "CARRIED", "CARRIED", "CARRIED x2", "carried x3", "CARRIED X4", "STASH", "CARRIED x40000"])
        rows.append({"Text": str(n), "Sub": [r["Name"], flag, str(rng.int(1, 3)), rng.pick(["Inventory", "Equipped"]),
                                              str(r["Encum"])]})
    return rows


def paste_text(rng, st, eq, worn, with_inv=True, with_spells=False, buffed=()):
    cls = next((c["Name"] for c in CLASSES if c["Number"] == st["cls"]), "Warrior")
    race = next((r["Name"] for r in RACES if r["Number"] == st["race"]), "Human")
    parts = []
    enc = rng.int(0, 400) if rng.chance(75) else None
    parts.append(stat_block(rng, name=rng.pick(["Brogan Ironhand", "Zed", "Mira Vale-Strand", "Q'Tor"]), race=race, cls=cls,
                            level=rng.int(1, 120), stats=[rng.int(10, 140) for _ in range(6)], buffed=buffed, enc=enc,
                            ac=rng.pick([None, (24, 3), (118, 44), (7, 0), (1234, 5)])))
    ents = equip_entries(rng, eq, worn)
    if with_inv:
        inv = []
        for n in rng.sample(NOEQ, rng.int(0, 5)):
            nmv = iname(n)
            inv.append(rng.pick([nmv, nmv, "%d %s" % (rng.int(2, 4), nmv), "%s (%d)" % (nmv, rng.int(2, 5))]))
        if rng.chance(30):
            inv.append(rng.pick(JUNK))
        for slot in rng.sample(sorted(eq), min(len(eq), rng.int(0, 1))):    # carried as well as worn
            inv.append(iname(eq[slot]))
        order = rng.pick([0, 1, 2])
        a, b = equip_line(ents), carry_line(inv) + rng.pick(["", "."])
        parts.append(a + "\n" + b if order != 2 else b + "\n" + a)
    else:
        parts.append(equip_line(ents))
    if with_spells:
        parts.append(spell_lines(pick_spells(rng, rng.int(1, 6)), rng))
    if rng.chance(40):
        parts = parts[::-1]
    return "\n".join(parts)


def build_full():
    rng = Rng(0xF011)
    for i in range(50):
        eq, worn = gen_equipment(rng, rng.int(0, 9))
        st = random_state(rng, eq, worn)
        st["rows"] = existing_rows(rng, NOEQ + [n for s_ in eq for n in [eq[s_]]])
        if rng.chance(30):
            st["loaded"] = True
            st["prompt_save"] = rng.chance(60)
            st["autosave"] = rng.chance(40)
            st["session_file"] = rng.pick(["", "c:\\chars\\a.mmec", "missing.mmec", "x.txt"])
        if rng.chance(25):
            st["sim"] = True
        if rng.chance(20):
            st["learned"] = [s_["Number"] for s_ in rng.sample(SPELLS, 3)]
            st["lclass"] = rng.pick([0, 1, 3, 12])
        buffed = tuple(rng.sample(range(6), rng.int(0, 2))) if rng.chance(35) else ()
        text = paste_text(rng, st, eq, worn, with_inv=rng.chance(80), with_spells=rng.chance(25), buffed=buffed)
        run_case("f", st, text,
                 msgbox=rng.pick([[], [], [6], [7], [6, 6, 6], [7, 7, 7], [2], [6, 7], [7, 6, 6]]),
                 inputbox=rng.pick([[], [], ["70"], ["", "55"], ["0"]]), read=READS)



BASE = {"cls": 1, "race": 1, "lvl": 5, "stats": [20, 20, 20, 20, 20, 20], "sim": True}


def build_hand():
    rng = Rng(0x4A4D)
    w, hs = EQ[16][0], EQ[0][0]
    body = stat_block(rng, enc=120)
    two = "%s (Weapon Hand), %s (Head)" % (iname(w), iname(hs))
    hand = [
        ("", {}), ("short", {}), ("x" * 9, {}), ("no data at all, just words here", {}),
        ("Paste the commands below into your game to get your stats.\r\nCopy and paste the output here.\r\n\r\npowers\r\nspells\r\ninventory\r\nstat", {}),
        ("You have no spells.", {"learned": [1, 2, 3], "lclass": 4}), ("You have no powers.", {"learned": [5], "lclass": 2}),
        ("you have no spells", {}), (body, {}), (body.replace("\n", "\r\n"), {}), (body.replace("\n", "\r"), {}),
        (body + "\n" + equip_line([two]), {}), (body + "\n" + equip_line([two]), {"hold": [16]}),
        (equip_line([two]), {}), ("You are currently equipped with: " + two.lower(), {}),
        ("YOU ARE CURRENTLY EQUIPPED WITH: " + two, {}), ("equipped with: " + two, {}),
        ("You are carrying " + iname(NOEQ[0]) + ", " + iname(NOEQ[1]) + " (3)", {}),
        ("Name: A\nRace: Dwarf\nClass: Mage Level: 7", {}), ("Name: A\nRace: Dwarf Exp: 5 Class: Mage Level: 7", {}),
        ("Race: " + "Q" * 40 + "\nClass: " + "W" * 30 + "\nName: " + "Z" * 50 + "\nLevel: 3", {}),
        ("Race: Human Exp: 5\nClass: Mage Level: 0\nLevel: 0", {}), ("Level: 9", {}), ("Level: 9*", {}), ("Level:", {}),
        ("Strength: 99*\nIntellect: 88 *\nCharm: 0", {}), ("Strength: 99999999999\nHealth: 5", {}),
        ("Armour Class: 12/3", {}), ("Armour Class:  5/12 ", {}), ("Armour Class: 12345/3", {}), ("Armour Class: 0/3", {}),
        ("Armour Class:   9/0", {}), ("Armour Class: 100/", {}),
        ("Encumbrance: 100/300\n" + equip_line(["%s (Head)" % iname(hs)]), {}),
        ("Encumbrance: 2/300\n" + equip_line(["%s (Head)" % iname(hs)]), {}),
        ("Name: Bob\nRace: Elf\nClass: Priest\nLevel: 3\nEncumbrance: 80/300\n" + carry_line([iname(NOEQ[2]), iname(NOEQ[3])]), {}),
        ("head (Head), (Ears), ((Neck)), tst (Back", {}), ("%s (Head" % iname(hs), {}), ("%s )(Head)" % iname(hs), {}),
        ("a, b, c (WRIST), d (WRIST), e (WRIST), f (FINGER), g (FINGER), h (FINGER)", {}),
        ("x (Worn), y (Worn), z (Worn)", {}), ("Strength:\nHealth:\nAgility:", {}),
    ]
    for text, extra in hand:
        st = dict(BASE)
        st.update(extra)
        run_case("h", st, text, read=READS)
    # the class-change question, the save question, unequip-missing and hold on a loaded character
    cls_text = "Name: A\nRace: Dwarf\nClass: Mage\nLevel: 7\n" + equip_line(["%s (Head)" % iname(hs)])
    for loaded, ps, auto, sess in [(True, True, False, ""), (True, False, False, ""), (True, True, True, "a.mmec"),
                                   (True, True, True, ""), (True, False, True, "gone-missing.mmec"), (True, False, False, "z.mmec")]:
        for ans in ([6, 6], [7, 6], [2], []):
            st = dict(BASE)
            st.update(loaded=loaded, prompt_save=ps, autosave=auto, session_file=sess, lclass=1, learned=[10, 11])
            run_case("h", st, cls_text, msgbox=ans, read=READS)
    # spells pasted with a character, then its stats; a race below its minimum; buffed manual stats
    for r_ in RACES[:6]:
        st = dict(BASE)
        st.update(race=0, stats=[10, 10, 10, 10, 10, 10])
        txt = "Race: %s\nClass: Warrior\nLevel: 3\nStrength: 15\nAgility: 90*" % r_["Name"]
        run_case("h", st, txt, inputbox=[["55"], []][r_["Number"] % 2], read=READS)
    st = dict(BASE)
    st.update(race=2, cls=0, stats=[10] * 6, lclass=3, learned=[1], filter=True, loaded=False)
    for ans in ([6], [7], []):
        run_case("h", st, "Race: Human\nClass: Mage\nLevel: 2\n" + spell_lines(pick_spells(rng, 2), rng), msgbox=ans, read=READS)
    pr = real_dup_pair()
    st = dict(BASE)
    st["combos"] = {8: [9008], 3: []}
    run_case("h", st, "x (Hands), tst glove (Hands), tst glove (Hands)", read=READS)
    run_case("h", st, "tst twin (Worn), tst twin2 (Worn), tst twin (Worn)", read=READS)
    run_case("h", st, "tst twin (Worn), x (Back)", read=READS)
    run_case("h", st, "tst ghost (Hands), tst glove (Hands)", read=READS)
    st["only_in_game"] = True
    run_case("h", st, "tst ghost (Hands)", read=READS)
    used_eq.update(["tstglove", "tsttwin", "tsttwin2", "tstghost"])
    used_inv.update(["tst glove", "tst twin", "tst twin2", "tst ghost", "tst sundry"])
    used_numbers.update(range(9001, 9012))


def spell_case(prefix, st, text, **kw):
    st = dict(st)
    return add(prefix, forms={"frmMain": forms_for(st)}, globals=globals_for(st),
               steps=[{"call": "PasteSpells", "args": [{"ref": "string", "value": text}]}],
               read=reads("learned", "lclass"), **kw)


def build_spells():
    rng = Rng(0x59E1)
    ok_names = [s_["Name"] for s_ in SPELLS if s_["Name"] and s_["Short"] and plain(s_["Name"])]
    for cls in [0, 1, 4, 12, 14, 15, 3, 9]:
        for variant in range(5):
            names = pick_spells(rng, rng.int(1, 9))
            if variant == 1:
                names = names + names[:2]
            text = spell_lines(names, rng, header=variant != 2)
            if variant == 3:
                text = text.replace("\n", "\r\n") + "\r\n"
            if variant == 4:
                text = text.replace("Level Mana", "Level Kai ").replace("spells:", "powers:") + "\nxx"
            st = {"cls": cls, "filter": variant % 2 == 0, "learned": [3, 4] if variant == 3 else [], "lclass": 2 if variant == 3 else 0}
            spell_case("s", st, text)
    st = {"cls": 0}
    spell_case("s", st, "You have no spells.", )
    spell_case("s", {"cls": 5, "learned": [7, 8], "lclass": 5}, "You have no powers. ")
    spell_case("s", st, "short")
    spell_case("s", st, "")
    spell_case("s", st, "You have the following spells:\nLevel Mana Short Spell name\n")
    spell_case("s", st, "  1   2   ab  zzz not a spell  \n  x   2   ab  Foo Bar Baz\n 3 0 ab The Name Ok Fine\n 2 x abc long nonsense name here")
    names = pick_spells(rng, 6)
    spell_case("s", st, "You have the following spells:\n" + "\n".join(" 1 2 ab %s  " % n_ for n_ in names))
    spell_case("s", st, "You have the following spells:\n" + "\n".join("1\t2\tab\t%s" % n_ for n_ in names))
    spell_case("s", st, "\n".join("   1         2         ab        %s" % n_ for n_ in names))
    spell_case("s", st, "1 2 ab " + " ".join(names[:2]))
    uniq = [n_ for n_ in ok_names if sum(1 for q in SPELLS if (q['Name'] or '').strip(' ') == n_.strip(' ')) == 1]
    big = rng.sample(uniq, 104)
    for n_ in big:
        used_spell.add(n_.strip())
    spell_case("s", st, spell_lines(big, rng))
    spell_case("s", {"cls": 12}, spell_lines(big[:30], rng))
    spell_case("s", {"cls": 0, "learned": list(range(1, 40)), "lclass": 7}, spell_lines(big[:3], rng))
    spell_case("s", {"cls": 4, "lclass": 1}, spell_lines(big[:12], rng))


def inven_case(prefix, st, text, extra_only, **kw):
    st = dict(st)
    st["clip"] = text
    args = [text] if extra_only else [{"t": "missing"}]
    return add(prefix, forms={"frmMain": forms_for(st)}, globals=globals_for(st), read=reads("rows", "dontrefresh"),
               steps=[{"call": "PasteInventoryManager", "args": args}], **kw)


def build_inven():
    rng = Rng(0x1A2B)
    for i in range(24):
        eq, worn = gen_equipment(rng, rng.int(0, 4))
        st = {"cls": rng.pick([0, 1, 12, 7]), "lvl": rng.pick([5, 30, 99]), "rows": existing_rows(rng, NOEQ + EQ[0]),
              "charm_bonus": 0}
        st["stats"] = [10, 10, 10, 10, 10, rng.pick([10, 55, 120])]
        inv = [iname(n) for n in rng.sample(NOEQ + EQ[0] + EQ[16], rng.int(0, 5))]
        inv = [rng.pick([x, x, "%s (%d)" % (x, rng.int(2, 5))]) for x in inv]
        text = equip_line(equip_entries(rng, eq, worn)) + "\n" + carry_line(inv)
        extra_only = rng.chance(50)
        inven_case("i", st, text, extra_only, msgbox=rng.pick([[], [6, 6], [7, 6, 7], [6, 6, 6], [7, 7]]))
    for text in ["", "abcd", "You are carrying nothing", "You are carrying " + iname(NOEQ[0])]:
        for eo in (False, True):
            inven_case("i", {"cls": 1}, text, eo)


def shop_tokens(r):
    return [t.strip().lower() for t in (r.get("Obtained From") or "").split(",") if t.strip().lower().startswith("shop")]


def item_row(n):
    r = ITEMS[n]
    return {"n": n, "name": r["Name"], "t": r["ItemType"], "w": r["Worn"], "wt": r["WeaponType"], "at": r["ArmourType"],
            "enc": r["Encum"], "price": r["Price"], "cur": r["Currency"], "g": r["Gettable"], "ig": r["In Game"],
            "of": r.get("Obtained From") or None, "cr": [r.get("ClassRest-%d" % k) or 0 for k in range(10)], "ab": ab(r)}


def build_data():
    keys = {n: strip_sp(r["Name"]) for n, r in ITEMS.items()}
    lows = {n: r["Name"].strip(" ").lower() for n, r in ITEMS.items()}
    rows = [n for n in ITEMS if keys[n] in used_eq or lows[n] in used_inv or n in used_numbers]
    shops_used = set()
    for n in rows:
        for t in shop_tokens(ITEMS[n]):
            m = __import__("re").search(r"(\d+)", t)
            if m and int(m.group(1)) in SHOPS:
                shops_used.add(int(m.group(1)))
    rooms_used = set()
    for sn in shops_used:
        for part in (SHOPS[sn].get("Assigned To") or "").split(","):
            m = __import__("re").search(r"(\d+)/(\d+)", part)
            if m and "room" in part.lower() and (int(m.group(1)), int(m.group(2))) in ROOMS:
                rooms_used.add((int(m.group(1)), int(m.group(2))))
    spells = [sp for sp in SPELLS if (sp["Name"] or "").strip(" ") in used_spell]
    return {
        "items": [item_row(n) for n in rows],
        "shops": [{"n": n, "name": SHOPS[n]["Name"], "markup": SHOPS[n]["Markup%"], "asg": SHOPS[n].get("Assigned To") or None}
                  for n in sorted(shops_used)],
        "rooms": [{"m": m, "r": r, "name": ROOMS[(m, r)]} for (m, r) in sorted(rooms_used)],
        "classes": [{"n": c["Number"], "name": c["Name"], "mt": c["MageryType"], "ml": c["MageryLVL"], "wt": c["WeaponType"],
                     "at": c["ArmourType"], "ab": ab(c, 10)} for c in CLASSES],
        "races": [{"n": r["Number"], "name": r["Name"], "mins": [r["mSTR"], r["mINT"], r["mWIL"], r["mAGL"], r["mHEA"], r["mCHM"]]}
                  for r in RACES],
        "spells": [{"n": sp["Number"], "name": sp["Name"], "short": 1 if (sp["Short"] or "").strip(" ") else 0,
                    "learnable": sp["Learnable"], "lf": (sp["Learned From"] or "")[:5], "cb": (sp["Casted By"] or "")[:2],
                    "cls": sp["Classes"], "magery": sp["Magery"], "mlvl": sp["MageryLVL"], "req": sp["ReqLevel"]}
                   for sp in spells],
    }


def scenario():
    return {
        "family": "paste_char",
        "description": "FAM-25 character half (frmMain PasteCharacter, PasteInventoryManager, PasteSpells, ApplyPastedStat, "
                       "PastedStatIsBuffed; modMMudFunc TestPasteChar, TestAlphaChar) over the stock database.",
        "notes": [
            "Procedures are cut from frmMain.frm (scenario key frm). The paste dialog lines are replaced through subs: the "
            "pasted text is frmMain.txtClip.Text and the dialog always ends with Continue. Me.Enabled lines are dropped.",
            "VB6 raises a combo's Click and a text box's Change only when the value really changes; the cut code assigns "
            "ListIndex and Text through subs that call the real cmbEquip_Click, cmbGlobalRace_Click, cmbGlobalClass_Click "
            "(both indexes) and txtCharStats_Change only in that case. Refresh-only handlers are stubs.",
            "SaveCharacter is recorded as an event (success); FileExists is true for a name ending .mmec without 'missing'; "
            "FormIsLoaded sees the case's loaded_forms; LoadCharacter, RefreshAll and the list sort/colour are no-ops, so "
            "the Item Manager rows are in insertion order (the sort call is dropped from PasteInventoryManager; the port reports it as sort_after).",
            "A case's msgbox lists the answers in the order MME shows boxes (every box takes one, information boxes too); "
            "inputbox lists the answers to InputBox. Boxes and handled errors are in events.",
            "data holds the Items rows whose space-stripped or lower-case name a case can reach, their shops and rooms, all "
            "classes and races, and the spells whose names are pasted, for the Rust test. Generated by "
            "tools/oracle/gen/paste_char.py.",
        ],
        "database": True,
        "modules": ["modItemParse.bas"],
        "frm": FRM,
        "globals": {"nNMRVer": 1.83},
        "loaded_forms": [],
        "read": {},
        "forms": {
            "frmMain": {"cmbGlobalClass": {"$array": {"0": combo_spec(CLASS_LIST, 0, True), "1": combo_spec(CLASS_LIST, 0, True)}},
                        "cmbGlobalRace": {"$array": {"0": combo_spec(RACE_LIST, 0, True)}},
                        "cmbEquip": {"$array": {str(i): combo_spec([("None", 0)], 0) for i in range(20)}},
                        "txtCharMaxStats": {"$array": {str(i): {} for i in range(6)}},
                        "lvItemManager": {"$listview": []}, "cmbGlobalAlignment": {"ListIndex": 0}},
            "frmPasteChar": {}, "frmMonsterAttackSim": {},
        },
        "tolerance": {"default": 0},
        "cases": cases,
    }


def main():
    build_pure()
    build_apply()
    build_full()
    build_hand()
    build_spells()
    build_inven()
    scn = scenario()
    scn["data"] = build_data()
    scn["db_extra"] = {"Items": EXTRA_ITEMS}
    scn["paste_procs"] = True
    with open(OUT, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("wrote", OUT, len(cases), "cases", file=sys.stderr)


if __name__ == "__main__":
    main()

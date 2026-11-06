"""Writes fixtures/scenarios/paste_party.json: python3 -I tools/oracle/gen/paste_party.py fixtures/scenarios/paste_party.json
Paste party (card P3-41): frmPasteChar ParsePasteParty and CalculateAverageParty.

Deterministic (splitmix64 draws, no clock). Party texts are invented in the layout MME's own comment shows for the
game's stat and inventory output (never real logs), then bent: odd spacing, missing lines, health percentages,
ranks, junk, lower-case keys, wrapped inventory sentences. Item, class, race and spell rows come from the stock
SQLite, plus invented rows numbered 9001+ for branches no stock row reaches; the rows the cases can touch are
written once into the scenario's "data" so the Rust test can rebuild the tables."""
import glob
import json
import os
import re
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
con.row_factory = sqlite3.Row

MASK = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & MASK

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def below(self, n):
        return self.next() % n

    def between(self, lo, hi):
        return lo + self.below(hi - lo + 1)

    def choice(self, seq):
        return seq[self.below(len(seq))]

    def chance(self, pct):
        return self.below(100) < pct

    def sample(self, seq, k):
        pool = list(seq)
        out = []
        for _ in range(min(k, len(pool))):
            out.append(pool.pop(self.below(len(pool))))
        return out


rng = Rng(0x41A7C)

# ---------------------------------------------------------------------------------- rows
STOCK_ITEMS = [dict(r) for r in con.execute("select * from Items order by rowid")]
CLASSES = [dict(r) for r in con.execute("select * from Classes order by rowid")]
RACES = [dict(r) for r in con.execute("select * from Races order by rowid")]
SPELLS = [dict(r) for r in con.execute("select * from Spells order by rowid")]


def abil(*pairs, n=10):
    d = {}
    for i in range(n):
        a, v = pairs[i] if i < len(pairs) else (0, 0)
        d["Abil-%d" % i] = a
        d["AbilVal-%d" % i] = v
    return d


def blank(table_row, **kw):
    r = {k: (0 if not isinstance(v, str) else "") for k, v in table_row.items()}
    r.update(kw)
    return r


# invented classes: hp regen and dodge from the class, anti-magic with a negative value (does not count), a class
# without abilities, and a caster
EXTRA_CLASSES = [
    blank(CLASSES[0], Number=9001, Name="Tst Regen", CombatLVL=5, **abil((123, 25), (34, -3), (51, 0))),
    blank(CLASSES[0], Number=9002, Name="Tst Negam", CombatLVL=2, **abil((51, -1), (34, 0), (123, -10))),
    blank(CLASSES[0], Number=9003, Name="Tst Bare", CombatLVL=0),
    blank(CLASSES[0], Number=9004, Name="Tst Caster", CombatLVL=3, MageryType=1, MageryLVL=3, **abil((123, 5))),
    blank(CLASSES[0], Number=9005, Name="Tst Big Class", CombatLVL=9, MageryType=2, MageryLVL=2, **abil((34, 4000), (123, 4000))),
]
EXTRA_RACES = [
    blank(RACES[0], Number=9001, Name="Tst Regen Race", **abil((123, 30), (34, 12), (51, 0))),
    blank(RACES[0], Number=9002, Name="Tst Slow Race", **abil((123, -50), (34, -20))),
    blank(RACES[0], Number=9003, Name="Tst Huge", **abil((123, 20000), (34, 20000))),
]
ALL_CLASSES = CLASSES + EXTRA_CLASSES
ALL_RACES = RACES + EXTRA_RACES

# invented items: abilities no stock row carries (or carries rarely), a Worn 1 item for the "Worn" slot, names whose
# characters the paste filter drops, and sums that overflow an Integer
def item(n, name, **kw):
    base = blank(STOCK_ITEMS[0], Number=n, Name=name, **{"In Game": 1})
    base.update(kw)
    return base


EXTRA_ITEMS = [
    item(9001, "tst crit ring", Worn=4, Accy=3, **abil((7, 12), (11, 7), (13, 3), (14, 2), (15, 4), n=20)),
    item(9002, "tst skill gloves", Worn=3, Accy=0, **abil((37, 6), (40, 5), (38, 4), (41, 3), (39, 2), (42, 1), n=20)),
    item(9003, "tst ma damage gloves", Worn=3, Accy=2, **abil((34, 9), (35, 8), (36, 7), (19, 5), n=20)),
    item(9004, "tst regen amulet", Worn=8, Accy=-4, **abil((123, 15), (145, 6), (22, 12), n=20)),
    item(9005, "tst accy belt", Worn=10, Accy=9, **abil((22, 30), (22, 50), n=20)),
    item(9006, "tst worn charm", Worn=1, Accy=1, **abil((7, 2), n=20)),
    item(9007, "tst worn head", Worn=16, Accy=1, **abil((11, 3), n=20)),
    item(9008, "tst crit hat", Worn=2, Accy=0, **abil((7, 32000), n=20)),
    item(9009, "tst crit bracers", Worn=6, Accy=0, **abil((7, 32000), n=20)),
    item(9010, "tst accy overflow", Worn=14, Accy=40000, **abil((22, 40000), n=20)),
    item(9011, "tst zero value", Worn=13, Accy=0, **abil((7, 0), (11, 0), (123, 0), n=20)),
    item(9012, "Zed's #1 & ring", Worn=4, Accy=5, **abil((7, 4), n=20)),
    item(9013, "tst upper slot", Worn=3, Accy=1, **abil(*([(0, 0)] * 15), (7, 5), (123, 9), n=20)),
    item(9014, "tst out of game", Worn=4, Accy=6, **{"In Game": 0}, **abil((13, 7), n=20)),
    item(9015, "tst regen overflow a", Worn=13, Accy=0, **abil((123, 32000), n=20)),
    item(9016, "tst regen overflow b", Worn=5, Accy=0, **abil((123, 32000), n=20)),
    item(9017, "tst slow light dagger", ItemType=1, Worn=0, Accy=4, Speed=900, StrReq=0, Min=2, Max=5, WeaponType=1,
         Encum=5, **abil((13, 5), n=20)),
    item(9018, "tst heavy maul", ItemType=1, Worn=0, Accy=-2, Speed=2200, StrReq=180, Min=20, Max=44, WeaponType=4,
         Encum=90, **abil((7, 6), n=20)),
    item(9019, "tst free spear", ItemType=1, Worn=0, Accy=0, Speed=0, StrReq=0, Min=0, Max=0, WeaponType=2, Encum=0),
    item(9020, "tst second wrist", Worn=14, Accy=1, **abil((7, 1), n=20)),
]

# ---------------------------------------------------------------------------------- item pool
LABEL_OF_WORN = {2: "Head", 3: "Hands", 4: "Finger", 5: "Feet", 6: "Arms", 7: "Back", 8: "Neck", 9: "Legs", 10: "Waist",
                 11: "Torso", 12: "Off-Hand", 13: "Finger", 14: "Wrist", 15: "Ears", 16: "Worn", 1: "Worn"}


def strip(name):
    return (name or "").replace(" ", "")


def has_ab(r, a, n=20):
    return any(r["Abil-%d" % i] == a for i in range(n))


all_items = STOCK_ITEMS + EXTRA_ITEMS
by_strip = {}
for r in all_items:
    by_strip.setdefault(strip(r["Name"]), []).append(r)

wearable = [r for r in STOCK_ITEMS if r["Worn"] in LABEL_OF_WORN and r["Name"]]
weapons = [r for r in STOCK_ITEMS if r["ItemType"] == 1 and r["Name"] and not has_ab(r, 43)
           and 0 < r["Speed"] and r["Min"] + r["Max"] > 0]
with_ab = [r for r in wearable if any(has_ab(r, a) for a in (13, 14, 34, 36, 145, 123, 7, 11, 37, 19))]
pool_wear = {}
for w in range(1, 17):
    rows = [r for r in wearable if r["Worn"] == w]
    if not rows:
        continue
    pool_wear[w] = rng.sample(rows, 5)
    pool_wear[w] += rng.sample([r for r in with_ab if r["Worn"] == w], 3)
pool_weapons = rng.sample(weapons, 30)
pool_stock = [r for w in pool_wear.values() for r in w] + pool_weapons
pool_extra = [r for r in EXTRA_ITEMS]
pool_all = pool_stock + pool_extra
# every row sharing a stripped name with a pool item is part of the table (duplicates change what the scan finds)
names_in_pool = {strip(r["Name"]) for r in pool_all}
table_rows = [r for r in all_items if strip(r["Name"]) in names_in_pool]
table_numbers = {r["Number"] for r in table_rows}
# a few more rows that share nothing with the pool: the scan must skip them
all_names = {strip(r["Name"]) for r in all_items}

CAST_FREE_WEAPONS = [r for r in pool_weapons] + [r for r in EXTRA_ITEMS if r["ItemType"] == 1]
assert all(not has_ab(r, 43) for r in CAST_FREE_WEAPONS)

# ---------------------------------------------------------------------------------- spells
spell_pool_rows = [r for r in SPELLS if len((r["Short"] or "").strip()) == 4 and r["AttType"] in (0, 1, 2, 3, 5)
                   and r["MaxBase"] > 0 and (r["Learnable"] > 0 or len(r["Casted By"] or "") > 1)]
chosen_shorts = []
by_class = {}
for cl in CLASSES:
    if cl["MageryType"] == 0:
        continue
    rows = [r for r in spell_pool_rows if r["Magery"] in (0, cl["MageryType"])]
    by_class[cl["Number"]] = [r["Short"].strip() for r in rng.sample(rows, 4)]
    chosen_shorts += by_class[cl["Number"]]
extra_shorts = [r["Short"].strip() for r in rng.sample(spell_pool_rows, 8)]
chosen_shorts = sorted(set(chosen_shorts + extra_shorts))
spell_table_rows = [r for r in SPELLS if (r["Short"] or "").strip() in chosen_shorts]
# attack-type spells outside the chosen shorts are never named, so they are not needed

# ---------------------------------------------------------------------------------- encode rows for "data"


def pairs_flat(r, n):
    flat = []
    for i in range(n):
        flat += [r["Abil-%d" % i], r["AbilVal-%d" % i]]
    while flat and flat[-1] == 0 and flat[-2] == 0:
        flat = flat[:-2]
    return flat


def enc_item(r):
    return [r["Number"], r["Name"] or "", r["Encum"], r["Accy"], r["StrReq"], r["Speed"], r["Min"], r["Max"],
            r["WeaponType"], pairs_flat(r, 20), r["Worn"], r["ItemType"], r["In Game"]]


def enc_class(r):
    return [r["Number"], r["Name"], r["CombatLVL"], r["MageryType"], r["MageryLVL"], pairs_flat(r, 10)]


def enc_race(r):
    return [r["Number"], r["Name"], pairs_flat(r, 10)]


SPELL_KEYS = ["Number", "Name", "Short", "ReqLevel", "EnergyCost", "ManaCost", "MinBase", "MaxBase", "Diff", "TypeOfResists",
              "Targets", "Dur", "AttType", "Magery", "MageryLVL", "Cap", "MaxIncLVLs", "MaxInc", "MinIncLVLs", "MinInc",
              "DurIncLVLs", "DurInc", "Learnable", "Learned From", "Casted By", "Classes"]


def enc_spell(r):
    return [r[k] if r[k] is not None else "" for k in SPELL_KEYS] + [pairs_flat(r, 10)]


data = {
    "items": [enc_item(r) for r in table_rows],
    "classes": [enc_class(r) for r in ALL_CLASSES],
    "races": [enc_race(r) for r in ALL_RACES],
    "spells": [enc_spell(r) for r in spell_table_rows],
    "spell_keys": SPELL_KEYS,
}

# ---------------------------------------------------------------------------------- names and text
FIRST = ["Kratos", "Buster", "Happy", "Zelda", "Mog", "Thorn", "Ayla", "Brogan", "Dax", "Elmira", "Fenwick", "Gorm",
         "Hilda", "Ivo", "Jask", "Kira", "Lorn", "Mina", "Nox", "Orla", "Pike", "Quill", "Rook", "Sable", "Tam"]
LAST = ["Brown", "Gilmore", "Stonefist", "Windrider", "Ashgrove", "Blackwood", "Ironside", "Moonwhisper", "Redcliff"]
RANKS = ["Sir", "Lady", "Lord", "Captain", "Elder"]
STOCK_CLASS_NAMES = [c["Name"] for c in CLASSES]
STOCK_RACE_NAMES = [r["Name"] for r in RACES]


def member(i=None, full=True):
    m = {}
    nm = rng.choice(FIRST)
    if rng.chance(40):
        nm += " " + rng.choice(LAST)
    if rng.chance(8):
        nm = rng.choice(RANKS) + " " + nm
    m["name"] = nm
    cls = rng.choice(CLASSES + (EXTRA_CLASSES if rng.chance(25) else []))
    race = rng.choice(RACES + (EXTRA_RACES if rng.chance(20) else []))
    m["cls"], m["race"] = cls, race
    m["level"] = rng.choice([rng.between(1, 30), rng.between(1, 120), rng.between(60, 130), rng.between(1, 255)])
    m["exp"] = rng.between(0, 2000000000) * (rng.between(1, 9) if rng.chance(30) else 1)
    mhp = rng.choice([rng.between(20, 400), rng.between(300, 2400), rng.between(1000, 9000)])
    m["hits"] = (rng.between(1, mhp), mhp)
    m["ac"] = (rng.between(0, 120), rng.between(0, 60))
    caster = cls["MageryType"] > 0 and rng.chance(85)
    m["mana"] = None
    m["spellcasting"] = None
    if caster:
        mm = rng.between(5, 900)
        m["mana"] = (rng.between(0, mm), mm)
        m["spellcasting"] = rng.between(10, 360)
    m["stats"] = {k: rng.choice([rng.between(30, 120), rng.between(60, 255)]) for k in
                  ("Strength", "Intellect", "Willpower", "Agility", "Health", "Charm")}
    m["mr"] = rng.choice([0, rng.between(0, 60), rng.between(40, 250)])
    mx = rng.between(1500, 9000)
    m["enc"] = (rng.between(0, mx + (mx // 4 if rng.chance(15) else 0)), mx)
    m["lives"] = (rng.between(1, 9), rng.between(0, 2000))
    m["equip"] = []
    return m


def equip_for(m, melee=True):
    """Equipment as (name, label) pairs: mostly honest labels, sometimes a wrong or odd one."""
    out = []
    used = set()
    slots = rng.sample(list(pool_wear), rng.between(0, 13))
    for w in slots:
        r = rng.choice(pool_wear[w])
        label = LABEL_OF_WORN[w]
        out.append((r["Name"], label))
    if rng.chance(25):
        out.append((rng.choice(pool_extra[:16])["Name"], rng.choice(["Finger", "Wrist", "Hands", "Back", "Worn", "Head", "Torso"])))
    if rng.chance(12):
        out.append((rng.choice(pool_wear[4])["Name"], "Finger"))
        out.append((rng.choice(pool_wear[13])["Name"], "Finger"))
        out.append((rng.choice(pool_wear[13])["Name"], "Finger"))
    if rng.chance(12):
        out.append((rng.choice(pool_wear[14])["Name"], "Wrist"))
        out.append((rng.choice(pool_wear[14])["Name"], "Wrist"))
        out.append((rng.choice(EXTRA_ITEMS[-1:])["Name"], "Wrist"))
    if rng.chance(15):
        out.append((rng.choice(pool_wear[16])["Name"], "Worn"))
        out.append((rng.choice(pool_wear[16] + [EXTRA_ITEMS[5], EXTRA_ITEMS[6]])["Name"], "Worn"))
        out.append((EXTRA_ITEMS[5]["Name"], "Worn"))
    if melee:
        w = rng.choice(CAST_FREE_WEAPONS)
        out.append((w["Name"], rng.choice(["Weapon Hand"] * 8 + ["Two-Handed", "Weapon hand", "weaponhand", "Off-Hand"])))
    if rng.chance(15):
        out.append((rng.choice(pool_weapons)["Name"], "Off-Hand"))
    if rng.chance(10):
        out.append((rng.choice(["rusty hoe", "bag of tricks", "mystery thing"]), rng.choice(["Weapon Hand", "Head"])))
    # order is the game's slot order only loosely: shuffle with the seeded draws
    for k in range(len(out) - 1, 0, -1):
        j = rng.below(k + 1)
        out[k], out[j] = out[j], out[k]
    return out


def pad(s, n):
    return s + " " * max(0, n - len(s))


def wrap(text, width, rng_, mid_word=False):
    """Hard-wrap like a terminal: at a space (the space becomes the break) or, with mid_word, anywhere."""
    lines = []
    cur = ""
    for w in text.split(" "):
        if cur and len(cur) + 1 + len(w) > width:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w) if cur else w
    lines.append(cur)
    if mid_word:
        joined = " ".join(lines)
        lines = [joined[i:i + width] for i in range(0, len(joined), width)]
    return lines


def render(m, style, eol):
    """One character's stat output followed by the inventory sentence."""
    hp = "%d/%d" % m["hits"]
    ac = "%d/%d" % m["ac"]
    st = m["stats"]
    L = []
    if style == 0:        # the game's columns
        L.append(pad("Name: %s" % m["name"], 39) + "Lives/CP:  %7s" % ("%d/%d" % m["lives"]))
        L.append(pad("Race: %s" % m["race"]["Name"], 18) + pad("Exp: %d" % m["exp"], 21) + "Perception:  %5d" % 75)
        L.append(pad("Class: %s" % m["cls"]["Name"], 18) + pad("Level: %d" % m["level"], 21) + "Stealth:  %8d" % 0)
        L.append(pad("Hits: %6s" % hp, 18) + pad("Armour Class: %s" % ac, 21) + "Thievery:  %7d" % 0)
        if m["mana"]:
            L.append(pad("Mana: * %s" % ("%d/%d" % m["mana"]), 18) + pad("Spellcasting: %d" % m["spellcasting"], 21) + "Traps:  %9d" % 0)
        else:
            L.append(" " * 39 + "Traps:  %9d" % 0)
        L.append(" " * 39 + "Picklocks:  %6d" % 0)
        L.append(pad("Strength:  %d" % st["Strength"], 18) + pad("Agility: %d" % st["Agility"], 21) + "Tracking:  %6d" % 0)
        L.append(pad("Intellect: %d" % st["Intellect"], 18) + pad("Health:  %d" % st["Health"], 21) + "Martial Arts:  %2d" % 21)
        L.append(pad("Willpower: %d" % st["Willpower"], 18) + pad("Charm:   %d" % st["Charm"], 21) + "MagicRes:  %6d" % m["mr"])
    elif style == 1:      # one run-on line, single spaces
        parts = ["Name: %s" % m["name"], "Race: %s" % m["race"]["Name"], "Class: %s" % m["cls"]["Name"], "Level: %d" % m["level"],
                 "Hits: %s" % hp, "Armour Class: %s" % ac]
        if m["mana"]:
            parts += ["Mana: %d/%d" % m["mana"], "Spellcasting: %d" % m["spellcasting"]]
        parts += ["Strength: %d" % st["Strength"], "Intellect: %d" % st["Intellect"], "Willpower: %d" % st["Willpower"],
                  "Agility: %d" % st["Agility"], "Health: %d" % st["Health"], "Charm: %d" % st["Charm"], "MagicRes: %d" % m["mr"]]
        L.append(" ".join(parts))
    elif style == 2:      # one field per line, tabs, shuffled
        fields = [("Name", m["name"]), ("Race", m["race"]["Name"]), ("Class", m["cls"]["Name"]), ("Level", str(m["level"])),
                  ("Hits", hp), ("Armour Class", ac), ("Strength", str(st["Strength"])), ("Intellect", str(st["Intellect"])),
                  ("Willpower", str(st["Willpower"])), ("Agility", str(st["Agility"])), ("Health", str(st["Health"])),
                  ("Charm", str(st["Charm"])), ("MagicRes", str(m["mr"]))]
        if m["mana"]:
            fields += [("Mana", "%d/%d" % m["mana"]), ("Spellcasting", str(m["spellcasting"]))]
        rest = fields[1:]
        for k in range(len(rest) - 1, 0, -1):
            j = rng.below(k + 1)
            rest[k], rest[j] = rest[j], rest[k]
        for k, v in [fields[0]] + rest:
            L.append("%s:\t%s" % (k, v) if rng.chance(50) else "%s :  %s" % (k, v) if rng.chance(30) else "%s: %s" % (k, v))
    else:                 # MegaMUD-ish party row, mostly junk to the parser
        L.append("%s the %s (%s) [%d%%] HP %s MA %s" % (m["name"], m["cls"]["Name"], m["race"]["Name"], rng.between(1, 100), hp,
                                                          "%d/%d" % m["mana"] if m["mana"] else "n/a"))
        L.append("Hits: %d%%   Level: %d" % (rng.between(1, 100), m["level"]))
    if m.get("junk") and style != 3:
        L.insert(rng.below(len(L) + 1), rng.choice(["You feel protected!", "Your ears are ringing.", "Rank: Sir (5)",
                                                    "Hits: 100%", "Level: ???", "Name:", "Class: ", "(nothing)", "Race:    "]))
    out = list(L)
    if m["equip"] is not None and len(m["equip"]) > 0:
        lead = rng.choice(["You are carrying ", "You are carrying ", "You are equipped with: ", "You are carrying  "])
        text = lead + ", ".join("%s (%s)" % (n, lab) for n, lab in m["equip"])
        if rng.chance(40):
            text += ", " + ", ".join(rng.sample(["rope and grapple", "mine pass", "3 waterskin", "torch", "5 gold crowns"], 3))
        out += wrap(text, rng.choice([60, 78, 78, 40]), rng, mid_word=rng.chance(15))
        if rng.chance(50):
            out.append("You have the following keys:  helmet key, 2 dragon keys, ancient obsidian key.")
        if rng.chance(40):
            out.append("Wealth: %d copper farthings" % rng.between(0, 99999))
    if m["enc"] and rng.chance(85) and style != 3:
        out.append("Encumbrance: %d/%d - Medium [%d%%]" % (m["enc"] + (rng.between(0, 99),)))
    return eol.join(out)


def party_text(members, styles, eol, sep):
    return sep.join(render(m, s, eol) for m, s in zip(members, styles))


# ---------------------------------------------------------------------------------- controls
IDX = range(0, 7)
TEXT_ARRAYS = ["txtPastePartyName", "txtPastePartyAC", "txtPastePartyDR", "txtPastePartyMR", "txtPastePartyDodge",
               "txtPastePartyHitpoints", "txtPastePartyRestHP", "txtPastePartyRegenHP", "txtPastePartyACCY",
               "txtPastePartyHeals", "txtPastePartyDMG", "txtPastePartySpellDMG", "txtPastePartySwings"]


def arr(props=None, lo=0, hi=6, **kw):
    props = props or {}
    return {"$array": {str(i): dict(props.get(i, {})) for i in range(lo, hi + 1)}, **kw}


def radio(selected=None, tags=None):
    tags = tags or {}
    return arr({i: {"Value": i == selected, "Tag": tags.get(i, "")} for i in range(1, 7)}, 1, 6, **{"$radio": True})


combo_class = {"$array": {"0": {"List": ["Any"] + [c["Name"] for c in ALL_CLASSES],
                                "ItemData": {str(i + 1): c["Number"] for i, c in enumerate(ALL_CLASSES)}}}}
combo_race = {"$array": {"0": {"List": ["Any"] + [r["Name"] for r in ALL_RACES],
                               "ItemData": {str(i + 1): r["Number"] for i, r in enumerate(ALL_RACES)}}}}


def form_state(prior):
    """The frmPasteChar boxes as the case leaves them before the procedure runs. prior: dict of sparse overrides."""
    f = {}
    for name in TEXT_ARRAYS:
        vals = prior.get(name)
        if vals:
            f[name] = arr({i: {"Text": v} for i, v in vals.items()})
    if "am" in prior:
        f["chkPastePartyAM"] = arr({i: {"Value": v} for i, v in prior["am"].items()})
    if "atk" in prior:
        sel, tags = prior["atk"]
        f["optPastyPartyAtkLast"] = radio(sel, tags)
    if "total" in prior:
        f["txtPastePartyPartyTotal"] = {"Text": prior["total"]}
    if "amtotal" in prior:
        f["txtPastePartyAMTotal"] = {"Text": prior["amtotal"]}
    return f


def prior_state(full):
    """A random earlier session: some boxes filled (the parse clears most of them), heals/damage/swings left alone."""
    p = {}
    if rng.chance(50):
        for name in ("txtPastePartyAC", "txtPastePartyName", "txtPastePartyHitpoints", "txtPastePartyMR"):
            if rng.chance(50):
                p[name] = {i: rng.choice(["12", "Old Name", "7", "0", " 5 "]) for i in rng.sample(range(0, 7), rng.between(1, 4))}
    for name in ("txtPastePartyHeals", "txtPastePartyDMG", "txtPastePartySpellDMG", "txtPastePartySwings"):
        if rng.chance(30):
            p[name] = {i: rng.choice(["15", "1.5", "0", "", "300", " 40", "abc", "2.25"]) for i in rng.sample(range(0, 7), rng.between(1, 5))}
    if rng.chance(30):
        p["am"] = {i: rng.choice([0, 1]) for i in range(0, 7)}
    if rng.chance(30):
        sel = rng.between(1, 6)
        tags = {sel: rng.choice([rng.choice(FIRST), "Kratos", ""]) if rng.chance(70) else ""}
        p["atk"] = (sel if rng.chance(80) else None, tags)
    if rng.chance(30):
        p["total"] = rng.choice(["0", "3", "", "6"])
    if rng.chance(20):
        p["amtotal"] = "2"
    return p


# probe code spliced into ParsePasteParty's out: section (the line after `out:` is `On Error Resume Next`): copies
# the locals the form never stores into captions the harness reads back
PROBE = "\n".join([
    "On Error Resume Next",
    "For iChar = 1 To 6",
    'sText = ""',
    "For x = 0 To 19",
    'sText = sText & sEquipLoc(iChar, x) & "~"',
    "Next x",
    'frmMain.lblEquip(iChar).Caption = sText & sWorn(iChar, 0) & "~" & sWorn(iChar, 1)',
    'frmMain.lblProbe(iChar).Caption = nWeaponNum(iChar) & "|" & nWeaponSpeed(iChar) & "|" & nWeaponSTR(iChar) & "|" & nAccyWorn(iChar) & "|" & nAccyAbil(iChar) & "|" & nPlusDodge(iChar) & "|" & nPlusRegen(iChar) & "|" & nPhysDamage(iChar) & "|" & nSwings(iChar) & "|" & nSpellDamage(iChar) & "|" & tCharacter(iChar).nHP & "|" & tCharacter(iChar).nHPRegen & "|" & tCharacter(iChar).nManaRegen & "|" & tCharacter(iChar).nEncumPCT & "|" & tCharacter(iChar).nAccuracy & "|" & tCharacter(iChar).nLevel & "|" & tCharacter(iChar).nClass & "|" & tCharacter(iChar).nRace & "|" & tCharacter(iChar).nCombat & "|" & tCharacter(iChar).nSTR & "|" & tCharacter(iChar).nAGI & "|" & tCharacter(iChar).nCHA & "|" & tCharacter(iChar).nWis & "|" & tCharacter(iChar).nINT & "|" & tCharacter(iChar).nHEA & "|" & tCharacter(iChar).nSpellcasting & "|" & tCharacter(iChar).nMaxMana & "|" & tCharacter(iChar).nCrit & "|" & tCharacter(iChar).nDodge & "|" & tCharacter(iChar).nPlusMaxDamage & "|" & tCharacter(iChar).nPlusBSaccy & "|" & tCharacter(iChar).nPlusBSmindmg & "|" & tCharacter(iChar).nPlusBSmaxdmg & "|" & tCharacter(iChar).nStealth & "|" & tCharacter(iChar).nMAPlusSkill(1) & "|" & tCharacter(iChar).nMAPlusSkill(2) & "|" & tCharacter(iChar).nMAPlusSkill(3) & "|" & tCharacter(iChar).nMAPlusAccy(1) & "|" & tCharacter(iChar).nMAPlusAccy(2) & "|" & tCharacter(iChar).nMAPlusAccy(3) & "|" & tCharacter(iChar).nMAPlusDmg(1) & "|" & tCharacter(iChar).nMAPlusDmg(2) & "|" & tCharacter(iChar).nMAPlusDmg(3)',
    "Next iChar",
    'frmMain.lblProbeX.Caption = nEnergy & "|" & nCombat & "|" & nNameCount & "|" & sFindAtkLast',
])

FRM = {"file": "frmPasteChar.frm", "enums": ["PartyCalc"], "decls": ["bHoldPartyRefresh"],
       "procs": ["ParsePasteParty", "CalculateAverageParty"],
       "subs": {"ParsePasteParty": {"On Error Resume Next": PROBE}},
       "stub_procs": ["cmdCancel_Click"]}

READ = {}
for _n in TEXT_ARRAYS:
    READ[_n] = "%%c.%s.Text[0..6]" % _n
READ.update({
    "am": "%c.chkPastePartyAM.Value[0..6]",
    "amTotal": "%c.txtPastePartyAMTotal.Text",
    "partyTotal": "%c.txtPastePartyPartyTotal.Text",
    "atk": "%c.optPastyPartyAtkLast.Value[1..6]",
    "atkTag": "%c.optPastyPartyAtkLast.Tag[1..6]",
    "probe": "%c.lblProbe.Caption[1..6]",
    "equip": "%c.lblEquip.Caption[1..6]",
    "probeX": "%c.lblProbeX.Caption",
})
CALC_READ = {k: v for k, v in READ.items() if k not in ("probe", "equip", "probeX")}

SETUP_FORMS = {"frmMain": {
    **{n: arr() for n in TEXT_ARRAYS},
    "chkPastePartyAM": arr(),
    "optPastyPartyAtkLast": radio(),
    "lblProbe": arr(lo=1), "lblEquip": arr(lo=1),
    "cmbGlobalClass": combo_class, "cmbGlobalRace": combo_race,
    "lblDatVer": {"Caption": "v1.11p"},
}}

# ---------------------------------------------------------------------------------- cases
cases = []


def add(cid, **kw):
    c = {"id": cid}
    c.update(kw)
    cases.append(c)


def parse_case(cid, text, prior=None, answers=None, **globs):
    forms = {"frmPasteChar": {"txtText": {"Text": text}}}
    f = form_state(prior or {})
    if f:
        forms["frmMain"] = f
    kw = {"forms": forms, "steps": [{"call": "ParsePasteParty"}]}
    if answers is not None:
        kw["inputbox"] = answers
    g = {k: v for k, v in globs.items()}
    if g:
        kw["globals"] = g
    add(cid, **kw)


NL = "\r\n"
ATTACKS = ["a", "a", "aa", "aaa", "bs", "pu", "kick", "jk", "bash", "smash", "sm", "ba", "p", "k", "j", "jumpkick", "backstab",
           "att", "attack", "at", "punch", "ki", "jumpk", "", "0", "xyz", " a ", "A", "zzzzz", "0 "]


def spell_answer(m):
    cl = m["cls"]["Number"]
    if cl in by_class and rng.chance(75):
        return rng.choice(by_class[cl])
    return rng.choice(extra_shorts + ["nope", "abcd"])


def answers_for(members):
    out = []
    for m in members:
        r = rng.below(10)
        if r < 5:
            out.append(rng.choice(ATTACKS[:15]))
        elif r < 7:
            out.append(spell_answer(m))
        elif r < 8:
            out.append(None)
        else:
            out.append(rng.choice(ATTACKS))
    return out


def random_party(size, with_equip=True):
    ms = [member() for _ in range(size)]
    for m in ms:
        if with_equip and rng.chance(75):
            m["equip"] = equip_for(m, melee=rng.chance(85))
        else:
            m["equip"] = []
        m["junk"] = rng.chance(15)
        if rng.chance(10):
            m["mana"] = None
            m["spellcasting"] = None
        if rng.chance(8):
            m["enc"] = None
    return ms


GLOBS = {"nNMRVer": 1.83, "gAvgLevelMaxAllStats": 0}


def rand_globals():
    g = {}
    if rng.chance(25):
        g["bOnlyInGame"] = True
    if rng.chance(25):
        g["bInvenUse2ndWrist"] = True
    g["nNMRVer"] = rng.choice([1.83, 1.83, 1.83, 1.7, 1.5, 1.8])
    if rng.chance(15):
        g["gAvgLevelMaxAllStats"] = rng.choice([40.5, 120, 260, 300.5])
    return g


# --- hand-made cases
parse_case("empty", "")
parse_case("short-9", "Name: Bob")
parse_case("short-9b", "123456789")
parse_case("nomatch-total0", "this text is long enough but has nothing the pattern wants in it")
parse_case("nomatch-total3", "this text is long enough but has nothing the pattern wants in it", prior={"total": "3", "txtPastePartyAC": {1: "40"}})
parse_case("nomatch-lowercase", "name: bob  hits: 10/20  armour class: 5/6  level: 4 race: elf class: mage")
parse_case("only-name", "Name: Solo Hero\nRace: Elf")

# one canonical stock-looking block per class/race pair, equipment-free and equipped
for k in range(10):
    m = member()
    m["equip"] = equip_for(m) if k % 2 else []
    parse_case("one-%02d" % k, party_text([m], [0], NL, NL), answers=answers_for([m]), **rand_globals())

# --- random parties
SIZES = [1, 2, 2, 3, 3, 3, 4, 4, 5, 5, 6, 6, 6, 7, 8]
for k in range(80):
    size = rng.choice(SIZES)
    ms = random_party(size)
    styles = [rng.choice([0, 0, 0, 0, 1, 2, 3]) for _ in ms]
    eol = rng.choice(["\r\n", "\r\n", "\n", "\r"])
    sep = eol * rng.choice([1, 2, 2, 3])
    text = party_text(ms, styles, eol, sep)
    if rng.chance(15):
        text = "   \t" + text + eol + "  "
    prior = prior_state(True) if rng.chance(60) else {}
    parse_case("party-%03d" % k, text, prior=prior, answers=answers_for(ms), **rand_globals())

# --- dropping fields: a character missing lines shifts nobody into the wrong slot after a new Name:
DROPPABLE = ["Mana", "Spellcasting", "Encumbrance", "Level", "Race", "Class", "Hits", "Armour Class", "MagicRes", "Strength",
             "Agility", "Charm", "Health", "Willpower", "Intellect"]
for k in range(28):
    size = rng.between(2, 5)
    ms = random_party(size)
    styles = [rng.choice([0, 0, 1, 2]) for _ in ms]
    eol = "\r\n"
    parts = []
    for m, s in zip(ms, styles):
        t = render(m, s, eol)
        for d in rng.sample(DROPPABLE, rng.between(0, 4)):
            if rng.chance(60):
                t = re.sub(r"%s:\s*\*?\s*[-\d/]+" % re.escape(d), "", t, count=1)
        parts.append(t)
    text = (eol * 2).join(parts)
    # sometimes a wholly missing character block header
    if rng.chance(20) and len(parts) > 1:
        text = text.replace("Name:", "Nam:", 1)
    parse_case("drop-%03d" % k, text, prior=prior_state(True) if rng.chance(40) else {}, answers=answers_for(ms), **rand_globals())

# --- repeated fields, label look-alikes, case and wrapping oddities
for k in range(15):
    ms = random_party(rng.between(1, 2))
    text = party_text(ms, [0] * len(ms), "\n", "\n\n")
    kind = k % 5
    if kind == 0:     # doubled lines push every later slot
        text = text.replace("Level:", "Level: 5 Level:", 1)
    elif kind == 1:   # lower-case keys match the pattern but not the Select Case
        text = text.replace("Hits:", "hits:", 1).replace("Class:", "CLASS:", 1)
    elif kind == 2:   # the name is the next label
        text = text.replace("Name: ", "Name:\n", 1)
    elif kind == 3:   # rank junk and label look-alikes
        text = "Rank: Sir Level: 99\nSurname: Smith\nXName: Y\n" + text
    else:             # armour class without the slash, health percent, signed values
        text = text.replace("Armour Class: ", "Armour Class: -", 1) + "\nHits: 55%  Armour Class:  12\nMana: * -5/10"
    parse_case("odd-%03d" % k, text, answers=answers_for(ms), **rand_globals())

# --- numbers that overflow a 16-bit field, or sit at its edge
OVER = [
    ("Level: 99999", "level"), ("Strength: 40000", "str"), ("MagicRes: 70000", "mr"), ("Hits: 5/3000000000", "hp"),
    ("Armour Class: 99999/5", "ac"), ("Armour Class: 5/99999", "dr"), ("Mana: 5/40000", "mana"), ("Spellcasting: 33000", "sc"),
    ("Encumbrance: 99999999/1", "enc"), ("Encumbrance: 5/0", "enc0"), ("Encumbrance: -50/1000", "encneg"),
    ("Level: 32767", "lvl-edge"), ("Level: 0000000000000000000000012", "zeros"), ("Agility: 32768", "agi"),
]
for k, (frag, tag) in enumerate(OVER):
    m = member()
    m["equip"] = equip_for(m)
    t = render(m, 0, NL) + NL + frag
    parse_case("over-%s" % tag, t, answers=answers_for([m]), **rand_globals())
    if k % 3 == 0:
        m2 = member()
        parse_case("over2-%s" % tag, render(m, 0, NL) + NL + NL + render(m2, 0, NL) + NL + frag, answers=answers_for([m, m2]), **rand_globals())

# --- item scan corner cases: every invented item, honest and dishonest labels
for k, it in enumerate(EXTRA_ITEMS):
    m = member()
    m["cls"] = rng.choice(CLASSES)
    label = LABEL_OF_WORN.get(it["Worn"], "Weapon Hand") if it["ItemType"] != 1 else "Weapon Hand"
    m["equip"] = [(it["Name"], label)]
    if rng.chance(50):
        m["equip"].append((rng.choice(pool_wear[4])["Name"], "Finger"))
    parse_case("item-%02d" % k, render(m, 0, NL), answers=answers_for([m]), **rand_globals())

# a ring list that fills both finger slots then a third; wrists with and without the second-wrist flag
for flag in (False, True):
    for oig in (False, True):
        m = member()
        m["equip"] = [(pool_wear[14][0]["Name"], "Wrist"), (pool_wear[14][1]["Name"], "Wrist"), (EXTRA_ITEMS[19]["Name"], "Wrist"),
                      (pool_wear[4][0]["Name"], "Finger"), (pool_wear[13][0]["Name"], "Finger"), (pool_wear[13][1]["Name"], "Finger"),
                      (EXTRA_ITEMS[13]["Name"], "Finger"), (pool_wear[3][0]["Name"], "Hands")]
        m2 = member()
        m2["equip"] = [(pool_wear[14][1]["Name"], "Wrist"), (pool_wear[3][1]["Name"], "Hands")]
        parse_case("wrist-%d%d" % (flag, oig), render(m, 0, NL) + NL + NL + render(m2, 0, NL), answers=["a", "a"],
                   bInvenUse2ndWrist=flag, bOnlyInGame=oig, nNMRVer=1.83)

# inventory placement: sentences without the lead words, before any Name, reordered, labels in odd case
for k in range(12):
    m = member()
    m["equip"] = equip_for(m)
    body = render(m, 0, NL)
    if k % 4 == 0:
        body = body.replace("You are carrying", "Carrying:").replace("You are equipped with:", "Equipment:")
    elif k % 4 == 1:
        i = body.find("You are")
        if i > 0:
            body = body[i:] + NL + body[:i]
    elif k % 4 == 2:
        body = re.sub(r"\((\w[\w -]*)\)", lambda mm: "(%s)" % mm.group(1).upper() if rng.chance(50) else "(%s)" % mm.group(1).lower(), body)
    else:
        body = body.replace(", ", " ,  ").replace("(", " ( ")
    parse_case("place-%02d" % k, body, answers=answers_for([m]), **rand_globals())

# a 7th and 8th Name: block is ignored; a text past 32767 characters overflows the 16-bit loop counters
ms = random_party(8)
parse_case("eight", party_text(ms, [0] * 8, NL, NL * 2), answers=answers_for(ms), **rand_globals())
ms = random_party(2)
long_junk = ("lorem ipsum dolor sit amet, consectetur adipiscing elit " * 600)[:32900]
parse_case("long-33000", party_text(ms[:1], [0], NL, NL) + NL + long_junk + NL + party_text(ms[1:], [0], NL, NL), answers=["a", "a"], **rand_globals())

# --- attack answers for fixed members: every attack word with and without a weapon, MA skill and spellcasting
for k in range(40):
    m = member()
    m["equip"] = equip_for(m, melee=rng.chance(70))
    if rng.chance(30):
        m["equip"].append((EXTRA_ITEMS[1]["Name"] if rng.chance(50) else EXTRA_ITEMS[2]["Name"], "Hands"))
    ans = rng.choice(ATTACKS) if k % 3 else spell_answer(m)
    parse_case("atk-%02d" % k, render(m, 0, NL), answers=[ans], prior=prior_state(True) if rng.chance(30) else {}, **rand_globals())
# an answer list that ends early (cancel), and null for the default
ms = random_party(4)
parse_case("atk-cancel", party_text(ms, [0] * 4, NL, NL), answers=["a"], **rand_globals())
parse_case("atk-default", party_text(ms, [0] * 4, NL, NL), answers=[None, None, None, None], **rand_globals())
parse_case("atk-default-prior", party_text(ms, [0] * 4, NL, NL), answers=[None, None, None, None],
           prior={"txtPastePartyDMG": {1: "30", 3: "0"}, "txtPastePartySpellDMG": {2: "12"}}, **rand_globals())

# --- CalculateAverageParty on its own: random form states, every nWhat
WHATS = list(range(0, 14)) + [14, 20]
for k in range(60):
    p = {}
    for name in TEXT_ARRAYS:
        if rng.chance(75):
            p[name] = {i: rng.choice([str(rng.between(0, 200)), str(rng.between(0, 30)), "", "  ", " 7 ", "abc", "1.5", "-4", "2.25", "0"])
                       for i in rng.sample(range(0, 7), rng.between(0, 6))}
    if rng.chance(70):
        p["am"] = {i: rng.choice([0, 1, 1, 2]) for i in range(0, 7)}
    if rng.chance(60):
        sel = rng.between(1, 6)
        p["atk"] = (sel if rng.chance(90) else None, {sel: rng.choice(["", "Bob", "Kratos"])})
    if rng.chance(30):
        p["total"] = "9"
    if rng.chance(20):
        p["amtotal"] = "9"
    # members whose names the atk-last tag reconciliation can see
    if "txtPastePartyName" not in p and rng.chance(50):
        p["txtPastePartyName"] = {i: rng.choice(["Bob", "Kratos", "  Kratos ", "", "x"]) for i in range(1, 7)}
    what = WHATS[k % len(WHATS)]
    kw = {"forms": {"frmMain": form_state(p)}, "steps": [{"call": "CalculateAverageParty", "args": [what]}], "read": CALC_READ}
    if rng.chance(15):
        kw["globals"] = {"bHoldPartyRefresh": True}
    add("avg-%03d-w%d" % (k, what), **kw)
# no argument at all: Optional nWhat defaults to 0
add("avg-noarg", forms={"frmMain": form_state({"txtPastePartyAC": {1: "10", 2: "20", 3: "35"},
                                                 "atk": (2, {2: "Bob"}), "txtPastePartyName": {2: "Bob"}})},
    steps=[{"call": "CalculateAverageParty"}], read=CALC_READ)

scn = {
    "family": "paste_party",
    "description": "frmPasteChar ParsePasteParty and CalculateAverageParty (stock): party text to per-member rows and the averages.",
    "database": True,
    "notes": [
        "Invented party texts in the layout of the game's stat and inventory output; rows 9001+ are invented (db_extra).",
        "frmMain.lblProbe/lblEquip/lblProbeX captions are written by a probe line spliced into ParsePasteParty's out: section "
        "and hold the locals the form never stores; they are the oracle's read-back, not MME output.",
        "InputBox answers come from the case's inputbox list in call order; null accepts the default.",
        "Weapons that cast (ability 43) are not used: the cast text needs the spell-text tables.",
    ],
    "frm": FRM,
    "db_extra": {"Items": EXTRA_ITEMS, "Classes": EXTRA_CLASSES, "Races": EXTRA_RACES},
    "data": data,
    "globals": GLOBS,
    "forms": SETUP_FORMS,
    "read": READ,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases, %d items, %d spells" % (len(cases), len(data["items"]), len(data["spells"])), file=sys.stderr)

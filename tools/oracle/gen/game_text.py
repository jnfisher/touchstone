"""Writes fixtures/scenarios/game_text.json (FAM-22 text parsers). Stdlib only, deterministic:
    python3 -I tools/oracle/gen/game_text.py
All texts are invented MajorMUD-style lines, never real logs. The random part uses splitmix64 so it
does not depend on the Python version."""
import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "game_text.json")
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


ITEMS = ["iron helm", "stormmetal greataxe", "elven cloak", "flask of water", "bandage roll", "ruby ring",
         "silver locket", "oak staff", "leather boots", "wolf pelt", "glowing shard", "brass lantern",
         "ash arrow", "bone dagger", "woolen scarf", "copper bracelet", "healing salve", "mithril buckler",
         "frayed rope", "dwarven pickaxe", "jade amulet", "spiked club", "silk sash", "tin whistle"]
PLURALS = ["flasks of water", "bandage rolls", "ash arrows", "wolf pelts", "glowing shards", "tin whistles",
           "apples", "torches", "brushes", "glasses", "daisies", "boxes", "pies", "buses", "sandals"]
SLOTS = ["Head", "Weapon Hand", "Off-Hand", "Torso", "Legs", "Feet", "Back", "Neck", "Ears", "Wrist", "Finger",
         "Waist", "Hands"]
KEYS = ["rusty key", "skeleton key", "bone key", "crystal key", "iron keys", "golden idol", "silver idols",
        "brass keys", "jade idols", "obsidian key", "ornate box key", "gate key"]
COINS = ["12 gold crowns", "5 silver nobles", "3 copper farthings", "1 platinum piece", "20 runic coins",
         "1 gold crown", "400 copper farthings"]
ROOMS = ["Moss-covered Courtyard", "Old Guardhouse", "Flooded Cellar", "Hall of Pillars", "Cobbled Lane",
         "Smoky Taproom", "Ruined Chapel", "Narrow Stair"]
EXITS = ["Obvious exits: north, east", "Obvious exits: south", "Obvious exits: up, down, west",
         "Obvious exits: northeast, southwest"]
SLOTLESS_JUNK = ["Name: Brogan Ironhand", "Race: Dwarf", "Class: Warrior", "Wealth: 12 gold crowns",
                 "Encumbrance: 40/120 - Light [33%]", "*Combat Off*", "Also here: a bored goblin.",
                 "[HP=45/60]:", "[HP=45/60]:n", "[MA=3/9]:", "A faint draft stirs the dust."]
MOVES = ["n", "s", "e", "w", "ne", "nw", "se", "sw", "north", "south", "east", "west", "northeast", "NW",
         "[HP=40/60]:n", "[HP=40/60]:  east", "up", "[Hp=1]:SE"]
SQ = [" ", "  ", "   "]


def wrap(rng, text, lo=18, hi=60):
    """Break text at spaces into lines of random width, as a client would wrap it."""
    words = text.split(" ")
    lines, cur = [], ""
    width = rng.int(lo, hi)
    for w in words:
        if cur and len(cur) + 1 + len(w) > width:
            lines.append(cur)
            cur = w
            width = rng.int(lo, hi)
        else:
            cur = (cur + " " + w) if cur else w
    lines.append(cur)
    return lines


def count_prefix(rng, name):
    r = rng.int(1, 10)
    if r <= 5:
        return name
    if r <= 7:
        return "%d %s" % (rng.int(2, 6), name)
    if r <= 9:
        return "%s (%d)" % (name, rng.int(2, 5))
    return "%d %s (%d)" % (rng.int(2, 4), name, rng.int(2, 4))


def inv_text(rng):
    items = []
    for _ in range(rng.int(0, 7)):
        k = rng.int(1, 10)
        if k <= 3:
            items.append("%s (%s)" % (rng.pick(ITEMS), rng.pick(SLOTS)))
        elif k <= 5:
            items.append(count_prefix(rng, rng.pick(PLURALS)))
        elif k <= 6:
            items.append(rng.pick(COINS))
        elif k <= 7:
            items.append("[%s]" % rng.pick(["HP=3", "Hall", "Tick"]))
        else:
            items.append(count_prefix(rng, rng.pick(ITEMS)))
    return items


def join_items(rng, items):
    sep = rng.pick([", ", ",", " , ", ",  "])
    return sep.join(items)


def block_inventory(rng):
    items = inv_text(rng)
    head = rng.pick(["You are carrying", "You are carrying ", "you are carrying", "YOU ARE CARRYING",
                     "You are carrying:"])
    if not items:
        return [head + rng.pick(["", " nothing", "."])]
    body = head + " " + join_items(rng, items) + rng.pick(["", "", "."])
    return wrap(rng, body)


def block_keys(rng):
    r = rng.int(1, 10)
    if r <= 2:
        return [rng.pick(["You have no keys.", "You have no keys", "you have no keys. "])]
    ks = []
    for _ in range(rng.int(1, 4)):
        ks.append(count_prefix(rng, rng.pick(KEYS)))
    body = rng.pick(["You have the following keys:", "You have the following keys", "You have the following keys :"])
    body += " " + join_items(rng, ks) + rng.pick([".", ".", "", " ."])
    return wrap(rng, body)


def notice_items(rng):
    items = []
    for _ in range(rng.int(1, 6)):
        k = rng.int(1, 10)
        if k <= 5:
            items.append(count_prefix(rng, rng.pick(ITEMS)))
        elif k <= 6:
            items.append(rng.pick(COINS))
        elif k <= 7:
            items.append("[%s] %s" % (rng.pick(["HP=2", "Tick"]), rng.pick(ITEMS)))
        else:
            items.append(count_prefix(rng, rng.pick(PLURALS)))
    return items


def block_notice(rng):
    body = "You notice " + join_items(rng, notice_items(rng)) + " here."
    if rng.chance(15):
        body = body.replace("here.", "here")
    lines = wrap(rng, body, 15, 50)
    if rng.chance(20) and len(lines) > 1:
        lines.insert(rng.int(1, len(lines) - 1), "[HP=44/60]:")
    return lines


def block_room(rng, with_notice=True):
    out = []
    if rng.chance(70):
        out.append(rng.pick(ROOMS))
    if with_notice and rng.chance(85):
        out += block_notice(rng)
    if rng.chance(85):
        out.append(rng.pick(EXITS))
    if rng.chance(25):
        out.append(rng.pick(SLOTLESS_JUNK))
    return out


def compose(rng):
    lines = []
    for _ in range(rng.int(1, 6)):
        k = rng.int(1, 12)
        if k <= 2:
            lines += block_inventory(rng)
        elif k <= 3:
            lines += block_keys(rng)
        elif k <= 7:
            lines += block_room(rng)
        elif k <= 9:
            lines.append(rng.pick(MOVES))
        elif k <= 10:
            lines.append(rng.pick(SLOTLESS_JUNK))
        elif k <= 11:
            lines.append("")
        else:
            lines.append("   " + rng.pick(ROOMS) + "  ")
    # inline header joins: glue the next section's first line onto the previous line
    if rng.chance(25) and len(lines) >= 2:
        i = rng.int(0, len(lines) - 2)
        lines[i] = lines[i] + " " + lines[i + 1]
        del lines[i + 1]
    eol = rng.pick(["\n", "\r\n", "\r\n", "\n"])
    text = eol.join(lines)
    if rng.chance(30):
        text += eol
    return text


HAND = [
    # inventory / equipped
    "You are carrying a stormmetal greataxe (Weapon Hand), 3 flasks of water, iron helm (Head), [HP=10]\r\nplatinum pieces",
    "You are carrying nothing.",
    "You are carrying",
    "You are carrying: oak staff (Weapon Hand), ruby ring (Finger), ruby ring (finger), RUBY RING (Finger)",
    "You are carrying ruby ring (Finger), ruby ring (Finger)",
    "You are carrying bone dagger,, ,jade amulet,,",
    "You are carrying   bone    dagger  (2) ,   jade   amulet   ",
    "You are carrying 12 gold crowns, 5 silver nobles, 3 copper farthings, 1 platinum piece, 20 runic coins",
    "You are carrying gold crown pouch, a gold crowned helm, silvery nobles, copper farthingale",
    "You are carrying [HP=3], [Hall], oak staff",
    "You are carrying (2), (x), ( )",
    "You are carrying rope (2), rope (3), 2 rope, rope",
    "You are carrying 2 rope (3), 1 rope (4), 0 rope, rope (0)",
    "You are carrying rope (a), rope (2) (3), rope (3) (a)",
    "You are carrying 99999999999 coins of tin, bone dagger",
    "You are carrying rope (99999999999), bone dagger",
    "You are carrying -2 apples, 0 pears, 1.5 bread, 1e2 arrows, &H10 stones, $5 tithe",
    "You are carrying   ,  , ",
    "You are carrying iron helm (Head)\n\nYou have no keys.",
    "You are carrying iron helm (Head)\nsilk sash (Waist)\nWealth: 1 gold crown\nEncumbrance: 4/20",
    "You are carrying iron helm (Head)\nName: Brogan\nbone dagger",
    "You are carrying iron helm (Head)\nRace: Dwarf\nbone dagger",
    "You are carrying iron helm (Head)\nClass: Warrior\nbone dagger",
    "You are carrying iron helm (Head)\n[HP=1]:\nbone dagger",
    "You are carrying iron helm (Head)\nObvious exits: north\nbone dagger",
    "You are carrying iron helm (Head)\nAlso here: a goblin\nbone dagger",
    "You are carrying iron helm (Head)\n*Combat On*\nbone dagger",
    "You are carrying iron helm (Head)\nwealth: 5\nbone dagger",
    "You are carrying iron helm (Head), bone dagger. You have the following keys: rusty key.",
    "You are carrying iron helm (Head)\nbone dagger You have no keys.",
    "You are carrying iron helm (Head)\nbone dagger Wealth: 4",
    "You are carrying iron helm (Head)\nbone dagger Encumbrance: 4/10",
    "You are carrying iron helm (Head)\nbone dagger you notice a rope here.",
    "You are carrying iron helm\nbone dagger you are carrying a spoon",
    "You are carrying a (Head) b",
    "You are carrying x (",
    "You are carrying x )",
    "You are carrying (Head)",
    "You are carrying iron helm (Head) (Head)",
    "You are carrying a (b) (c)",
    "You are carrying iron helm (Head)\nYou are carrying a second helm (Head)",
    "You are carrying iron helm (Head)\r\nYou are carrying a second helm (Head), spoon",
    # keys
    "You have the following keys: rusty key, iron keys, golden idols, silver idols (2), 2 golden idols",
    "You have the following keys:\nrusty key,\niron keys.",
    "You have the following keys: 3 iron keys, 2 bone keys (4), crystal key (2), brass keys (3)",
    "You have the following keys: 2 boxes, 3 glasses, 2 daisies, 2 buses, 2 brushes, 2 benches, 2 pies, 2 axes, 2 keys",
    "You have the following keys: 2 ss, 2 sses, 2 es, 2 xes, 2 ies, 2 s, 2 Boxes, 2 CHES, 2 SHES, 3 Hies",
    "You have the following keys: 2 sses, 2 aches, 2 ashes, 2 Xes, xes",
    "You have the following keys: idols, 2 idols, gate keys, Keys, keys",
    "You have the following keys: gate key.",
    "You have the following keys: gate key.; extra",
    "You have the following keys: gate key;",
    "You have the following keys: gate key ;. ",
    "You have the following keys",
    "You have the following keys:",
    "You have the following keys: no keys at all.",
    "You have the following keys: gate key\nStill more keys, wrapped.\nWealth: 4",
    "You have the following keys: gate key\nbrass key.\nrusty key",
    "You have the following keys: gate key\nbrass key\nName: Brogan",
    "You have the following keys: gate key\nbrass key\nYou have no keys.",
    "You have no keys.",
    "You have no keys\nYou are carrying rope",
    "YOU HAVE NO KEYS. extra",
    "rope You have no keys.",
    "You have no keys. You have the following keys: gate key.",
    "You have the following keys: gate key. You are carrying rope",
    "You have the following keys: gate key You notice a spoon here.",
    "You have the following keys: gate key\nbrass key *Combat* text",
    "You have the following keys: gate key\n\nbrass key.",
    # notices
    "You notice a dagger, 2 apples here.\nObvious exits: north",
    "You notice a dagger here.",
    "You notice a dagger",
    "You notice nothing in particular.",
    "You notice\nHERE.",
    "You notice a dagger, 2 apples,\n3 torches, gold crowns here.\nObvious exits: north",
    "You notice a dagger, [HP=3] spoon, [tick] here.\nObvious exits: north",
    "You notice a dagger,\n[HP=40/60]:\nspoon here.\nObvious exits: north",
    "You notice a dagger,\n[x\ny]\nspoon here.\nObvious exits: north",
    "You notice [nested [deep] bracket] a dagger [unclosed here.\nObvious exits: north",
    "You notice a dagger, spoon (3), spoon (2), SPOON, 2 spoon here.\nObvious exits: north",
    "You notice 12 gold crowns, 3 silver nobles, dagger here.\nObvious exits: north",
    "You notice 2 apples, apples (3), apples here.\nObvious exits: east",
    "You notice a dagger (0), spoon (-3), spoon () here.\nObvious exits: north",
    "You notice dagger (99999999999) here.\nObvious exits: north",
    "You notice 99999999999 daggers here.\nObvious exits: north",
    "You notice daggers (2000000000) here.\nObvious exits: north\nn\nYou notice daggers (2000000000) here.\nObvious exits: south",
    "You notice daggers (2000000000) here.\nObvious exits: north\nn\nYou notice daggers (2000000000) here.\nObvious exits: north\nn\nYou notice daggers (2000000000) here.\nObvious exits: east",
    # rooms
    "Old Guardhouse\nYou notice a dagger, 2 apples here.\nObvious exits: north\nOld Guardhouse\nYou notice a dagger, 3 apples here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger, 2 apples here.\nObvious exits: north\nn\nOld Guardhouse\nYou notice a dagger, 3 apples here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nn\nCobbled Lane\nYou notice a dagger here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nOld Guardhouse\nYou notice a dagger here.\nObvious exits: south",
    "old guardhouse\nYou notice a dagger here.\nObvious exits: north\nOLD GUARDHOUSE\nYou notice a dagger here.\nOBVIOUS EXITS: NORTH",
    "Cobbled Lane\nObvious exits: north\nYou notice a dagger here.\nn\nOld Guardhouse\nObvious exits: south",
    "Cobbled Lane\nYou notice a dagger here.\nn\nOld Guardhouse\nYou notice a spoon here.\nObvious exits: south",
    "You notice a dagger here.\nn\nYou notice a spoon here.\nObvious exits: south",
    "You notice a dagger here.\nNo movement here",
    "You notice a dagger here.\nn",
    "You notice a dagger here.\nObvious exits: north\nn\nYou notice a spoon here.",
    "You notice a dagger here.\nYou notice a spoon here.\nObvious exits: south",
    "Cobbled Lane\nYou notice a dagger here.\nBlank:\n\n   \nObvious exits: south",
    "Cobbled Lane\nName: Brogan\nRace: Dwarf\nClass: Warrior\nAlso here: a goblin.\n[HP=3]:\nYou notice a dagger here.\nObvious exits: south",
    "Cobbled Lane\nYou are carrying rope\nYou have the following keys: gate key.\nYou have no keys.\nYou notice a dagger here.\nObvious exits: south",
    "Cobbled Lane\nYou notice a dagger,\nspoon\nhere.\nObvious exits: south\nn\nYou notice\na bowl here.\nObvious exits: south",
    "[HP=10/10]:n\nOld Guardhouse\nYou notice a dagger here.\nObvious exits: north",
    "[HP=10/10]:   N  \nOld Guardhouse\nYou notice a dagger here.\nObvious exits: north",
    "[HP=10/10]:nn\nOld Guardhouse\nYou notice a dagger here.\nObvious exits: north",
    "x]:n\nOld Guardhouse\nYou notice a dagger here.\nObvious exits: north",
    "]:\nOld Guardhouse\nYou notice a dagger here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nsouthwest\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nsw\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nup\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nnorthwest\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nnortheast\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nsoutheast\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nwest\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\neast\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nsouth\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nne\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nse\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nnw\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\ns\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\ne\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nw\nYou notice a spoon here.\nObvious exits: north",
    "Old Guardhouse\nYou notice a dagger here.\nObvious exits: north\nnorth\nYou notice a spoon here.\nObvious exits: north",
    "", " ", "\n", "\r\n\r\n", "just some words", "[HP=3]:", "You", "You notice", "You notice here.",
    "You notice ,,, here.", "You notice [only brackets] here.", "You notice gold crowns here.",
    "Wealth: 1 gold crown\nEncumbrance: 3/9\n*Combat Off*",
    "  You are carrying   rope  \n  You have no keys.  \n  You notice   a bowl   here.  \nObvious exits: up",
    "You are carrying rope\rYou notice a bowl here.\rObvious exits: up",
    "You are carrying rope\r\n\r\nYou notice a bowl here.\r\nObvious exits: up\r\n",
    "You notice a dagger here.\r\nObvious exits: north\r\nYou are carrying rope (2)\r\nYou have the following keys: gate key, 2 gate keys.\r\n",
]

NAMEQTY = ["rope", "rope (2)", " rope  (2) ", "rope (2) (3)", "rope (3) (2)", "rope (0)", "rope (-2)", "rope (1)",
           "rope (a)", "rope (2) (a)", "rope (a) (2)", "rope ( 2 )", "rope ()", "rope (", "rope )", ")", "(", "()",
           "(2)", "2 rope", "rope (2) extra", "rope(2)", "rope (2)(3)", "rope (1.6)", "rope (2.5)", "rope (1e2)",
           "rope (&H10)", "rope ($5)", "rope (99999999999)", "rope (1,000)", "rope (-0)", "rope (+3)", "", "   ",
           "iron helm (Head)", "iron helm (Head) (2)", "x (3)", "(3) x", "a (b) (4)", "a (4) (b)", "a ((4))",
           "a (4))", "a ((4)", "a (4) )", "rope (2)\t", "rope\t(2)", "rope (  )", "rope (2 )", "rope ( 2)"]

NAMES_FOR_ITEMS = ["iron helm", "IRON HELM", "  iron helm  ", "rope", "Rope", "gate key", "spoon", "bone dagger",
                   "", "   ", "helm", "iron helm (Head)", "ruby ring", "golden idol", "ROPE"]
IPR_SETS = [
    {"sEquipped": ["iron helm (Head)", "ruby ring (Finger)"], "sInventory": ["rope (2)", "bone dagger"],
     "sKeys": ["gate key", "golden idol (2)"]},
    {"sEquipped": [""], "sInventory": [""], "sKeys": [""]},
    {"sEquipped": ["  ", "iron helm"], "sInventory": ["   "], "sKeys": ["gate key (3)"]},
    {"sEquipped": [], "sInventory": ["Rope  (4)"], "sKeys": []},
    {"sEquipped": ["a (b) (Head)", "spoon (3)"], "sInventory": ["spoon (3)", "x (a)"], "sKeys": ["rope (a) (2)"]},
    {"sEquipped": ["iron helm (Head)"], "sInventory": ["rope (99999999999)"], "sKeys": ["gate key"]},
    {"sEquipped": [], "sInventory": [], "sKeys": []},
]
FLAGS = [[], [True, True, True], [False, True, True], [True, False, True], [True, True, False],
         [False, False, True], [False, True, False], [True, False, False], [False, False, False]]


def build():
    cases = []
    n = 0
    for t in HAND:
        cases.append({"id": "h%03d" % n, "proc": "ParseGameTextInventory", "args": [t]})
        n += 1
    rng = Rng(0x22D0C5)
    for i in range(520):
        cases.append({"id": "r%03d" % i, "proc": "ParseGameTextInventory", "args": [compose(rng)]})
    for i, t in enumerate(NAMEQTY):
        cases.append({"id": "q%03d" % i, "steps": [{"call": "ParseNameAndQty",
                      "args": [t, {"ref": "string", "value": ""}, {"ref": "long", "value": 0}], "as": "pnq"}]})
    k = 0
    for si, st in enumerate(IPR_SETS):
        fields = {kk: v for kk, v in st.items()}
        fields["sGround"] = []
        for nm in NAMES_FOR_ITEMS:
            for fl in FLAGS[:2] if si > 3 else FLAGS:
                cases.append({"id": "i%03d" % k, "proc": "ItemNameInParseResult",
                              "args": [{"udt": "ItemParseResult", "fields": fields}, nm] + fl})
                k += 1
    return cases


def main():
    scn = {
        "family": "game_text",
        "description": "FAM-22 text half (modItemParse): ParseGameTextInventory with its section collectors, "
                       "tokenisers and room-wise ground consolidation, ParseNameAndQty, ItemNameInParseResult.",
        "notes": ["Texts are invented MajorMUD-style lines; generated by tools/oracle/gen/game_text.py.",
                  "ParseGameTextInventory returns ItemParseResult; an empty list is one empty string, as in MME.",
                  "ParseNameAndQty is called with ByRef String/Long outputs (outputs <as>.refs)."],
        "database": False,
        "modules": ["modItemParse.bas"],
        "tolerance": {"default": 0},
        "cases": build(),
    }
    with open(OUT, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("wrote", OUT, len(scn["cases"]), "cases")


if __name__ == "__main__":
    main()

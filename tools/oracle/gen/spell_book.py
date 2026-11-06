"""Writes fixtures/scenarios/spell_book.json: python3 -I tools/oracle/gen/spell_book.py fixtures/scenarios/spell_book.json
The Spell Book window (card P4-32): frmSpellBook cmdListSpells_Click (which spells a class, level and alignment may
learn or cast, and whether the loaded character builds the rows), cmbClass_Click (a class sets the magery boxes) and
CopyGlobalChar (the global filter fills the window). Deterministic; the stock database only."""
import glob
import hashlib
import json
import os
import sqlite3
import sys

OUT = sys.argv[1]
con = sqlite3.connect(glob.glob(os.path.expanduser("~/.cache/touchstone/mdb/eba20f06*.sqlite"))[0])
classes = [(r[0], r[1], r[2], r[3]) for r in con.execute("select Number, Name, MageryType, MageryLVL from Classes order by Number")]
CLASS_NUMS = [c[0] for c in classes]


def H(*a):
    return int(hashlib.sha256(":".join(map(str, a)).encode()).hexdigest()[:12], 16)


def combo(items, index):
    return {"Items": items, "ListIndex": index}


CLASS_ITEMS = [0] + CLASS_NUMS      # "none" first, then the classes


def forms(cls=0, level="1", align=0, magery=0, mlvl=0, filt=0, glevel="1", gclass=0, gclass1=None, galign=0, extra=None):
    """frmMain's controls the window reads. cls is a class number (0 none); the combos' ItemData is the class number."""
    ci = CLASS_ITEMS.index(cls)
    gi = CLASS_ITEMS.index(gclass)
    g1 = gi if gclass1 is None else CLASS_ITEMS.index(gclass1)
    f = {
        "cmbClass": combo(CLASS_ITEMS, ci), "txtLevel": {"Text": level}, "cmbAlignment": {"ListIndex": align},
        "cmbSpellMagery": {"ListIndex": magery, "ListCount": 6}, "cmbSpellMageryLevel": {"ListIndex": mlvl, "ListCount": 4},
        "chkGlobalFilter": {"Value": filt}, "txtGlobalLevel": {"$array": {"0": {"Text": glevel}, "1": {"Text": glevel}}},
        "cmbGlobalClass": {"$array": {"0": {"Items": CLASS_ITEMS, "ListIndex": gi},
                                       "1": {"Items": CLASS_ITEMS, "ListIndex": g1}}},
        "cmbGlobalRace": {"$array": {"0": {"Items": [0, 1], "ListIndex": 0}}},
        "cmbGlobalAlignment": {"ListIndex": galign},
        "lvSpellBook": {"$recordlv": []},
    }
    f.update(extra or {})
    return {"frmMain": f}


cases = []
LIST = {"call": "cmdListSpells_Click", "capture": {"rows": "%lvn.lvSpellBook", "char": "%g.gBookChar"}}


def add(cid, steps, f, globs=None):
    c = {"id": cid, "steps": steps, "forms": f}
    g = {"nNMRVer": 1.83, "bOnlyInGame": True}
    g.update(globs or {})
    c["globals"] = g
    cases.append(c)


# every class at a few levels and alignments
for i, cls in enumerate(CLASS_NUMS):
    for level in ("1", "20"):
        add("list-c%d-l%s" % (cls, level), [LIST], forms(cls, level, align=(H("al", cls, level) % 4)))
    add("list-c%d-l999" % cls, [LIST], forms(cls, "999", align=0))
add("list-none-l50", [LIST], forms(0, "50"))
add("list-none-l0", [LIST], forms(0, "0"))
for al in (1, 2, 3):
    add("list-mage-a%d" % al, [LIST], forms(12, "40", align=al))
    add("list-priest-a%d" % al, [LIST], forms(5, "40", align=al))
add("list-all-in-game-off", [LIST], forms(12, "60"), {"bOnlyInGame": False})
add("list-kai-noauto", [LIST], forms(15, "60"), {"bDisableKaiAutolearn": True})
add("list-kai-auto", [LIST], forms(15, "60"), {"bDisableKaiAutolearn": False})
add("list-old-data", [LIST], forms(11, "30"), {"nNMRVer": 1.0})
add("list-data-17", [LIST], forms(9, "30"), {"nNMRVer": 1.7})
add("list-data-18", [LIST], forms(9, "30"), {"nNMRVer": 1.8})
add("list-level-text", [LIST], forms(12, "abc"))
add("list-level-fraction", [LIST], forms(12, "12.5"))
add("list-level-negative", [LIST], forms(12, "-5"))
add("list-level-huge", [LIST], forms(12, "40000"))
# whether the loaded character builds the rows: the filter on with a level, the same class, matching magery boxes
M = {"nGlobalCharMagery": 1, "nGlobalCharMageryLVL": 3}
add("char-yes", [LIST], forms(12, "30", magery=1, mlvl=3, filt=1, glevel="30", gclass=12), M)
add("char-no-filter", [LIST], forms(12, "30", magery=1, mlvl=3, filt=0, glevel="30", gclass=12), M)
add("char-no-level", [LIST], forms(12, "30", magery=1, mlvl=3, filt=1, glevel="0", gclass=12), M)
add("char-other-class", [LIST], forms(12, "30", magery=1, mlvl=3, filt=1, glevel="30", gclass=11), M)
add("char-class-none", [LIST], forms(0, "30", magery=1, mlvl=3, filt=1, glevel="30", gclass=12), M)
add("char-global-none", [LIST], forms(12, "30", magery=1, mlvl=3, filt=1, glevel="30", gclass=0), M)
add("char-magery-any", [LIST], forms(12, "30", magery=0, mlvl=3, filt=1, glevel="30", gclass=12), M)
add("char-mlvl-any", [LIST], forms(12, "30", magery=1, mlvl=0, filt=1, glevel="30", gclass=12), M)
add("char-magery-differs", [LIST], forms(12, "30", magery=2, mlvl=3, filt=1, glevel="30", gclass=12), M)
add("char-mlvl-differs", [LIST], forms(12, "30", magery=1, mlvl=2, filt=1, glevel="30", gclass=12), M)
add("char-yes-priest", [LIST], forms(5, "30", magery=2, mlvl=3, filt=1, glevel="30", gclass=5), {"nGlobalCharMagery": 2, "nGlobalCharMageryLVL": 3})

# CopyGlobalChar: the global filter fills the window
COPY = {"call": "CopyGlobalChar"}
RC = {"cls": "%c.cmbClass.ListIndex", "al": "%c.cmbAlignment.ListIndex", "lvl": "%c.txtLevel.Text"}


def copy_case(cid, **kw):
    add(cid, [dict(COPY, capture=RC)], forms(**kw))


copy_case("copy-off", cls=0, level="7", filt=0, glevel="30", gclass=12, galign=2)
copy_case("copy-on", cls=0, level="7", filt=1, glevel="30", gclass=12, galign=2)
copy_case("copy-on-no-class", cls=3, level="7", filt=1, glevel="30", gclass=0, galign=0)
copy_case("copy-on-no-level", cls=0, level="7", filt=1, glevel="0", gclass=5, galign=3)
copy_case("copy-on-level-text", cls=0, level="7", filt=1, glevel="12abc", gclass=5, galign=1)
copy_case("copy-on-alignment-any", cls=0, level="7", align=2, filt=1, glevel="9", gclass=9, galign=0)
copy_case("copy-bars-differ", cls=0, level="7", filt=1, glevel="30", gclass=12, gclass1=5, galign=1)
copy_case("copy-bar1-none", cls=4, level="7", filt=1, glevel="30", gclass=12, gclass1=0, galign=1)
copy_case("copy-bar0-none", cls=4, level="7", filt=1, glevel="30", gclass=0, gclass1=12, galign=1)

# cmbClass_Click: a class sets the magery boxes
CLICK = {"call": "cmbClass_Click", "capture": {"mlvl": "%c.cmbSpellMageryLevel.ListIndex", "m": "%c.cmbSpellMagery.ListIndex"}}
for cls in CLASS_NUMS + [0]:
    add("class-%d" % cls, [CLICK], forms(cls, "1", magery=0, mlvl=0))
add("class-boxes-short", [CLICK], forms(15, "1", extra={"cmbSpellMagery": {"ListIndex": 4, "ListCount": 3},
                                                       "cmbSpellMageryLevel": {"ListIndex": 2, "ListCount": 1}}))

FRM = {
    "file": "frmSpellBook.frm",
    "decls": ["nMagicLVL", "nMagery"],
    "extra": ["Public gBookChar As Boolean"],
    "me_is_form": True,
    "strip_prefix": ["frmSpellBook"],
    "procs": ["cmdListSpells_Click", "cmbClass_Click", "CopyGlobalChar"],
    "stub_procs": ["lvSpellBook_ItemClick", "RefreshLearnedSpellColors_byLV"],
    "subs": {"cmdListSpells_Click": {
        "Call PopulateCharacterProfile(tChar, False, True, , , bDontUseCharacter)":
            "Call PopulateCharacterProfile(tChar, False, True, , , bDontUseCharacter)\ngBookChar = tChar.bIsLoadedCharacter"}},
}
def spell_row(r):
    return [r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]]


cols = ["Number", "Learnable", "Learned From", "Casted By", "Magery", "MageryLVL", "ReqLevel", "Classes"]
srows = []
for r in con.execute('select Number, Name, Learnable, length("Learned From"), length("Casted By"), Magery, MageryLVL, ReqLevel, Classes, '
                     + ", ".join('"Abil-%d", "AbilVal-%d"' % (i, i) for i in range(10)) + " from Spells order by Number"):
    ab = [x for i in range(10) for x in (r[9 + 2 * i], r[10 + 2 * i]) if r[9 + 2 * i]]
    srows.append([r[0], r[1] or "", r[2], r[3] or 0, r[4] or 0, r[5], r[6], r[7], r[8] or "", ab])
DATA = {"spells": srows, "classes": [list(c[:1]) + [c[2], c[3]] for c in classes]}

scn = {
    "family": "spell_book",
    "description": "Spell Book window: cmdListSpells_Click, cmbClass_Click, CopyGlobalChar",
    "notes": [
        "MME's own frmSpellBook procedures run against the stock database. The list is a recording ListView read as "
        "'number' or 'number:tag' per row in the order AddSpell2LV added it (the sort after it is recorded as an lv_refresh_sort "
        "event). 'char' is whether PopulateCharacterProfile built the profile for the loaded character.",
        "Row cells are not compared here: AddSpell2LV is the rows family's. The class box holds 'none' (0) and the 15 classes; "
        "combos are given as ItemData lists.",
        "data.spells is [number, name, learnable, length of Learned From, length of Casted By, magery, magery level, required level, "
        "Classes, [ability, value ...]]; data.classes [number, magery type, magery level].",
        "The window's list-item click and the learned-spell colouring are stubs (UI after the list is built).",
    ],
    "database": True,
    "data": DATA,
    "frm": FRM,
    "cases": cases,
}
with open(OUT, "w") as f:
    json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    f.write("\n")
print("%d cases" % len(cases), file=sys.stderr)

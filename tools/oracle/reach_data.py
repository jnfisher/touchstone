"""The compact table rows the reachability family ships in its fixture header ("data"), read from the converted
stock database. Only the columns modReachability reads. Rooms keep their ten exit texts joined by '|' (no exit text
contains one)."""
import sqlite3

EXITS = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"]


def _s(x):
    return "" if x is None else str(x)


def rows(db, only=None):
    """{"rooms", "tbinfo", "spells", "monsters", "items"}; `only` is the scenario's db_only (keys per table)."""
    only = only or {}
    c = sqlite3.connect("file:%s?mode=ro" % db, uri=True)
    out = {}
    keep = only.get("Rooms")
    keep = None if keep is None else {tuple(k) if isinstance(k, list) else (k,) for k in keep}
    rooms = []
    for r in c.execute('SELECT "Map Number","Room Number",Name,NPC,CMD,Spell,Placed,%s FROM Rooms ORDER BY rowid'
                       % ",".join(EXITS)):
        if keep is not None and (r[0], r[1]) not in keep and (r[0],) not in keep:
            continue
        ex = ["" if (e is None or str(e) == "0") else str(e) for e in r[7:]]
        assert not any("|" in e for e in ex)
        rooms.append([r[0], r[1], _s(r[2]), r[3] or 0, r[4] or 0, r[5] or 0, _s(r[6]), "|".join(ex)])
    out["rooms"] = rooms

    def sel(table, cols, key=0, order="rowid"):
        k = only.get(table)
        k = None if k is None else set(k)
        res = []
        for r in c.execute("SELECT %s FROM %s ORDER BY %s" % (cols, table, order)):
            if k is None or r[key] in k:
                res.append(r)
        return res
    out["tbinfo"] = [[r[0], r[1] or 0, _s(r[2]), _s(r[3])]
                     for r in sel("TBInfo", 'Number,LinkTo,Action,"Called From"')]
    ab = ",".join("[Abil-%d],[AbilVal-%d]" % (i, i) for i in range(10))
    sp = []
    for r in sel("Spells", 'Number,ReqLevel,Dur,"Casted By",' + ab):
        flat = [x or 0 for x in r[4:]]
        while flat and flat[-1] == 0:
            flat.pop()
        sp.append([r[0], r[1] or 0, r[2] or 0, _s(r[3]), flat])
    out["spells"] = sp
    out["monsters"] = [[r[0], _s(r[1]), r[2] or 0, 1 if r[3] else 0, _s(r[4])]
                       for r in sel("Monsters", 'Number,Name,GreetTXT,"In Game","Summoned By"', order="Number")]
    out["items"] = [[r[0], _s(r[1])] for r in sel("Items", 'Number,"Obtained From"') if r[1]]
    return out

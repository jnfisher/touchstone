"""Shims: stand-ins for what MME's formula modules touch that is not VB code.

- `Database` / `Recordset`: DAO table-type recordsets over the converted stock SQLite.
- `FormShim` / `ControlShim`: frmMain and its controls as plain value holders the scenario fills.
- `Clipboard`, `Forms`, Win32 and UI procedures: recorded no-ops.

Nothing here reads MME source; the VB side is whatever the interpreter loaded.
"""
import sqlite3

from vbi import values as V
from vbi.errors import VBError
from vbi.objects import ExtObject
from vbi.values import NULL, NOTHING

# Index names MME sets (`tabX.Index = "..."`) and the columns each orders by. The stock .mdb names
# them; mdbtools does not export index definitions, so this is read off how MME calls Seek: one key
# for every table except Rooms, which is seeked with (map, room).
INDEXES = {
    "pkitems": ("Items", ("Number",)),
    "pkclasses": ("Classes", ("Number",)),
    "pkraces": ("Races", ("Number",)),
    "pkspells": ("Spells", ("Number",)),
    "pkmonsters": ("Monsters", ("Number",)),
    "pkshops": ("Shops", ("Number",)),
    "pktbinfo": ("TBInfo", ("Number",)),
    "idxrooms": ("Rooms", ("Map Number", "Room Number")),
}

# global name -> table, as MME's open procedure assigns them (Set tabX = DB.OpenRecordset("T"))
RECORDSET_GLOBALS = {
    "tabItems": "Items", "tabClasses": "Classes", "tabRaces": "Races", "tabSpells": "Spells",
    "tabMonsters": "Monsters", "tabShops": "Shops", "tabRooms": "Rooms", "tabInfo": "Info",
    "tabTBInfo": "TBInfo", "tabLairs": "Lairs",
}


def _vb_value(raw, decl="TEXT"):
    """A SQLite value as the Variant DAO would hand back: Long, Double or String.
    (Every integer column in the stock .mdb is Long Integer, every real one Double.) The .mdb
    declares its columns NOT NULL and its text columns allow zero length, while the SQLite
    conversion stores an empty text as NULL, so NULL comes back as the column's empty value."""
    if raw is None:
        return {"INTEGER": V.I32(0), "REAL": 0.0}.get(decl.upper(), "")
    if isinstance(raw, bool):
        return V.I16(-1 if raw else 0)
    if isinstance(raw, int):
        return V.mk_i32(raw) if -2147483648 <= raw <= 2147483647 else float(raw)
    return raw


def _key(v):
    """Comparison key for Seek: numbers by value, strings case-insensitively (Jet default)."""
    if v is NULL or v is None:
        return (0, 0)
    if isinstance(v, str):
        return (2, v.lower())
    try:
        return (1, V.as_float(v))
    except Exception:
        return (3, str(v))


class Database:
    """Read-only handle on the converted stock database; tables are loaded on first use."""

    def __init__(self, path):
        self.path = path
        self._tables = {}

    def table(self, name):
        t = self._tables.get(name)
        if t is None:
            con = sqlite3.connect("file:%s?mode=ro" % self.path, uri=True)
            try:
                info = list(con.execute('PRAGMA table_info("%s")' % name.replace('"', "")))
                cols = [r[1] for r in info]
                decls = [r[2] for r in info]
                if not cols:
                    raise VBError(3078, "The Microsoft Jet database engine cannot find the input table "
                                        "or query '%s'." % name)
                rows = [tuple(_vb_value(x, d) for x, d in zip(r, decls))
                        for r in con.execute('SELECT * FROM "%s" ORDER BY rowid' % name)]
            finally:
                con.close()
            t = (cols, rows)
            self._tables[name] = t
        return t

    def open_recordset(self, name):
        return Recordset(self, name)


class _Fields(ExtObject):
    def __init__(self, rs):
        self.rs = rs

    def vb_get(self, name, args):
        n = name.lower()
        if n == "count":
            return V.I16(len(self.rs.cols))
        if n == "item":
            return self.rs.field(args[0])
        raise VBError(438)

    def vb_default(self, args):
        return self.rs.field(args[0])


class Recordset(ExtObject):
    """DAO table-type Recordset. Cursor starts on the first record in insertion order (no index)."""

    def __init__(self, db, table):
        self.db = db
        self.tname = table
        self._cols = None
        self._rows = None
        self.index = ""
        self.order = None     # row positions in current index order
        self.pos = 0          # position within `order`; len(order) means EOF, -1 BOF
        self.no_match = False
        self.closed = False

    # -- lazily loaded data
    def _load(self):
        if self._rows is None:
            self._cols, self._rows = self.db.table(self.tname)
            self._colix = {c.lower(): i for i, c in enumerate(self._cols)}
            self.order = list(range(len(self._rows)))
            self.pos = 0
            self._seek_map = None

    @property
    def cols(self):
        self._load()
        return self._cols

    def _set_index(self, name):
        self._load()
        if name == "":
            self.index = ""
            self.order = list(range(len(self._rows)))
            self._seek_map = None
            return
        spec = INDEXES.get(name.lower())
        if spec is None or spec[0].lower() != self.tname.lower():
            raise VBError(3015, "'%s' is not an index in this table." % name)
        kcols = [self._colix[c.lower()] for c in spec[1]]
        keys = [tuple(_key(r[i]) for i in kcols) for r in self._rows]
        self.order = sorted(range(len(self._rows)), key=lambda i: (keys[i], i))
        self.index = name
        # first record in index order for each key (Jet's Seek "=" finds the first match)
        m = {}
        for i in self.order:
            m.setdefault(keys[i], i)
        self._seek_map = (len(kcols), m)
        self.pos = 0 if self.order else 0

    # -- cursor
    def _current(self):
        self._load()
        if self.pos < 0 or self.pos >= len(self.order):
            raise VBError(3021, "No current record.")
        return self._rows[self.order[self.pos]]

    def field(self, which):
        row = self._current()
        if isinstance(which, str):
            i = self._colix.get(which.lower())
            if i is None:
                raise VBError(3265, "Item not found in this collection.")
        else:
            i = V.int_value(which)
            if i < 0 or i >= len(self._cols):
                raise VBError(3265, "Item not found in this collection.")
        return row[i]

    def vb_default(self, args):
        return self.field(args[0])

    def vb_bang(self, name):
        return self.field(name)

    def vb_get(self, name, args):
        n = name.lower()
        self._load()
        if n == "fields":
            return self.field(args[0]) if args else _Fields(self)
        if n == "nomatch":
            return self.no_match
        if n == "eof":
            return len(self.order) == 0 or self.pos >= len(self.order)
        if n == "bof":
            return len(self.order) == 0 or self.pos < 0
        if n == "recordcount":
            return V.mk_i32(len(self._rows))
        if n == "index":
            return self.index
        if n == "name":
            return self.tname
        if n == "bookmark":
            if self.pos < 0 or self.pos >= len(self.order):
                raise VBError(3021, "No current record.")
            return (self.index, self.order[self.pos])
        if n == "seek":
            self._seek(args)
            return None
        if n == "movefirst":
            if not self.order:
                raise VBError(3021, "No current record.")
            self.pos = 0
            return None
        if n == "movelast":
            if not self.order:
                raise VBError(3021, "No current record.")
            self.pos = len(self.order) - 1
            return None
        if n == "movenext":
            if self.pos >= len(self.order):
                raise VBError(3021, "No current record.")
            self.pos += 1
            return None
        if n == "moveprevious":
            if self.pos < 0:
                raise VBError(3021, "No current record.")
            self.pos -= 1
            return None
        if n == "close":
            self.closed = True
            return None
        raise VBError(438)

    def vb_set(self, name, args, value):
        n = name.lower()
        if n == "index":
            self._set_index(V.c_str(value))
            return
        if n == "bookmark":
            idx, row = value
            self._set_index(idx)
            self.pos = self.order.index(row)
            return
        raise VBError(438)

    def vb_iter(self):
        raise VBError(438)

    def _seek(self, args):
        self._load()
        if not self.index:
            raise VBError(3019, "Operation invalid without a current index.")
        op = V.c_str(args[0])
        if op != "=":
            raise VBError(3251, "Operation is not supported for this type of object.")
        nkeys, m = self._seek_map
        key = tuple(_key(a) for a in args[1:])
        if len(key) > nkeys or not key:
            raise VBError(3251, "Operation is not supported for this type of object.")
        if len(key) < nkeys:
            # a short key matches on the leading columns
            hit = None
            for k, i in m.items():
                if k[:len(key)] == key and (hit is None or self.order.index(i) < self.order.index(hit)):
                    hit = i
        else:
            hit = m.get(key)
        if hit is None:
            self.no_match = True
            self.pos = len(self.order)      # DAO leaves the cursor undefined; EOF is what MME observes
        else:
            self.no_match = False
            self.pos = self.order.index(hit)


# --------------------------------------------------------------------------- forms and controls

_PROP_DEFAULTS = {"text": "", "caption": "", "tag": "", "name": "", "listindex": V.I32(-1)}


def _plain(x):
    """A scenario JSON value as a VB value (typed objects are decoded by the caller)."""
    if isinstance(x, bool):
        return x
    if isinstance(x, int):
        return V.I16(x) if -32768 <= x <= 32767 else V.mk_i32(x)
    return x


class ControlShim(ExtObject):
    """A control as a bag of properties. Unset properties read as their VB default (empty string for
    Text/Caption/Tag, -1 for ListIndex, 0 otherwise). Indexed properties (ItemData, List) hold a
    dict keyed by index; a control-array element is a sub-control."""

    def __init__(self, props=None, decode=_plain):
        self.props = {}
        self.items = {}
        self.decode = decode
        for k, v in (props or {}).items():
            self.load(k, v)

    def load(self, key, v):
        kl = key.lower()
        if isinstance(v, list):
            self.items[kl] = {i: self.decode(x) for i, x in enumerate(v)}
        elif isinstance(v, dict) and v.get("t") is None:
            self.items[kl] = {int(i): self.decode(x) for i, x in v.items()}
        else:
            self.props[kl] = self.decode(v)

    def vb_get(self, name, args):
        n = name.lower()
        if args:
            tbl = self.items.get(n)
            i = V.int_value(args[0])
            if tbl is None or i not in tbl:
                if n in ("itemdata", "list"):
                    return V.I32(0) if n == "itemdata" else ""
                raise VBError(9)
            return tbl[i]
        if n in self.props:
            return self.props[n]
        if n in _PROP_DEFAULTS:
            return _PROP_DEFAULTS[n]
        if n in ("listcount", "value", "count"):
            return V.I16(len(self.items.get("list", {})) if n == "listcount" else 0)
        return V.I16(0)

    def vb_set(self, name, args, value):
        n = name.lower()
        if args:
            self.items.setdefault(n, {})[V.int_value(args[0])] = value
        else:
            self.props[n] = value

    def vb_default(self, args):
        if args:
            return self.vb_get("value", [])
        return self.vb_get("value", [])


class FormShim(ExtObject):
    """frmMain (or any form): `form.control.Property`, control arrays `form.lbl(3).Caption`, and
    the form's own public variables (set from the scenario or by code)."""

    def __init__(self, spec=None, decode=_plain):
        self.controls = {}
        self.vars = {}
        self.decode = decode
        for name, props in (spec or {}).items():
            nl = name.lower()
            if isinstance(props, dict) and "$array" in props:
                self.controls[nl] = {int(i): ControlShim(p, decode) for i, p in props["$array"].items()}
            elif isinstance(props, dict):
                self.controls[nl] = ControlShim(props, decode)
            else:
                self.vars[nl] = decode(props)

    def vb_get(self, name, args):
        n = name.lower()
        if n in self.vars:
            return self.vars[n]
        c = self.controls.get(n)
        if c is None:
            c = self.controls[n] = ControlShim(decode=self.decode)
        if isinstance(c, dict):
            i = V.int_value(args[0])
            sub = c.get(i)
            if sub is None:
                sub = c[i] = ControlShim(decode=self.decode)
            return sub
        return c

    def vb_set(self, name, args, value):
        self.vars[name.lower()] = value


class ClipboardShim(ExtObject):
    def __init__(self):
        self.text = ""

    def vb_get(self, name, args):
        n = name.lower()
        if n == "gettext":
            return self.text
        if n == "settext":
            self.text = V.c_str(args[0])
            return None
        if n == "clear":
            self.text = ""
            return None
        raise VBError(438)


class FormsCollection(ExtObject):
    """`Forms`: no form is ever loaded in the oracle."""

    def vb_get(self, name, args):
        if name.lower() == "count":
            return V.I16(0)
        raise VBError(438)

    def vb_iter(self):
        return iter(())

    def vb_default(self, args):
        raise VBError(9)


# --------------------------------------------------------------------------- environment hooks

class Recorder:
    """What the UI-side shims were asked to do during a case (MsgBox text, handled errors)."""

    def __init__(self):
        self.events = []

    def add(self, kind, **kw):
        self.events.append(dict(kind=kind, **kw))


def msgbox_result(buttons):
    """What a MsgBox returns when nobody clicks: the default button."""
    b = int(buttons) if buttons is not None else 0
    base = b & 7
    default = (b >> 8) & 3
    table = {0: (1,), 1: (1, 2), 2: (3, 4, 5), 3: (6, 7, 2), 4: (6, 7), 5: (4, 2)}.get(base, (1,))
    return V.I16(table[min(default, len(table) - 1)])

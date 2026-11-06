"""Environment and case runner: loads MME's modules, wires the shims, runs scenario cases and
turns the results into fixtures (deterministic JSON)."""
import glob
import hashlib
import json
import os
import re
import subprocess

from vbi.interp import Interpreter
from vbi import values as V
from vbi.errors import VBError, OracleUnbound
from vbi.objects import VBArray, UDTInstance, ClsInstance, Var
from vbi.values import NULL, EMPTY, NOTHING
import shims
import scenario as scn_mod
import frm as frm_mod

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
GENERATOR_VERSION = 1
MODULES = ["modMain.bas", "modMMudDatabase.bas", "modMMudFunc.bas", "modSyntaxsFunc.bas",
           "clsMonsterAttackSim.cls", "modExpPerHour.bas"]
STOCK_MDB_SHA = "eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f"

# Procedures of the loaded modules that are replaced by recorded no-ops (UI, Win32, files, shutdown).
SHIMMED_PROCS = ["DebugLogPrint", "AppTerminate", "Sleep", "SendMessage", "GetWindowLong", "SetWindowLong",
                 "SetWindowPos", "Unload", "SetClipboardText"]


class OracleFatal(Exception):
    pass


# ----------------------------------------------------------------------------- locations

def mme_dir():
    env = os.environ.get("MME_DIR")
    if env:
        return env
    out = subprocess.run(["bash", os.path.join(ROOT, "tools", "mme-fetch.sh")], capture_output=True,
                         text=True, check=True).stdout.strip()
    return out.splitlines()[-1]


def mme_sha(mme):
    base = os.path.basename(os.path.normpath(mme))
    if re.fullmatch(r"[0-9a-f]{40}", base):
        return base
    with open(os.path.join(ROOT, "tools", "mme-fetch.sh")) as f:
        m = re.search(r"MME_SHA:-([0-9a-f]{40})", f.read())
    return m.group(1) if m else "unknown"


def db_path():
    env = os.environ.get("ORACLE_DB")
    if env:
        return env
    cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
    p = os.path.join(cache, "mdb", STOCK_MDB_SHA + ".sqlite")
    if not os.path.exists(p):
        subprocess.run(["bash", os.path.join(ROOT, "tools", "mdb-fetch.sh")], check=True, capture_output=True)
    return p


# ----------------------------------------------------------------------------- value coding

def decode(x):
    """Scenario JSON -> VB value. Typed form: {"t": "cur|single|i16|i32|byte|f64|str|empty|null|missing", "v": ...}; "missing" leaves an Optional argument out."""
    if isinstance(x, dict) and "t" in x:
        t, v = x["t"], x.get("v")
        if t == "cur":
            return V.c_cur(float(v))
        if t == "single":
            return V.f32(float(v))
        if t == "i16":
            return V.mk_i16(int(v))
        if t == "i32":
            return V.mk_i32(int(v))
        if t == "byte":
            return V.mk_u8(int(v))
        if t == "f64":
            return float(v)
        if t == "str":
            return str(v)
        if t == "empty":
            return EMPTY
        if t == "null":
            return NULL
        if t == "missing":
            return V.MISSING
        raise ValueError("unknown typed value %r" % (x,))
    if isinstance(x, bool):
        return x
    if isinstance(x, int):
        return V.I16(x) if -32768 <= x <= 32767 else V.mk_i32(x)
    return x


class F32Json(float):
    pass


def cur_display(n):
    sign = "-" if n < 0 else ""
    n = abs(int(n))
    whole, frac = divmod(n, 10000)
    s = "%s%d" % (sign, whole)
    if frac:
        s += "." + ("%04d" % frac).rstrip("0")
    return s


def f32_repr(x):
    import struct
    for p in range(1, 10):
        s = "%.*g" % (p, x)
        if struct.unpack("f", struct.pack("f", float(s)))[0] == x:
            return s
    return repr(x)


def encode(v):
    """VB value -> JSON-able value. Integers/Boolean/String/Double are plain (Double always prints
    with a point or exponent); Currency and Single are tagged so the Rust side keeps their type."""
    t = type(v)
    if t is bool:
        return v
    if t in (V.I16, V.I32, V.U8):
        return int(v)
    if t is V.Cur:
        return {"cur": int(v), "s": cur_display(v)}
    if t is V.F32:
        return {"single": float(v), "s": f32_repr(float(v))}
    if t is float:
        if v != v or v in (float("inf"), float("-inf")):
            return {"double": repr(v)}
        return v
    if t is str:
        return v
    if v is NULL:
        return {"null": True}
    if v is EMPTY:
        return {"empty": True}
    if v is NOTHING:
        return {"nothing": True}
    if isinstance(v, VBArray):
        return [encode(x) for x in (v.data or [])]
    if isinstance(v, UDTInstance):
        return {k: encode(x) for k, x in v.fields.items()}
    if isinstance(v, (ClsInstance, shims.ExtObject)):
        return {"object": True}
    if isinstance(v, int):
        return int(v)
    return {"unrepresentable": type(v).__name__}


class _Dump:
    """JSON writer: sorted keys, floats by repr, Currency/Single tagged by encode()."""

    @staticmethod
    def dumps(obj):
        return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)


# ----------------------------------------------------------------------------- environment

class Env:
    def __init__(self, mme, database=None, modules=None, frm=None, db_extra=None, paste_procs=False, db_only=None):
        self.mme = mme
        shims.FORM_METHODS.clear()
        self.db = shims.Database(database, db_extra, db_only) if database else None
        self.rec = shims.Recorder()
        self.msgbox_answers = []
        self.stubbed = []       # scenario stubs that replace a procedure of a loaded module (ExpandCombo)
        it = self.it = Interpreter()
        for f in (modules or MODULES):
            it.load_file(os.path.join(mme, f))
        if frm:
            self._load_form_procs(mme, frm)
        it.ensure_ready()
        for name in SHIMMED_PROCS + self.stubbed:
            it.procs.pop(name.lower(), None)
        for name in self.stubbed:       # a stub also replaces a Private procedure of a loaded module
            for m in it.modules.values():
                m.proc_map.pop(name.lower(), None)
        self._bind_procs()
        it.intrinsic_hooks["msgbox"] = self._msgbox
        for tn in ("scripting.dictionary", "dictionary"):
            it.new_hooks[tn] = shims.DictionaryShim
        self.lair_env = None
        self.db_open = False        # scenario "db_open": `DB.OpenRecordset(table)` works (item detail's second recordsets)
        self.shared_lairs = {}
        it.intrinsic_hooks["inputbox"] = self._inputbox
        if paste_procs:
            self._bind_paste_procs()
            it.intrinsic_hooks["inputbox"] = self._inputbox_paste
        self.inputbox_answers = []
        self.loaded_forms = []
        self.class_names = {m.name: m for m in it.modules.values() if m.is_class}
        self.cells = []
        for m in it.modules.values():
            if m.is_class:
                continue
            for low, cell in m.cells.items():
                self.cells.append((cell, m.varmap[low], m))

    def _find_proc(self, name):
        """A procedure by name, Private ones of the loaded modules too (event procedures such as cmbEquip_Click)."""
        proc = self.it.find_proc(name)
        if proc is None:
            for m in self.it.modules.values():
                if not m.is_class and name.lower() in m.proc_map:
                    return m.proc_map[name.lower()][0]
        return proc

    def _load_form_procs(self, mme, spec):
        """Scenario key "frm": procedures and variables cut out of a form (see frm.py), loaded as one
        module. The form's controls, named bare in that code, forward to the frmMain shim."""
        # a list of specs cuts procedures from several forms (the calculator windows); each form's
        # controls forward to the same frmMain shim, so a case must use one form's controls only
        for k, one in enumerate(spec if isinstance(spec, list) else [spec]):
            raw = open(os.path.join(mme, one.get("file", "frmMain.frm")), "rb").read()
            if one.get("file", "frmMain.frm") == "frmMain.frm":
                # `frmMain.Proc(args)` from another module reaches the form's own procedure
                for pn in one["procs"]:
                    shims.FORM_METHODS[pn.lower()] = (
                        lambda args, pn=pn: self.it.call_proc(self._find_proc(pn), None, list(args), [None] * len(args)))
            self.it.load_source(frm_mod.build_source(raw, one), name="frmMainProcs%s" % (k or ""), kind="bas")
            for name in frm_mod.control_names(raw):
                if name.lower() not in self.it.externals:
                    self.it.bind(name, shims.ControlProxy(self, name))
        specs = spec if isinstance(spec, list) else [spec]
        self.it.bind("objToolTip", shims.NoopShim())
        self.it.bind_proc("TColor", lambda it_, c=None: c)            # dark-mode colour map: identity
        self.it.bind_proc("RGB", lambda it_, r, g, b: V.mk_i32(V.int_value(r) + 256 * V.int_value(g) + 65536 * V.int_value(b)))
        self.it.bind_proc("ReadINI", lambda it_, *a: "0")             # settings.ini is not read
        for n in [n for one in specs for n in one.get("stub_procs", [])]:   # display-only helpers (tool-tip text)
            self.it.bind_proc(n, lambda it_, *a: "")
        # `stub_replace`: stubs for procedures of a loaded module (ExpandCombo), which the module's own copy would otherwise win
        for n in [n for one in specs for n in one.get("stub_replace", [])]:
            self.it.bind_proc(n, lambda it_, *a: "")
            self.stubbed.append(n)
        for n in [n for one in specs for n in one.get("stub_false", [])]:   # yes/no helpers of the UI (is a window loaded)
            self.it.bind_proc(n, lambda it_, *a: False)

    # -- shims
    def _msgbox(self, it, *a):
        text = V.c_str(a[0]) if a else ""
        buttons = int(a[1]) if len(a) > 1 and V.is_num(a[1]) else 0
        self.rec.add("msgbox", text=text)
        if self.msgbox_answers:       # a case's "msgbox": the answers to the boxes MME shows, in order
            return V.I16(self.msgbox_answers.pop(0))
        return shims.msgbox_result(buttons)

    def _bind_paste_procs(self):
        """Procedures of frmMain that code cut from it calls and that only touch the screen, the disk or the
        game window (family paste_char): recorded or ignored. SaveCharacter reports success."""
        it, rec = self.it, self.rec

        def save(it_, prompt=False, name=V.MISSING):
            rec.add("save", prompt=bool(prompt), file="" if name is V.MISSING else V.c_str(name))
            return V.I16(0)

        def file_exists(it_, name=""):
            n = V.c_str(name).lower()
            return n.endswith(".mmec") and "missing" not in n

        it.bind_proc("SaveCharacter", save)
        it.bind_proc("FileExists", file_exists)
        it.procs.pop("fileexists", None)
        for n in ("RefreshAll", "LoadCharacter", "ClearMonsterDamageVsCharALL", "ClearSavedDamageVsMonster",
                  "LV_Sort_ColumnClickOrSticky", "RefreshListviewItemColors_ItemManager"):
            it.bind_proc(n, lambda it_, *a: None)

    def _inputbox_paste(self, it, *a):
        """InputBox for family paste_char: a case's "inputbox" answers in order, else the default (OK pressed).
        Other families use `_inputbox` (an exhausted list cancels)."""
        text = V.c_str(a[0]) if a else ""
        default = V.c_str(a[2]) if len(a) > 2 and a[2] is not V.MISSING else ""
        self.rec.add("inputbox", text=text, default=default)
        if self.inputbox_answers:
            return self.inputbox_answers.pop(0)
        return default
    def _inputbox(self, it, prompt=None, title=None, default=None, *rest):
        """InputBox: answers from the case's "inputbox" list in call order (null accepts the default,
        an exhausted list cancels, which is an empty string)."""
        dflt = V.c_str(default) if default is not None and default is not V.MISSING else ""
        ans = self.inputbox_answers.pop(0) if self.inputbox_answers else ""
        ans = dflt if ans is None else ans
        self.rec.add("inputbox", prompt=V.c_str(prompt) if prompt is not None else "", default=dflt, answer=ans)
        return ans

    def _bind_procs(self):
        it = self.it
        rec = self.rec

        def noop(it_, *a):
            return None

        def handle_error(it_, *a):
            src = ""
            if a and isinstance(a[0], str):
                src = a[0]
            rec.add("handled_error", source=src, number=it_.err.number, description=it_.err.description)
            it_.err.clear()

        def zero(it_, *a):
            return V.I32(0)

        def false(it_, *a):
            return False

        for n in ("DebugLogPrint", "AppTerminate", "Sleep", "Unload", "SetClipboardText", "RefreshLearnedSpellColors",
                  "LV_AssignRowSeqIfMissing", "Load", "EnsureAppForeground"):
            it.bind_proc(n, noop)
        for n in ("SendMessage", "GetWindowLong", "SetWindowLong", "SetWindowPos"):
            it.bind_proc(n, zero)
        it.bind_proc("HandleError", handle_error)
        it.procs.pop("handleerror", None)

        def sort_listview(it_, lv, column, kind, ascending):
            # modListViewExt is not loaded: ListView sorting is left to the grid, so only the request is recorded
            # (the rows the list held, the column, the data type and the direction).
            rec.add("sort_listview", rows=len(getattr(lv, "rows", [])), column=V.int_value(column),
                    data_type=V.int_value(kind), ascending=bool(ascending))
        it.bind_proc("SortListView", sort_listview)

        def refresh_sort(shops_first):
            # LV_RefreshSort / LV_RefreshSort_ShopsFirst (item detail, P3-35): the same skipped feature; only the request
            # is recorded: rows held, column, data type, by tag, ascending (their optional defaults differ per procedure).
            defaults = (1, 1, True, False) if shops_first else (1, 0, False, True)

            def f(it_, lv, col=V.MISSING, dtype=V.MISSING, by_tag=V.MISSING, asc=V.MISSING):
                got = [d if a is V.MISSING else a for a, d in zip((col, dtype, by_tag, asc), defaults)]
                rec.add("lv_refresh_sort", shops_first=shops_first, rows=len(getattr(lv, "rows", [])),
                        column=V.int_value(got[0]), data_type=V.int_value(got[1]), by_tag=bool(got[2]), ascending=bool(got[3]))
            return f
        # a family that stubs them itself (filters) keeps its stub
        for n, sf in (("LV_RefreshSort", False), ("LV_RefreshSort_ShopsFirst", True)):
            if n.lower() not in it.ext_procs:
                it.bind_proc(n, refresh_sort(sf))
        for n, v in (("lvwcolumnleft", 0), ("lvwcolumnright", 1), ("lvwcolumncenter", 2)):     # ColumnHeaders.Add alignment
            it.bind(n, V.I32(v))
        for n, v in (("ldtstring", 0), ("ldtnumber", 1), ("ldtDateTime", 2)):     # modListViewExt's ListDataType
            it.bind(n, V.I32(v))
        # RegExpFindv2 drives VBScript.RegExp, which the interpreter does not model; the same
        # matching is done here with Python's re (ASCII classes, as in VBScript).
        it.bind_proc("RegExpFindv2", self._regexp_find)
        it.procs.pop("regexpfindv2", None)
        it.bind_proc("EnsureDatabaseConnection", false)
        it.procs.pop("ensuredatabaseconnection", None)

    def _regexp_find(self, it, look_in, pattern, match_case=True, multiline=False, allow_empty=False):
        """modSyntaxsFunc RegExpFindv2: every match with its non-empty submatches (empty ones too when
        allowed). No match gives one element with empty text and a one-element empty submatch array."""
        def val(x, default):
            x = x.v if hasattr(x, "v") and not isinstance(x, (str, int, float)) else x
            return default if x is V.MISSING else x
        look_in, pattern = V.c_str(val(look_in, "")), V.c_str(val(pattern, ""))
        flags = re.ASCII | (0 if val(match_case, True) else re.IGNORECASE) | (re.MULTILINE if val(multiline, False) else 0)
        keep = -1 if val(allow_empty, False) else 0
        found = list(re.finditer(pattern, look_in, flags))
        factory = lambda: it.new_udt("regexmatches")
        arr = VBArray("regexmatches", None, factory)
        arr.dynamic = True
        arr.allocate([(0, max(len(found), 1) - 1)])
        for i, m in enumerate(found or [None]):
            u = arr.get((i,))
            subs = []
            if m is not None:
                u.set_field("sfullmatch", m.group(0))
                subs = [g or "" for g in m.groups() if len(g or "") > keep]
            sm = u.fields["ssubmatches"]
            sm.allocate([(0, max(len(subs), 1) - 1)])
            for j, t in enumerate(subs):
                sm.set((j,), t)
        return arr

    # -- lair shims (scenario key "shims": ["lairs"]): GetLairInfo and GetDamageOutput answer from the
    # case's "lair_env" table, so the statistics are pinned without P3-16's cache or the combat state.
    def enable_lair_shims(self):
        for n in ("GetLairInfo", "GetDamageOutput"):
            self.it.procs.pop(n.lower(), None)
            for m in self.it.modules.values():      # callers in the same module resolve there first
                m.proc_map.pop(n.lower(), None)
        self.it.bind("dbOpenSnapshot", V.I32(4))     # DAO constant used by CalculateAverageLairs
        self.it.bind_proc("GetLairInfo", self._get_lair_info)
        self.it.bind_proc("GetDamageOutput", self._get_damage_output)

    def _get_lair_info(self, it, group_index, max_regen=V.MISSING):
        gi = V.c_str(group_index)
        u = it.new_udt("lairinfotype")
        row = ((self.lair_env or {}).get("lairs") or {}).get(gi)
        if row is None:
            row = self.shared_lairs.get(gi)     # scenario "data": {"lairs": ..}, written once for the family
        if row is None:
            return u
        for k, v in row.items():
            u.set_field(k.lower(), decode(v))
        regen = V.coerce_to(max_regen, "integer")     # the parameter is an Integer: Overflow past 32767
        u.set_field("sgroupindex", gi)
        u.set_field("nmaxregen", regen)
        u.set_field("nrtk", 1.0)
        u.set_field("nrtc", float(regen))
        return u

    def _get_damage_output(self, it, *args):
        def num(x):
            return V.int_value(x)
        self.rec.add("damage_output", args=[num(a) for a in args[1:]])
        u = it.new_udt("tdamageoutput")
        for k, v in ((self.lair_env or {}).get("damage_out") or {}).items():
            u.set_field(k.lower(), decode(v))
        return u

    # -- per-case state
    def reset(self, rnd=None):
        it = self.it
        for cell, decl, m in self.cells:
            cell.v = it.make_var(decl, m).v
        it.err.clear()
        it.depth = 0
        it.rnd.state = 0x50000 if rnd is None else rnd & 0xFFFFFF
        self.rec.events.clear()
        self.saved = {}
        it.bind("frmMain", shims.FormShim({}, decode))
        it.bind("Clipboard", shims.ClipboardShim())
        it.bind("Forms", shims.FormsCollection(self.loaded_forms))
        if self.db and self.db_open and "db" in self.it.globals:
            self.it.global_cell("DB").v = shims.DbShim(self.db)
        for g, table in shims.RECORDSET_GLOBALS.items():
            cell = it.globals.get(g.lower())
            if cell is not None:
                cell.v = self.db.open_recordset(table) if self.db else NOTHING

    def set_forms(self, spec):
        for name, props in (spec or {}).items():
            self.it.bind(name, shims.FormShim(props, decode))

    def set_globals(self, spec):
        for name, val in (spec or {}).items():
            if name.lower() in self.it.globals:
                cell = self.it.global_cell(name)
                if isinstance(val, dict) and "t" not in val:
                    val = {k: decode(x) for k, x in val.items()}
                elif isinstance(val, list):
                    val = [decode(x) for x in val]
                else:
                    val = decode(val)
                self.it.assign_python(cell, val)
            else:
                raise OracleFatal("scenario sets unknown global %s" % name)

    # -- objects and steps
    def make_object(self, spec):
        cls = self.class_names.get(spec["class"]) or self.it.classes.get(spec["class"].lower())
        if cls is None:
            raise OracleFatal("unknown class %s" % spec["class"])
        obj = self.it.instantiate(cls)
        for prop, val in (spec.get("props") or {}).items():
            if isinstance(val, list):
                for i, x in enumerate(val):
                    self.put(obj, prop, [V.I16(i)], decode(x))
            else:
                self.put(obj, prop, [], decode(val))
        return obj

    def put(self, obj, prop, idx, value):
        self.it.compiler.member_set(obj, prop.lower(), idx, value, "", prop, False)

    def get(self, obj, prop, idx):
        return self.it.compiler.member_get(obj, prop.lower(), idx, "obj", prop)

    def make_udt(self, spec):
        """{"udt": "TypeName", "fields": {name: value | [values]}}: a Type instance for a call
        argument; a non-empty list allocates that field's dynamic array (0-based), [] leaves it unallocated."""
        u = self.it.new_udt(spec["udt"].lower())
        for name, val in spec.get("fields", {}).items():
            cur = u.fields[name.lower()]
            if isinstance(cur, VBArray):
                if val and cur.bounds is not None and not getattr(cur, "dynamic", False):
                    # fixed array (`nMAPlusSkill(1 To 3)`): keep its bounds, fill from the lower one
                    lo = cur.bounds[0][0]
                    for i, x in enumerate(val):
                        cur.set((lo + i,), decode(x))
                elif val:
                    cur.allocate([(0, len(val) - 1)])
                    for i, x in enumerate(val):
                        cur.set((i,), decode(x))
            else:
                u.set_field(name.lower(), decode(val))
        return u

    def build_args(self, spec, objs, refs, lv=None, dicts=None):
        """Call arguments from a step's `args`. Beyond the plain values: `{"ref": ...}` (ByRef), `{"udt": ...}`
        (a Type; `out` makes it ByRef and lands in the step's refs), `{"obj": name}` (an object of the case),
        `{"saved": name}` (the ByRef Type a previous step kept under its `as` name, shared so later calls see
        what earlier ones left in it) and `{"lv": true}` (the recording ListView of a `rows` step)."""
        args = []
        for a in spec:
            if isinstance(a, dict) and "saved" in a:
                args.append(self.saved[a["saved"]][a.get("index", 0)])
                continue
            if isinstance(a, dict) and "lv" in a:
                args.append(lv)
                continue
            if isinstance(a, dict) and "key" in a:
                args.append(self.cur_key)        # the record number a `rows` step is on
                continue
            if isinstance(a, dict) and str(a.get("ref", "")).startswith("array:"):
                # ByRef array argument: {"ref": "array:currency|string|<udt>", "bounds": [[lo, hi], ..]};
                # without bounds the array starts unallocated, as a bare `Dim x() As T` does.
                code = a["ref"][6:].lower()
                factory = (lambda c=code: self.it.new_udt(c)) if code in self.it.udts else None
                arr = VBArray(code, None, factory)
                arr.dynamic = True
                if a.get("bounds"):
                    arr.allocate([tuple(b) for b in a["bounds"]])
                elif a.get("values"):
                    # input values: a zero-based array holding these (an empty list leaves it unallocated)
                    arr.allocate([(0, len(a["values"]) - 1)])
                    for i, x in enumerate(a["values"]):
                        arr.set((i,), decode(x))
                cell = Var(arr, a["ref"].lower())
                args.append(cell)
                refs.append(cell)
            elif isinstance(a, dict) and "ref" in a:
                cell = Var(decode(a["value"]), a["ref"])
                args.append(cell)
                refs.append(cell)
            elif isinstance(a, dict) and "udt" in a:
                u = self.make_udt(a)
                if a.get("out"):   # a Type argument the callee fills (ByRef): its final fields land in "<as>.refs"
                    u = Var(u, a["udt"].lower())
                    refs.append(u)
                args.append(u)
            elif isinstance(a, dict) and "dict" in a:
                # a Scripting.Dictionary of Long keys and counts, passed by reference; its final
                # items land in "<as>.dicts"
                d = shims.DictionaryShim()
                for k, v in a["dict"]:
                    d.items[float(k)] = (V.mk_i32(k), V.mk_i32(v))
                if dicts is not None:
                    dicts.append(d)
                args.append(d)
            elif isinstance(a, dict) and "control" in a:
                args.append(self.it.externals["frmmain"].vb_get(a["control"], []))  # a frmMain control, by reference
            elif isinstance(a, dict) and "obj" in a:
                args.append(objs[a["obj"]])  # an object made by this case, passed by reference
            else:
                args.append(decode(a))
        return args

    def invoke(self, objs, target, args):
        if "." in target:
            oname, meth = target.split(".", 1)
            r = self.get(objs[oname], meth, args)
        else:
            self.it.ensure_ready()
            proc = self.it.find_proc(target)
            if proc is None:
                # Private procedures are only in their module's map; cases may still call them.
                for m in self.it.modules.values():
                    if not m.is_class and target.lower() in m.proc_map:
                        proc = m.proc_map[target.lower()][0]
                        break
            if proc is None:
                raise OracleFatal("unknown procedure %s" % target)
            r = self.it.call_proc(proc, None, args, [None] * len(args))
        return r

    def call(self, objs, step, results):
        target = step["call"]
        refs = []
        dicts = []
        args = self.build_args(step.get("args", []), objs, refs, dicts=dicts)
        r = self.invoke(objs, target, args)
        if "as" in step:
            results[step["as"]] = encode(r) if r is not None else None
        if dicts:
            results[step.get("as", target) + ".dicts"] = [[[encode(k), encode(v)] for k, v in d.items.values()]
                                                          for d in dicts]
        if refs:
            results[step.get("as", target) + ".refs"] = [encode(c.v) for c in refs]
            self.saved[step.get("as", target)] = refs
        if step.get("capture"):
            # `capture` {name: read expression}: the reads taken right after this call (the controls a
            # Sub fills), so one case can run many calls on one set-up; lands in "<as>.cap"
            results[step.get("as", target) + ".cap"] = {k: self.read(objs, x) for k, x in step["capture"].items()}

    def select_step(self, st, results):
        """Step `{"select": "cmbEquip", "index": 3, "item": 1234, "as": "name"}`: the user picks the list entry whose
        ItemData is `item` (the first one, as a list search would) in a frmMain ComboBox element, which raises its Click
        event; `as` records whether such an entry existed (item 0 picks the "(none)" entry)."""
        ctl = self.it.externals["frmmain"].vb_get(st["select"], [V.I16(st["index"])])
        data = ctl.items.get("itemdata", {})
        for k in range(len(ctl.items.get("list", {}))):
            if int(data.get(k, 0)) == st["item"]:
                ctl.vb_set("listindex", [], V.I32(k))
                found = True
                break
        else:
            found = False
        if "as" in st:
            results[st["as"]] = found

    def rows_step(self, objs, st, results):
        """Step `{"rows": {"table", "index", "keys", "lv": "each"|"shared", "watch"}, "call", "args", "as", "post"}`:
        for each key, seek that record in the table's global recordset and call a ListView-building procedure
        (`{"lv": true}` in `args` is a recording ListView). `lv` "each" (default) gives every key a fresh ListView
        and stores `null` or the one row it built; "shared" fills one ListView with every key and stores its rows,
        after the `post` calls (`{"call", "args"}` with `{"lv": true}` as the ListView) ran on it, each result and
        the ListView after it going to `<as>.post`. `watch` names a control element (`cmbCharBless(0)`) whose
        list gains entries (AddItem): `<as>.watch` holds `[text, ItemData]` or null per key. Rows are
        `[Text, Tag, [[sub Text, sub Tag, sub ForeColor], ..]]`, plus the row's ForeColor as a fourth element when it was
        assigned; a Tag or ForeColor never assigned is null. `hwnd` gives the ListView an hWnd other than 0, the
        value frmMain's own lists read, for code that asks whether it was handed `lvMonsters`."""
        spec = st["rows"]
        rs = self.it.get_global(spec["table"])
        rs.vb_set("index", [], spec["index"])
        shared = spec.get("lv") == "shared"
        lv_shared = shims.RecordingListViewShim() if shared else None
        if shared and "hwnd" in spec:
            lv_shared.props["hwnd"] = V.I32(spec["hwnd"])
        watch = None
        if spec.get("watch"):
            wname, widx = re.match(r"(\w+)\((\d+)\)$", spec["watch"]).groups()
            watch = self.it.externals["frmmain"].vb_get(wname, [V.I16(int(widx))])
        per, seen, captured = [], [], []
        for key in spec["keys"]:
            rs.vb_get("seek", ["=", decode(key)])
            self.cur_key = decode(key)
            lv = lv_shared or shims.RecordingListViewShim()
            if "hwnd" in spec:
                lv.props["hwnd"] = V.I32(spec["hwnd"])   # a list that is not frmMain's own (its hWnd reads 0)
            before = len(watch.items.get("list", {})) if watch else 0
            args = self.build_args(st.get("args", []), objs, [], lv)
            n_events = len(self.rec.events)
            self.invoke(objs, st["call"], args)
            if spec.get("capture"):
                # `capture`: {"name": "%c.txtDetail.Text"}: read expressions evaluated after each key; with `events` the
                # recorded events of that key move from the case's event list into the capture
                cap = {k: self.read(objs, expr) for k, expr in spec["capture"].items()}
                if spec.get("all_rows"):
                    cap["rows"] = lv.dump(encode)       # every row of this key's ListView (the plain output keeps the first)
                if spec.get("events"):
                    cap["events"] = self.rec.events[n_events:]
                    del self.rec.events[n_events:]
                captured.append(cap)
            if spec.get("lv") == "list":
                per.append(lv.dump_detail(encode))     # every row of the list, with Bold (the detail panes)
            elif not shared and not spec.get("all_rows"):
                rows = lv.dump(encode)
                per.append(rows[0] if rows else None)
            if watch is not None:
                lst, data = watch.items.get("list", {}), watch.items.get("itemdata", {})
                seen.append([lst[before], int(data.get(before, 0))] if len(lst) > before else None)
        name = st["as"]
        post = []
        for p in st.get("post", []):
            pargs = self.build_args(p.get("args", []), objs, [], lv_shared)
            r = self.invoke(objs, p["call"], pargs)
            post.append({"result": encode(r) if r is not None else None, "rows": lv_shared.dump(encode)})
        results[name] = lv_shared.dump(encode) if shared else per
        if post:
            results[name + ".post"] = post
        if watch is not None:
            results[name + ".watch"] = seen
        if spec.get("capture"):
            results[name + ".capture"] = captured

    _READ = re.compile(r"^(?P<obj>[A-Za-z_]\w*)\.(?P<prop>\w+)(?:\[(?P<idx>[^\]]*)\])?$")

    _CTL = re.compile(r"^%c\.(?P<ctl>\w+)(?:\((?P<ci>\d+)\))?\.(?P<prop>\w+)(?:\[(?P<lo>\d+)\.\.(?P<hi>\d+)\])?$")

    def read(self, objs, expr):
        if expr == "%rnd":
            return self.it.rnd.state
        if expr.startswith("%c."):
            # a frmMain control property: "%c.txtCharStats.Tag[0..5]" over a control array, "%c.lblX(3).Caption" for one element
            m = self._CTL.match(expr)
            if not m:
                raise OracleFatal("bad read expression %r" % expr)
            form = self.it.externals["frmmain"]

            def one(args):
                return encode(form.vb_get(m["ctl"], args).vb_get(m["prop"], []))
            if m["lo"] is not None:
                return [one([V.I16(i)]) for i in range(int(m["lo"]), int(m["hi"]) + 1)]
            return one([V.I16(int(m["ci"]))] if m["ci"] is not None else [])
        if expr.startswith("%combos."):
            # a frmMain ComboBox array after the steps: per element {"sel": ListIndex, "data": [ItemData..]}
            arr = self.it.externals["frmmain"].controls[expr[8:].lower()]
            return {str(i): {"sel": int(c.vb_get("listindex", [])),
                             "data": [encode(c.items.get("itemdata", {}).get(k, V.I32(0)))
                                      for k in range(len(c.items.get("list", {})))]}
                    for i, c in sorted(arr.items())}
        if expr.startswith("%f."):
            # a control of another form: "%f.frmMonsterAttackSim.txtUserAC.Text", "%f.frmX.ctl(2).Tag"
            m = re.match(r"^%f\.(\w+)\.(\w+)(?:\((\d+)\))?\.(\w+)$", expr)
            if not m:
                raise OracleFatal("bad read expression %r" % expr)
            form = self.it.externals[m[1].lower()]
            args = [V.I16(int(m[3]))] if m[3] is not None else []
            ctl = form.vb_get(m[2], args)
            if not isinstance(ctl, shims.ExtObject):
                # `form.txtX = v` (default property) landed in the form's variables: it is the control's Text
                return encode(V.c_str(ctl))
            return encode(ctl.vb_get(m[4], []))
        if expr.startswith("%combo."):
            # the elements of a frmMain combo array that hold more than the None entry or select something:
            # {"<index>": {"n": ListCount, "sel": ListIndex, "data": [ItemData..]}}
            ctl = self.it.externals["frmmain"].controls[expr[7:].lower()]
            out = {}
            for i in sorted(ctl):
                c = ctl[i]
                n = int(c.vb_get("listcount", []))
                sel = int(c.vb_get("listindex", []))
                if n > 1 or sel != 0:       # a combo holding only its None entry is left out
                    out[str(i)] = {"n": n, "sel": sel, "data": [int(c.vb_get("itemdata", [V.I16(k)])) for k in range(n)]}
            return out
        if expr == "%clip":
            # what the code under test left on the clipboard ("" when it never copied)
            return self.it.externals["clipboard"].text
        if expr.startswith("%lv."):
            # a frmMain ListView's rows after the steps ran: [{"text", "subs": [{"t", "g"?}]}]
            return self.it.externals["frmmain"].vb_get(expr[4:], []).dump(encode)
        if expr.startswith("%lvn."):
            # a frmMain recording ListView reduced to Text and Tag per row (the record number and the attack kind)
            # as "number" or "number:tag" strings, to keep the fixtures small
            return [r[0] if r[1] is None else "%s:%s" % (r[0], r[1])
                    for r in self.it.externals["frmmain"].vb_get(expr[5:], []).dump(encode)]
        if expr.startswith("%list."):
            # "%list.cmbEquip[0..19]": the ItemData of every entry of each control-array element, in list order
            m = re.match(r"^%list\.(\w+)\[(\d+)\.\.(\d+)\]$", expr)
            form = self.it.externals["frmmain"]
            out = []
            for k in range(int(m[2]), int(m[3]) + 1):
                ctl = form.vb_get(m[1], [V.I16(k)])
                data = ctl.items.get("itemdata", {})
                out.append([int(V.int_value(data.get(j, 0))) for j in sorted(ctl.items.get("list", {}))])
            return out
        if expr.startswith("%g."):
            return encode(self.it.get_global(expr[3:]))
        if expr.startswith("%gt."):
            # a global array without its trailing zeros
            v = encode(self.it.get_global(expr[4:]))
            while v and v[-1] == 0:
                v.pop()
            return v
        m = self._READ.match(expr)
        if not m:
            raise OracleFatal("bad read expression %r" % expr)
        obj, prop, idx = objs[m["obj"]], m["prop"], m["idx"]
        if idx is None:
            return encode(self.get(obj, prop, []))
        if ".." in idx:
            lo, hi = (int(x) for x in idx.split(".."))
            return [encode(self.get(obj, prop, [V.I16(i)])) for i in range(lo, hi + 1)]
        return encode(self.get(obj, prop, [V.I16(int(x)) for x in idx.split(",")]))

    def run_case(self, case, setup):
        self.loaded_forms = list(case.get("loaded_forms", setup.get("loaded_forms", [])))
        self.reset(case.get("rnd"))
        self.msgbox_answers = list(case.get("msgbox", []))
        self.inputbox_answers = [str(x) for x in case.get("inputbox", [])]
        # a case's controls replace the family-wide setup's controls of the same name, form by form
        forms = {}
        for src in (setup.get("forms"), case.get("forms")):
            for fname, props in (src or {}).items():
                cur = forms.setdefault(fname, {})
                for k, v in props.items():
                    if isinstance(v, dict) and "$array+" in v:
                        # a control array element-wise over the family-wide one: elements given here are laid over theirs, property by property
                        base = (cur.get(k) or {}).get("$array", {}) if isinstance(cur.get(k), dict) else {}
                        v = {"$array": {**base, **{i: {**base.get(i, {}), **e} for i, e in v["$array+"].items()}}}
                    cur[k] = v
        self.set_forms(forms)
        self.inputbox_answers = list(case.get("inputbox") or [])
        self.set_globals(setup.get("globals"))
        self.set_globals(case.get("globals"))
        self.lair_env = case.get("lair_env")
        if self.lair_env and "summoned_by" in self.lair_env:
            sb = self.lair_env["summoned_by"]
            if sb == "$loc":               # the monster's text is the procedure's own first argument
                sb = case["args"][0]
            self.it.global_cell("tabMonsters").v = shims.FakeMonsters({"Summoned By": sb})
        if self.lair_env and "monsters" in self.lair_env:
            self.it.global_cell("DB").v = shims.FakeDB(self.lair_env["monsters"])
        objs = {}
        out = {}
        steps = list(case.get("steps", []))
        if "proc" in case:
            steps.append({"call": case["proc"], "args": case.get("args", []), "as": "result"})
        err = None
        try:
            for name, spec in (case.get("objects") or {}).items():
                objs[name] = self.make_object(spec)
                if spec.get("global"):
                    # The instance also stands in for an (unset) object-typed module global.
                    self.it.global_cell(spec["global"]).v = objs[name]
            for st in steps:
                if "rows" in st:
                    self.rows_step(objs, st, out)
                elif "select" in st:
                    self.select_step(st, out)
                elif "set" in st:
                    oname, prop = st["set"].split(".", 1)
                    idx = [decode(i) for i in st.get("index", [])]
                    self.put(objs[oname], prop, idx, decode(st["value"]))
                else:
                    self.call(objs, st, out)
        except VBError as e:
            err = {"number": e.number, "description": e.description}
        except OracleUnbound as e:
            raise OracleFatal("case %s: unbound %s %s" % (case["id"], e.name, e.detail or ""))
        for name, expr in (case["read"] if "read" in case else setup.get("read") or {}).items():
            try:
                out[name] = self.read(objs, expr)
            except VBError as e:
                out[name] = {"error": e.number}
        res = {"outputs": out}
        if err:
            res["error"] = err
        if self.rec.events:
            res["events"] = list(self.rec.events)
        return res


# ----------------------------------------------------------------------------- families

def scenario_dir():
    return os.path.join(ROOT, "fixtures", "scenarios")


def rules_dir():
    return os.path.join(ROOT, "fixtures", "rules")


def families():
    return sorted(os.path.splitext(os.path.basename(p))[0] for p in glob.glob(os.path.join(scenario_dir(), "*.json")))


def build_fixture(family, mme=None, sdir=None, part=None):
    """Return the fixture file's bytes for a family. With part=(k, n) only cases k, k+n, k+2n ... run
    and (header line, [(case index, line)]) comes back for assemble_fixture (cases never depend on one
    another: run_case resets the environment, so a family can be split across processes)."""
    mme = mme or mme_dir()
    path = os.path.join(sdir or scenario_dir(), family + ".json")
    raw = open(path, "rb").read()
    scn = json.loads(raw)
    cases = scn_mod.expand(scn)
    database = db_path() if scn.get("database") else None
    env = Env(mme, database, MODULES + list(scn.get("modules", [])), scn.get("frm"), scn.get("db_extra"),
              paste_procs=bool(scn.get("paste_procs")), db_only=scn.get("db_only"))
    env.db_open = bool(scn.get("db_open"))
    if "lairs" in scn.get("shims", []):
        env.enable_lair_shims()
        env.shared_lairs = (scn.get("data") or {}).get("lairs") or {}
    # "modules": extra MME files a family needs; "frm": procedures cut from a form; "db_extra": invented rows;
    # "paste_procs": stub frmMain's screen/disk procedures and answer InputBox with the default (paste_char)
    setup = {"globals": scn.get("globals", {}), "forms": scn.get("forms", {})}
    if "read" in scn:                       # family-wide default reads; a case's own "read" replaces them
        setup["read"] = scn["read"]
    header = {
        "family": family,
        "description": scn.get("description", ""),
        "mme_sha": mme_sha(mme),
        "generator_version": GENERATOR_VERSION,
        "scenario_sha256": hashlib.sha256(raw).hexdigest(),
        "database_sha256": STOCK_MDB_SHA if database else None,
        "tolerance": scn.get("tolerance", {"default": 0}),
        "setup": setup,
        "notes": scn.get("notes", []),
        "case_count": len(cases),
    }
    if "data" in scn:
        header["data"] = scn["data"]      # tables the Rust test needs once, shared by every case
    head = '{"header":' + _Dump.dumps(header) + ','
    out = []
    for i, c in enumerate(cases):
        if part is not None and i % part[1] != part[0]:
            continue
        res = env.run_case(c, setup)
        entry = {"id": c["id"], "inputs": {k: v for k, v in c.items() if k != "id"}}
        entry.update(res)
        out.append((i, _Dump.dumps(entry)))
    if part is not None:
        return head, out
    return assemble_fixture(head, out)


def assemble_fixture(head, indexed_lines):
    """The fixture bytes from the header line and the (case index, JSON line) pairs of every case."""
    ordered = [line for _, line in sorted(indexed_lines)]
    lines = [head, '"cases":[']
    for i, line in enumerate(ordered):
        lines.append(line + ("," if i + 1 < len(ordered) else ""))
    lines.append("]}")
    return ("\n".join(lines) + "\n").encode("ascii")

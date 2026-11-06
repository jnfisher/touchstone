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
    def __init__(self, mme, database=None, modules=None, frm=None, db_extra=None):
        self.mme = mme
        shims.FORM_METHODS.clear()
        self.db = shims.Database(database, db_extra) if database else None
        self.rec = shims.Recorder()
        self.msgbox_answers = []
        it = self.it = Interpreter()
        for f in (modules or MODULES):
            it.load_file(os.path.join(mme, f))
        if frm:
            self._load_form_procs(mme, frm)
        it.ensure_ready()
        for name in SHIMMED_PROCS:
            it.procs.pop(name.lower(), None)
        self._bind_procs()
        it.intrinsic_hooks["msgbox"] = self._msgbox
        self.class_names = {m.name: m for m in it.modules.values() if m.is_class}
        self.cells = []
        for m in it.modules.values():
            if m.is_class:
                continue
            for low, cell in m.cells.items():
                self.cells.append((cell, m.varmap[low], m))

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
                        lambda args, pn=pn: self.it.call_proc(self.it.find_proc(pn), None, list(args), [None] * len(args)))
            self.it.load_source(frm_mod.build_source(raw, one), name="frmMainProcs%s" % (k or ""), kind="bas")
            for name in frm_mod.control_names(raw):
                if name.lower() not in self.it.externals:
                    self.it.bind(name, shims.ControlProxy(self, name))
        specs = spec if isinstance(spec, list) else [spec]
        self.it.bind("objToolTip", shims.NoopShim())
        self.it.bind_proc("TColor", lambda it_, c=None: c)            # dark-mode colour map: identity
        self.it.bind_proc("ReadINI", lambda it_, *a: "0")             # settings.ini is not read
        for n in [n for one in specs for n in one.get("stub_procs", [])]:   # display-only helpers (tool-tip text)
            self.it.bind_proc(n, lambda it_, *a: "")

    # -- shims
    def _msgbox(self, it, *a):
        text = V.c_str(a[0]) if a else ""
        buttons = int(a[1]) if len(a) > 1 and V.is_num(a[1]) else 0
        self.rec.add("msgbox", text=text)
        if self.msgbox_answers:       # a case's "msgbox": the answers to the boxes MME shows, in order
            return V.I16(self.msgbox_answers.pop(0))
        return shims.msgbox_result(buttons)

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
                  "LV_AssignRowSeqIfMissing"):
            it.bind_proc(n, noop)
        for n in ("SendMessage", "GetWindowLong", "SetWindowLong", "SetWindowPos"):
            it.bind_proc(n, zero)
        it.bind_proc("HandleError", handle_error)
        it.procs.pop("handleerror", None)
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

    # -- per-case state
    def reset(self, rnd=None):
        it = self.it
        for cell, decl, m in self.cells:
            cell.v = it.make_var(decl, m).v
        it.err.clear()
        it.depth = 0
        it.rnd.state = 0x50000 if rnd is None else rnd & 0xFFFFFF
        self.rec.events.clear()
        it.bind("frmMain", shims.FormShim({}, decode))
        it.bind("Clipboard", shims.ClipboardShim())
        it.bind("Forms", shims.FormsCollection())
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

    def call(self, objs, step, results):
        target = step["call"]
        args = []
        refs = []
        for a in step.get("args", []):
            if isinstance(a, dict) and str(a.get("ref", "")).startswith("array:"):
                # ByRef array argument: {"ref": "array:currency|string|<udt>", "bounds": [[lo, hi], ..]};
                # without bounds the array starts unallocated, as a bare `Dim x() As T` does.
                code = a["ref"][6:].lower()
                factory = (lambda c=code: self.it.new_udt(c)) if code in self.it.udts else None
                arr = VBArray(code, None, factory)
                arr.dynamic = True
                if a.get("bounds"):
                    arr.allocate([tuple(b) for b in a["bounds"]])
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
            elif isinstance(a, dict) and "control" in a:
                args.append(self.it.externals["frmmain"].vb_get(a["control"], []))  # a frmMain control, by reference
            elif isinstance(a, dict) and "obj" in a:
                args.append(objs[a["obj"]])  # an object made by this case, passed by reference
            else:
                args.append(decode(a))
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
        if "as" in step:
            results[step["as"]] = encode(r) if r is not None else None
        if refs:
            results[step.get("as", target) + ".refs"] = [encode(c.v) for c in refs]

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
        if expr.startswith("%lv."):
            # a frmMain ListView's rows after the steps ran: [{"text", "subs": [{"t", "g"?}]}]
            return self.it.externals["frmmain"].vb_get(expr[4:], []).dump(encode)
        if expr.startswith("%g."):
            return encode(self.it.get_global(expr[3:]))
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
        self.reset(case.get("rnd"))
        self.msgbox_answers = list(case.get("msgbox", []))
        # a case's controls replace the family-wide setup's controls of the same name, form by form
        forms = {}
        for src in (setup.get("forms"), case.get("forms")):
            for fname, props in (src or {}).items():
                forms.setdefault(fname, {}).update(props)
        self.set_forms(forms)
        self.set_globals(setup.get("globals"))
        self.set_globals(case.get("globals"))
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
                if "set" in st:
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


def build_fixture(family, mme=None, sdir=None):
    """Return the fixture file's bytes for a family."""
    mme = mme or mme_dir()
    path = os.path.join(sdir or scenario_dir(), family + ".json")
    raw = open(path, "rb").read()
    scn = json.loads(raw)
    cases = scn_mod.expand(scn)
    database = db_path() if scn.get("database") else None
    env = Env(mme, database, MODULES + list(scn.get("modules", [])), scn.get("frm"), scn.get("db_extra"))
    # "modules": extra MME files a family needs; "frm": procedures cut from a form; "db_extra": invented rows
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
    lines = ['{"header":' + _Dump.dumps(header) + ',', '"cases":[']
    for i, c in enumerate(cases):
        res = env.run_case(c, setup)
        entry = {"id": c["id"], "inputs": {k: v for k, v in c.items() if k != "id"}}
        entry.update(res)
        lines.append(_Dump.dumps(entry) + ("," if i + 1 < len(cases) else ""))
    lines.append("]}")
    return ("\n".join(lines) + "\n").encode("ascii")

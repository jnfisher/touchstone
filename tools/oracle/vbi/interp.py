"""Interpreter: module loading, name resolution, calls, and execution of compiled procedures.

Procedures are compiled (compiler.py) into flat instruction lists so GoTo, On Error and Resume
work exactly like VB6's statement-level semantics.
"""
from .ast import TypeSpec
from .errors import VBError, OracleUnbound, EndProgram
from . import values as V
from .values import EMPTY, NULL, NOTHING, MISSING
from .objects import Var, ElemRef, FieldRef, VBArray, UDTDef, UDTInstance, ClsInstance, ExtObject
from .builtins import BUILTINS, CONSTANTS
from .parser import parse_module, parse_file, decode_source

OMIT = object()
EXIT = -1
REFTYPES = (Var, ElemRef, FieldRef)


class Frame:
    __slots__ = ("v", "this", "mod", "proc", "h", "on_error", "handling", "err_pc", "pc", "cp")

    def __init__(self, mod, proc, this, cp):
        self.v = {}
        self.this = this
        self.mod = mod
        self.proc = proc
        self.h = {}
        self.on_error = None      # None | 'next' | int (pc of handler)
        self.handling = False
        self.err_pc = 0
        self.pc = 0
        self.cp = cp


class LoadedModule:
    def __init__(self, ast, interp, load_procs=True):
        self.ast = ast
        self.name = ast.name
        self.low = ast.low
        self.kind = ast.kind
        self.interp = interp
        self.is_class = ast.kind == "cls"
        self.load_procs = load_procs
        self.varmap = {}          # low -> VarDecl (non-const)
        self.constmap = {}        # low -> ConstEntry
        self.cells = {}           # std module: low -> Var
        self.proc_map = ast.proc_map if load_procs else {}
        self.initialized = False
        self.option_base = ast.option_base

    def __repr__(self):
        return "<module %s>" % self.name


class ConstEntry:
    def __init__(self, decl, module, expr=None, tc=None):
        self.decl = decl
        self.module = module
        self.expr = expr
        self.tc = tc
        self.state = 0
        self.value = None
        self.is_enum = False

    def get(self, interp):
        if self.state == 2:
            return self.value
        if self.state == 1:
            raise VBError(5, "circular constant " + self.decl.name)
        self.state = 1
        try:
            if isinstance(self.expr, _AutoEnum):
                prev = self.expr.prev
                v = V.I32(0) if prev is None else V.mk_i32(int(prev.get(interp)) + 1)
            else:
                v = interp.compiler.const_expr(self.expr, self.module)(None)
                if self.tc is not None:
                    v = V.coerce_to(v, self.tc)
                elif self.is_enum:
                    v = V.c_long(v)
        except BaseException:
            self.state = 0
            raise
        self.value = v
        self.state = 2
        return v


class EnumType:
    def __init__(self, name):
        self.name = name
        self.members = {}


class ModuleRef:
    """`modName.member` qualification."""

    def __init__(self, mod):
        self.mod = mod


class ErrObject(ExtObject):
    def __init__(self):
        self.number = 0
        self.description = ""
        self.source = ""

    def clear(self):
        self.number = 0
        self.description = ""
        self.source = ""

    def vb_get(self, name, args):
        n = name.lower()
        if n == "number":
            return V.I32(self.number)
        if n == "description":
            return self.description
        if n == "source":
            return self.source
        if n == "clear":
            self.clear()
            return None
        if n == "raise":
            num = V.int_value(args[0])
            desc = V.c_str(args[2]) if len(args) > 2 and args[2] is not MISSING else None
            raise VBError(num, desc)
        raise VBError(438)

    def vb_set(self, name, args, value):
        n = name.lower()
        if n == "number":
            self.number = V.int_value(value)
        elif n == "description":
            self.description = V.c_str(value)
        elif n == "source":
            self.source = V.c_str(value)
        else:
            raise VBError(438)


class Interpreter:
    def __init__(self):
        from .compiler import Compiler
        self.modules = {}         # low -> LoadedModule
        self.classes = {}         # low -> LoadedModule
        self.globals = {}         # low -> Var (public vars of standard modules)
        self.gconsts = {}         # low -> ConstEntry (public constants, enum members)
        self.enums = {}           # low -> EnumType
        self.udts = {}            # low -> UDTDef
        self.procs = {}           # low -> [Proc]  (public procs of standard modules)
        self.externals = {}       # low -> value / ExtObject bound by a harness
        self.ext_procs = {}       # low -> python callable(interp, args)
        self.new_hooks = {}       # low typename -> factory() for `New X` of external classes
        self.rnd = V.Rnd()
        self.randomize_hook = None
        self.err = ErrObject()
        self.compiler = Compiler(self)
        self.depth = 0
        self.max_depth = 400
        self.call_log = None
        self._pending = []

    # ------------------------------------------------------------ loading
    def load_source(self, text, name=None, kind="bas", load_procs=True):
        return self.load_ast(parse_module(text, name, kind), load_procs)

    def load_file(self, path, load_procs=True):
        return self.load_ast(parse_file(path), load_procs)

    def load_ast(self, ast, load_procs=True):
        m = LoadedModule(ast, self, load_procs)
        ast.loaded = m
        self.modules[m.low] = m
        if m.is_class:
            self.classes[m.low] = m
        for low, (name, fields) in ast.types.items():
            d = UDTDef(name)
            d.fields = fields
            self.udts[low] = d
        for low, (name, members) in ast.enums.items():
            et = EnumType(name)
            self.enums[low] = et
            prev = None
            for mname, expr in members:
                ml = mname.lower()
                if expr is None:
                    ce = ConstEntry(_FakeDecl(mname), m, _AutoEnum(prev), None)
                else:
                    ce = ConstEntry(_FakeDecl(mname), m, expr, None)
                ce.is_enum = True
                et.members[ml] = ce
                self.gconsts[ml] = ce
                m.constmap[ml] = ce
                prev = ce
        for d in ast.vars:
            if d.is_const:
                tc = self.type_code(d.typespec, m) if d.typespec else None
                ce = ConstEntry(d, m, d.const_expr, tc)
                m.constmap[d.low] = ce
                if d.scope == "public" and not m.is_class:
                    self.gconsts[d.low] = ce
            else:
                m.varmap[d.low] = d
        if not m.is_class:
            for d in ast.vars:
                if not d.is_const:
                    self.pending_var(m, d)
        if load_procs and not m.is_class:
            for p in ast.procs:
                if p.scope != "private":
                    self.procs.setdefault(p.low, []).append(p)
        return m

    def pending_var(self, m, d):
        self._pending.append((m, d))

    def ensure_ready(self):
        while self._pending:
            m, d = self._pending.pop(0)
            cell = self.make_var(d, m)
            m.cells[d.low] = cell
            if d.scope == "public":
                self.globals[d.low] = cell

    # ------------------------------------------------------------ types
    def type_code(self, ts, module=None):
        if ts is None:
            return "variant"
        low = ts.low
        if low in ("integer", "long", "single", "double", "currency", "string", "boolean", "byte", "variant",
                   "date", "object"):
            if low == "string" and ts.strlen:
                return self.fixed_string(ts.strlen)
            return low
        if low in self.enums:
            return "long"
        return low

    @staticmethod
    def fixed_string(n):
        if isinstance(n, str):
            return "string"
        code = "string*%d" % n
        if code not in V.COERCE:
            V.COERCE[code] = lambda v, n=n: V.c_str(v)[:n].ljust(n)
        return code

    def default_value(self, code):
        if code.startswith("string*"):
            return " " * int(code[7:])
        if code in V.COERCE or code == "variant":
            return V.default_for(code)
        if code in self.udts:
            return self.new_udt(code)
        return NOTHING

    def new_udt(self, low):
        d = self.udts[low]
        fields = {}
        d.field_types = d.field_types or {}
        for f in d.fields:
            code = self.type_code(f.typespec)
            d.field_types[f.low] = code
            if f.dims is not None:
                fields[f.low] = self.make_array(f.dims, code, d.module if hasattr(d, "module") else None)
            else:
                fields[f.low] = self.default_value(code)
        return UDTInstance(d, fields)

    def make_array(self, dims, code, module):
        factory = None
        if code in self.udts:
            factory = lambda c=code: self.new_udt(c)
        arr = VBArray(code, None, factory)
        if dims:
            arr.allocate(self.eval_bounds(dims, module))
        else:
            arr.dynamic = True
        return arr

    def eval_bounds(self, dims, module):
        base = module.option_base if module is not None else 0
        out = []
        for lo, hi in dims:
            lov = base if lo is None else V.int_value(self.compiler.const_expr(lo, module)(None))
            hiv = V.int_value(self.compiler.const_expr(hi, module)(None))
            out.append((lov, hiv))
        return out

    def make_var(self, d, module):
        code = self.type_code(d.typespec, module)
        if d.dims is not None:
            return Var(self.make_array(d.dims, code, module), "array:" + code)
        if d.typespec is not None and d.typespec.new and code in self.classes:
            return Var(self.instantiate(self.classes[code]), code)
        return Var(self.default_value(code), code)

    def instantiate(self, cls):
        self.ensure_ready()
        inst = ClsInstance(cls)
        for low, d in cls.varmap.items():
            inst.vars[low] = self.make_var(d, cls)
        init = cls.proc_map.get("class_initialize")
        if init:
            self.call_proc(init[0], inst, [], [])
        return inst

    def new_object(self, typename):
        low = typename.lower()
        if low in self.classes:
            return self.instantiate(self.classes[low])
        hook = self.new_hooks.get(low)
        if hook:
            return hook()
        raise OracleUnbound("New " + typename)

    # ------------------------------------------------------------ binding API for shims (T-022)
    def bind(self, name, value):
        """Bind an external object or value (form, recordset, ...) under a global name."""
        self.externals[name.lower()] = value

    def bind_proc(self, name, fn):
        """Bind a Python callable(interp, *args) for a procedure of a module that is not loaded."""
        self.ext_procs[name.lower()] = fn

    def set_global(self, name, value):
        self.ensure_ready()
        cell = self.globals.get(name.lower())
        if cell is None:
            raise KeyError(name)
        cell.set(value)

    def get_global(self, name):
        self.ensure_ready()
        return self.globals[name.lower()].v

    def global_cell(self, name):
        self.ensure_ready()
        return self.globals[name.lower()]

    # ------------------------------------------------------------ calling
    def find_proc(self, name, module=None):
        low = name.lower()
        if module is not None and low in module.proc_map:
            return module.proc_map[low][0]
        ps = self.procs.get(low)
        return ps[0] if ps else None

    def call(self, name, *args, this=None, module=None):
        """Call a procedure by name with Python-level arguments (converted to VB values)."""
        self.ensure_ready()
        proc = self.find_proc(name, module and self.modules.get(module.lower()))
        if proc is None:
            raise KeyError(name)
        vals = [self.from_python(a) for a in args]
        return self.call_proc(proc, this, vals, [None] * len(vals))

    @staticmethod
    def from_python(a):
        if isinstance(a, bool):
            return a
        if isinstance(a, int):
            if -32768 <= a <= 32767:
                return V.I16(a)
            return V.mk_i32(a)
        if isinstance(a, float) or isinstance(a, str):
            return a
        return a

    def call_proc(self, proc, this, vals, names):
        cp = self.compiler.compiled(proc)
        mod = proc.module.loaded
        fr = Frame(mod, proc, this, cp)
        params = proc.params
        np_ = len(params)
        slots = [OMIT] * np_
        pos = 0
        extra = None
        for k, v in enumerate(vals):
            nm = names[k]
            if nm is None:
                if pos >= np_:
                    if np_ and params[-1].paramarray:
                        extra = (extra or []) + [v]
                        continue
                    raise VBError(450)
                idx = pos
                pos += 1
            else:
                idx = cp.param_index.get(nm)
                if idx is None:
                    raise VBError(448)
            slots[idx] = v
        fv = fr.v
        for i, p in enumerate(params):
            v = slots[i]
            code = cp.param_codes[i]
            if p.paramarray:
                a = VBArray("variant")
                items = [x.get() if isinstance(x, REFTYPES) else x for x in (extra or [])]
                a.allocate([(0, len(items) - 1)])
                a.data = items
                fv[p.low] = Var(a, "array:variant")
                continue
            if v is OMIT or v is MISSING:
                if p.default is not None:
                    dv = self.compiler.const_expr(p.default, mod)(None)
                    fv[p.low] = Var(V.coerce_to(dv, code) if code in V.COERCE else dv, code)
                elif p.optional:
                    if code == "variant":
                        fv[p.low] = Var(MISSING, code)
                    elif p.is_array:
                        fv[p.low] = Var(VBArray(code), "array:" + code)
                    else:
                        fv[p.low] = Var(self.default_value(code), code)
                else:
                    raise VBError(449)
                continue
            if p.is_array:
                arr = v.get() if isinstance(v, REFTYPES) else v
                if not isinstance(arr, VBArray):
                    raise VBError(13)
                fv[p.low] = Var(arr, "array:" + code)
                continue
            if isinstance(v, REFTYPES):
                if p.byref and getattr(v, "t", None) == code:
                    fv[p.low] = v if isinstance(v, Var) else _RefCell(v, code)
                    continue
                v = v.get()
            if code in V.COERCE:
                v = V.coerce_to(v, code)
            elif isinstance(v, UDTInstance):
                v = v.copy()
            fv[p.low] = Var(v, code)
        # locals
        for low, code, factory in cp.locals:
            fv[low] = factory(self)
        for low, cell in cp.statics_for(this).items():
            fv[low] = cell
        if proc.kind in ("function", "get"):
            rc = cp.ret_code
            fv[proc.low] = Var(cp.ret_default(self), rc)
        self.depth += 1
        if self.depth > self.max_depth:
            self.depth -= 1
            raise VBError(28, "Out of stack space")
        try:
            self.run(cp, fr)
        finally:
            self.depth -= 1
        if proc.kind in ("function", "get"):
            return fv[proc.low].v
        return None

    def run(self, cp, fr):
        instrs = cp.instrs
        n = len(instrs)
        pc = 0
        while True:
            try:
                while pc < n:
                    fr.pc = pc
                    r = instrs[pc](fr)
                    if r is None:
                        pc += 1
                    elif r < 0:
                        return
                    else:
                        pc = r
                return
            except VBError as e:
                mode = fr.on_error
                if mode is None or fr.handling:
                    raise
                self.err.number = e.number
                self.err.description = e.description
                if mode == "next":
                    pc = fr.pc + 1
                    continue
                fr.err_pc = fr.pc
                fr.handling = True
                pc = mode

    # ------------------------------------------------------------ runtime helpers used by compiled code
    def unbound(self, name, detail=None):
        raise OracleUnbound(name, detail)


class _RefCell:
    """Adapter so a non-Var reference (array element / field) can sit in a frame as a variable."""
    __slots__ = ("ref", "t")

    def __init__(self, ref, t):
        self.ref = ref
        self.t = t

    @property
    def v(self):
        return self.ref.get()

    @v.setter
    def v(self, value):
        self.ref.set(value)

    def get(self):
        return self.ref.get()

    def set(self, value):
        self.ref.set(value)


class _FakeDecl:
    def __init__(self, name):
        self.name = name
        self.low = name.lower()


class _AutoEnum:
    """Enum member without an initialiser: previous member + 1 (or 0)."""

    def __init__(self, prev):
        self.prev = prev

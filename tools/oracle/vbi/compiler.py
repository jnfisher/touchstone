"""Compiles procedure ASTs into flat instruction lists and expressions into Python closures."""
from .ast import Node
from .errors import VBError, OracleUnbound, EndProgram, VBSyntaxError
from . import values as V
from .values import EMPTY, NULL, NOTHING, MISSING, I16, I32
from .objects import Var, ElemRef, FieldRef, VBArray, UDTInstance, ClsInstance, ExtObject
from .builtins import BUILTINS, CONSTANTS, f_like
from .interp import (OMIT, EXIT, REFTYPES, ConstEntry, EnumType, ModuleRef, _RefCell)

BIN_ARITH = {"+": V.add, "-": V.sub, "*": V.mul, "/": V.div, "\\": V.int_div, "mod": V.mod, "^": V.power,
             "&": V.concat}
LOGIC = ("and", "or", "xor", "eqv", "imp")


class Label:
    __slots__ = ("pc", "name")

    def __init__(self, name=None):
        self.pc = None
        self.name = name


class CompiledProc:
    def __init__(self, proc):
        self.proc = proc
        self.instrs = []
        self.param_index = {}
        self.param_codes = []
        self.locals = []          # (low, code, factory)
        self.static_cells = {}    # low -> Var (shared across calls)
        self.static_decls = []
        self.ret_code = "variant"
        self.labels = {}

    def statics_for(self, this):
        return self.static_cells

    def ret_default(self, it):
        return it.default_value(self.ret_code)


class Ctx:
    def __init__(self, module, proc=None):
        self.module = module
        self.proc = proc
        self.locals = {}          # low -> code
        self.local_consts = {}    # low -> ConstEntry
        self.cp = None
        self.loops = []           # [(kind, exit_label)]
        self.labels = {}
        self.cell_names = set()


def compile_error(msg, node):
    raise VBSyntaxError(msg, getattr(node, "line", None))


class Compiler:
    def __init__(self, it):
        self.it = it
        self.sel_counter = 0
        self._const_cache = {}

    # ------------------------------------------------------------ entry points
    def const_expr(self, node, module):
        key = (id(node), id(module))
        fn = self._const_cache.get(key)
        if fn is None:
            fn = self._const_cache[key] = self.cx(node, Ctx(module, None))
        return fn

    def compiled(self, proc):
        if proc.compiled is not None:
            return proc.compiled
        self.it.ensure_ready()
        module = proc.module.loaded
        ctx = Ctx(module, proc)
        cp = CompiledProc(proc)
        ctx.cp = cp
        it = self.it
        for i, p in enumerate(proc.params):
            cp.param_index[p.low] = i
            code = it.type_code(p.typespec, module)
            cp.param_codes.append(code)
            ctx.locals[p.low] = code
        if proc.kind in ("function", "get"):
            cp.ret_code = it.type_code(proc.rettype, module) if proc.rettype else "variant"
            if proc.ret_is_array:
                cp.ret_code = "variant"
            ctx.locals[proc.low] = cp.ret_code
        self.collect_decls(proc.body, ctx, cp, module)
        self.stmts(proc.body, ctx)
        for name, lab in ctx.labels.items():
            if lab.pc is None:
                raise VBSyntaxError("label not defined: " + name, proc.line, module.name)
        cp.labels = {k: v.pc for k, v in ctx.labels.items()}
        proc.compiled = cp
        return cp

    def collect_decls(self, body, ctx, cp, module):
        it = self.it
        for st in body:
            k = st.kind
            if k == "dim":
                for d in st.decls:
                    if d.is_const:
                        tc = it.type_code(d.typespec, module) if d.typespec else None
                        ctx.local_consts[d.low] = ConstEntry(d, module, d.const_expr, tc)
                        continue
                    code = it.type_code(d.typespec, module)
                    ctx.locals[d.low] = ("array:" + code) if d.dims is not None else code
                    if d.static:
                        cp.static_cells[d.low] = it.make_var(d, module)
                    elif d.dims is None and (code in V.COERCE or code == "variant") and not code.startswith("string*"):
                        cp.locals.append((d.low, code, lambda it_, v=V.default_for(code), c=code: Var(v, c)))
                    else:
                        cp.locals.append((d.low, code, lambda it_, d=d, m=module: it_.make_var(d, m)))
            elif k == "if":
                for _, b in st.clauses:
                    self.collect_decls(b, ctx, cp, module)
                if st.els:
                    self.collect_decls(st.els, ctx, cp, module)
            elif k == "select":
                for _, b in st.cases:
                    self.collect_decls(b, ctx, cp, module)
                if st.els:
                    self.collect_decls(st.els, ctx, cp, module)
            elif k in ("for", "foreach", "do", "with"):
                self.collect_decls(st.body, ctx, cp, module)

    # ------------------------------------------------------------ name resolution
    def resolve(self, low, ctx):
        it = self.it
        if low in ctx.locals:
            return ("local", low)
        if low in ctx.local_consts:
            return ("const", ctx.local_consts[low])
        mod = ctx.module
        if low in mod.varmap:
            if mod.is_class:
                return ("clsvar", low)
            return ("global", mod.cells[low])
        if low in mod.constmap:
            return ("const", mod.constmap[low])
        if low in it.globals:
            return ("global", it.globals[low])
        if low in it.gconsts:
            return ("const", it.gconsts[low])
        if low in mod.proc_map:
            return ("proc", mod.proc_map[low], mod)
        if low in it.procs:
            return ("proc", it.procs[low], None)
        if low in BUILTINS:
            return ("builtin", BUILTINS[low])
        if low in CONSTANTS:
            return ("bconst", CONSTANTS[low])
        if low == "nothing":
            return ("bconst", NOTHING)
        if low == "empty":
            return ("bconst", EMPTY)
        if low == "null":
            return ("bconst", NULL)
        if low == "err":
            return ("err",)
        if low in it.enums:
            return ("enum", it.enums[low])
        if low in it.modules and low != mod.low:
            return ("module", it.modules[low])
        return ("ext", low)

    # ------------------------------------------------------------ expressions
    def cx(self, node, ctx):
        m = getattr(self, "x_" + node.kind, None)
        if m is None:
            compile_error("cannot compile expression " + node.kind, node)
        return m(node, ctx)

    def x_lit(self, node, ctx):
        v = node.value
        return lambda fr: v

    def x_paren(self, node, ctx):
        return self.cx(node.e, ctx)

    def x_me(self, node, ctx):
        def f(fr):
            if fr.this is None:
                raise VBError(91)
            return fr.this
        return f

    def x_un(self, node, ctx):
        e = self.cx(node.e, ctx)
        if node.op == "neg":
            return lambda fr: V.neg(e(fr))
        return lambda fr: V.not_(e(fr))

    def x_bin(self, node, ctx):
        op = node.op
        a = self.cx(node.l, ctx)
        b = self.cx(node.r, ctx)
        if op in BIN_ARITH:
            fn = BIN_ARITH[op]
            return lambda fr: fn(a(fr), b(fr))
        if op in LOGIC:
            return lambda fr: V.logic(op, a(fr), b(fr))
        if op == "=":
            def eq(fr):
                x, y = a(fr), b(fr)
                if type(x) is str and type(y) is str:
                    return x == y
                return V.compare(x, y) == 0
            return eq
        if op == "<>":
            def ne(fr):
                x, y = a(fr), b(fr)
                if type(x) is str and type(y) is str:
                    return x != y
                return V.compare(x, y) != 0
            return ne
        if op == "<":
            return lambda fr: V.compare(a(fr), b(fr)) < 0
        if op == ">":
            return lambda fr: V.compare(a(fr), b(fr)) > 0
        if op == "<=":
            return lambda fr: V.compare(a(fr), b(fr)) <= 0
        if op == ">=":
            return lambda fr: V.compare(a(fr), b(fr)) >= 0
        if op == "like":
            return lambda fr: f_like(a(fr), b(fr))
        if op == "is":
            return lambda fr: a(fr) is b(fr)
        compile_error("unknown operator " + op, node)

    def x_isnothing(self, node, ctx):
        e = self.cx(node.e, ctx)
        return lambda fr: e(fr) is NOTHING

    def x_typeof(self, node, ctx):
        e = self.cx(node.e, ctx)
        tl = node.typename.lower()

        def f(fr):
            v = e(fr)
            if isinstance(v, ClsInstance):
                return v.cls.low == tl
            if isinstance(v, ExtObject):
                return getattr(v, "vb_typename", "").lower() == tl
            if isinstance(v, UDTInstance):
                return v.defn.name.lower() == tl
            return False
        return f

    def x_new(self, node, ctx):
        it = self.it
        name = node.typename
        return lambda fr: it.new_object(name)

    # --- names
    def x_name(self, node, ctx):
        return self.name_value(node, ctx)

    def name_value(self, node, ctx):
        it = self.it
        low = node.low
        b = self.resolve(low, ctx)
        k = b[0]
        if k == "local":
            return lambda fr: fr.v[low].v
        if k == "clsvar":
            def f(fr):
                return fr.this.vars[low].v
            return f
        if k == "global":
            cell = b[1]
            return lambda fr: cell.v
        if k == "const":
            ce = b[1]
            def fc(fr):
                return ce.value if ce.state == 2 else ce.get(it)
            return fc
        if k == "bconst":
            v = b[1]
            return lambda fr: v
        if k == "proc":
            return self.call_proc_node(b, [], [], ctx)
        if k == "builtin":
            fn = b[1]
            return lambda fr: fn(it)
        if k == "err":
            return lambda fr: it.err
        if k == "enum":
            et = b[1]
            return lambda fr: et
        if k == "module":
            mr = ModuleRef(b[1])
            return lambda fr: mr
        name = node.name

        def fe(fr):
            if low in it.externals:
                return it.externals[low]
            if low in it.ext_procs:
                return it.ext_procs[low](it)
            raise OracleUnbound(name)
        return fe

    def call_proc_node(self, b, argfns, names, ctx):
        it = self.it
        procs, mod = b[1], b[2]
        proc = procs[0]
        same_class = mod is not None and mod.is_class
        if len(procs) > 1:
            # property variants: pick the getter for reads
            getters = [p for p in procs if p.kind in ("get", "function", "sub")]
            proc = getters[0] if getters else procs[0]

        def f(fr):
            vals = [a(fr) for a in argfns]
            return it.call_proc(proc, fr.this if same_class else None, vals, names)
        return f

    def carg(self, node, ctx):
        """Compile a call argument: returns fn(fr) -> value, a reference (for ByRef), or OMIT."""
        if node is None:
            return lambda fr: OMIT
        k = node.kind
        if k == "name":
            b = self.resolve(node.low, ctx)
            low = node.low
            if b[0] == "local":
                return lambda fr: fr.v[low]
            if b[0] == "clsvar":
                return lambda fr: fr.this.vars[low]
            if b[0] == "global":
                cell = b[1]
                return lambda fr: cell
            return self.cx(node, ctx)
        if k == "call":
            t = node.target
            if t.kind == "name":
                b = self.resolve(t.low, ctx)
                if b[0] in ("local", "clsvar", "global") and node.args:
                    base = self.cx(t, ctx)
                    args = [self.cx(a, ctx) for _, a in node.args]

                    def fe(fr):
                        arr = base(fr)
                        if isinstance(arr, VBArray):
                            return ElemRef(arr, tuple(V.int_value(a(fr)) for a in args))
                        return self.index_value(arr, [a(fr) for a in args], t.name)
                    return fe
            return self.cx(node, ctx)
        if k == "member":
            if node.obj is None:
                objfn = self.with_obj(node.withid)
            else:
                objfn = self.cx(node.obj, ctx)
            low = node.low
            desc = self.obj_desc(node.obj, ctx) if node.obj is not None else "." + node.name
            full = desc + "." + node.name

            def fm(fr):
                o = objfn(fr)
                if isinstance(o, UDTInstance) and low in o.fields:
                    return FieldRef(o, low)
                if isinstance(o, ClsInstance) and low in o.vars:
                    return o.vars[low]
                return self.member_get(o, low, [], desc, node.name)
            return fm
        return self.cx(node, ctx)

    def with_obj(self, wid):
        def f(fr):
            return fr.h[("with", wid)]
        return f

    def obj_desc(self, node, ctx):
        """Description of an object expression for OracleUnbound; '' for a loaded class variable
        (an unset one is a real VB error 91, not a missing shim)."""
        root = node
        while root is not None and root.kind in ("member", "call", "bang"):
            root = root.obj if root.kind != "call" else root.target
        if root is not None and root.kind == "me":
            return ""
        if root is not None and root.kind == "name" and node is root:
            b = self.resolve(root.low, ctx)
            code = None
            if b[0] == "local":
                code = ctx.locals[root.low]
            elif b[0] == "clsvar":
                code = self.it.type_code(ctx.module.varmap[root.low].typespec)
            elif b[0] == "global":
                code = b[1].t
            if code in self.it.classes:
                return ""
        return self.describe(node)

    def describe(self, node):
        if node is None:
            return "?"
        k = node.kind
        if k == "name":
            return node.name
        if k == "member":
            return (self.describe(node.obj) if node.obj is not None else "") + "." + node.name
        if k == "call":
            return self.describe(node.target) + "()"
        if k == "me":
            return "Me"
        if k == "bang":
            return (self.describe(node.obj) if node.obj is not None else "") + "!" + node.name
        return "?"

    # --- calls / indexing
    def x_call(self, node, ctx):
        return self.call_expr(node.target, node.args, ctx, False)

    def call_expr(self, target, args, ctx, is_stmt):
        it = self.it
        argfns = [self.carg(a, ctx) for _, a in args]
        names = [n for n, _ in args]
        tk = target.kind
        if tk == "name":
            low = target.low
            b = self.resolve(low, ctx)
            k = b[0]
            if k in ("local", "clsvar", "global"):
                base = self.name_value(target, ctx)
                name = target.name

                def fi(fr):
                    arr = base(fr)
                    return self.index_value(arr, [self.deref(a(fr)) for a in argfns], name)
                return fi
            if k == "proc":
                return self.call_proc_node(b, argfns, names, ctx)
            if k == "builtin":
                fn = b[1]

                def fb(fr):
                    vals = [MISSING if (v := a(fr)) is OMIT else (v.get() if isinstance(v, REFTYPES) else v)
                            for a in argfns]
                    return fn(it, *vals)
                return fb
            if k == "const":
                return self.index_expr(self.name_value(target, ctx), argfns, target.name)
            name = target.name

            def fe(fr):
                if low in it.ext_procs:
                    vals = [self.deref(a(fr)) for a in argfns]
                    return it.ext_procs[low](it, *vals)
                if low in it.externals:
                    o = it.externals[low]
                    vals = [self.deref(a(fr)) for a in argfns]
                    return self.default_member(o, vals, name)
                raise OracleUnbound(name)
            if k == "err":
                return self.index_expr(self.name_value(target, ctx), argfns, name)
            return fe
        if tk == "member":
            if target.obj is None:
                objfn = self.with_obj(target.withid)
                desc = "." + target.name
            else:
                objfn = self.cx(target.obj, ctx)
                desc = self.obj_desc(target.obj, ctx)
            low = target.low
            mname = target.name

            def fm(fr):
                o = objfn(fr)
                return self.member_get(o, low, [a(fr) for a in argfns], desc, mname, names)
            return fm
        base = self.cx(target, ctx)

        def fx(fr):
            return self.index_value(base(fr), [self.deref(a(fr)) for a in argfns], "?")
        return fx

    def index_expr(self, basefn, argfns, name):
        def f(fr):
            return self.index_value(basefn(fr), [self.deref(a(fr)) for a in argfns], name)
        return f

    @staticmethod
    def deref(v):
        if v is OMIT:
            return MISSING
        return v.get() if isinstance(v, REFTYPES) else v

    def index_value(self, arr, vals, name):
        if isinstance(arr, VBArray):
            if not vals:
                return arr
            return arr.get(tuple(V.int_value(v) for v in vals))
        if isinstance(arr, ExtObject):
            return arr.vb_default(vals)
        if isinstance(arr, ClsInstance) or arr is NOTHING:
            raise VBError(91 if arr is NOTHING else 438)
        if not vals:
            return arr
        raise VBError(13)

    def default_member(self, o, vals, name):
        if isinstance(o, ExtObject):
            return o.vb_default(vals)
        if isinstance(o, VBArray):
            return o.get(tuple(V.int_value(v) for v in vals))
        raise VBError(438)

    # --- members
    def x_member(self, node, ctx):
        # Enum.Member and module.member resolve at compile time when possible
        if node.obj is not None and node.obj.kind == "name":
            b = self.resolve(node.obj.low, ctx)
            if b[0] == "enum":
                ce = b[1].members.get(node.low)
                if ce is None:
                    compile_error("unknown enum member " + node.name, node)
                it = self.it
                return lambda fr: ce.get(it)
        if node.obj is None:
            objfn = self.with_obj(node.withid)
            desc = ""
        else:
            objfn = self.cx(node.obj, ctx)
            desc = self.obj_desc(node.obj, ctx)
        low = node.low
        name = node.name

        def f(fr):
            return self.member_get(objfn(fr), low, [], desc, name)
        return f

    def x_bang(self, node, ctx):
        objfn = self.with_obj(node.withid) if node.obj is None else self.cx(node.obj, ctx)
        desc = self.describe(node.obj) if node.obj is not None else ""
        name = node.name

        def f(fr):
            o = objfn(fr)
            if isinstance(o, ExtObject):
                return o.vb_bang(name)
            if o is NOTHING:
                raise OracleUnbound(desc or "?", "!" + name)
            raise VBError(438)
        return f

    def member_get(self, o, low, args, desc, name, names=None):
        it = self.it
        if isinstance(o, UDTInstance):
            if low not in o.fields:
                raise VBError(438)
            v = o.fields[low]
            if args:
                return self.index_value(v, [self.deref(a) for a in args], name)
            return v
        if isinstance(o, ClsInstance):
            cls = o.cls
            procs = cls.proc_map.get(low)
            if procs:
                proc = procs[0]
                for p in procs:
                    if p.kind in ("get", "function", "sub"):
                        proc = p
                        break
                return it.call_proc(proc, o, args, names or [None] * len(args))
            if low in o.vars:
                v = o.vars[low].v
                if args:
                    return self.index_value(v, [self.deref(a) for a in args], name)
                return v
            raise VBError(438)
        if isinstance(o, ExtObject):
            return o.vb_get(name, [self.deref(a) for a in args])
        if o is NOTHING:
            # unbound external object (or a genuinely unset reference)
            if self._is_external_desc(desc):
                raise OracleUnbound(desc, "." + name)
            raise VBError(91)
        if isinstance(o, ModuleRef):
            m = o.mod
            if low in m.constmap:
                return m.constmap[low].get(it)
            if low in m.cells:
                v = m.cells[low].v
                return self.index_value(v, [self.deref(a) for a in args], name) if args else v
            if low in m.proc_map:
                return it.call_proc(m.proc_map[low][0], None, args, names or [None] * len(args))
            raise VBError(438)
        if isinstance(o, EnumType):
            ce = o.members.get(low)
            if ce is None:
                raise VBError(438)
            return ce.get(it)
        raise VBError(424 if not V.is_num(o) and not isinstance(o, str) else 438)

    @staticmethod
    def _is_external_desc(desc):
        return bool(desc)

    def member_set(self, o, low, args, value, desc, name, is_set):
        it = self.it
        if isinstance(o, UDTInstance):
            if low not in o.fields:
                raise VBError(438)
            if args:
                arr = o.fields[low]
                arr.set(tuple(V.int_value(self.deref(a)) for a in args), value)
            else:
                o.set_field(low, value)
            return
        if isinstance(o, ClsInstance):
            procs = o.cls.proc_map.get(low)
            if procs:
                want = ("set", "let") if is_set else ("let", "set")
                for w in want:
                    for p in procs:
                        if p.kind == w:
                            it.call_proc(p, o, list(args) + [value], [None] * (len(args) + 1))
                            return
                raise VBError(451)
            if low in o.vars:
                cell = o.vars[low]
                if args:
                    cell.v.set(tuple(V.int_value(self.deref(a)) for a in args), value)
                else:
                    self.assign_cell(cell, value)
                return
            raise VBError(438)
        if isinstance(o, ExtObject):
            o.vb_set(name, [self.deref(a) for a in args], value)
            return
        if o is NOTHING:
            if desc:
                raise OracleUnbound(desc, "." + name + " =")
            raise VBError(91)
        if isinstance(o, ModuleRef):
            m = o.mod
            if low in m.cells:
                self.assign_cell(m.cells[low], value)
                return
            raise VBError(438)
        raise VBError(438)

    @staticmethod
    def assign_cell(cell, value):
        cur = cell.v
        if isinstance(cur, VBArray):
            if isinstance(value, VBArray):
                cur.assign_from(value)
                return
            raise VBError(13)
        if isinstance(value, UDTInstance):
            value = value.copy()
        t = cell.t
        f = V.COERCE.get(t)
        if f is not None:
            value = f(value)
        elif isinstance(value, VBArray) and t == "variant":
            value = value.copy()
        cell.v = value

    # ------------------------------------------------------------ assignment targets
    def setter(self, target, ctx, is_set=False):
        """Returns fn(fr, value)."""
        it = self.it
        k = target.kind
        if k == "name":
            low = target.low
            b = self.resolve(low, ctx)
            bk = b[0]
            ac = self.assign_cell
            if bk == "local":
                code = ctx.locals[low]
                f = V.COERCE.get(code)
                if f is not None:
                    def sl(fr, value):
                        fr.v[low].v = f(value)
                    return sl
                return lambda fr, value: ac(fr.v[low], value)
            if bk == "clsvar":
                return lambda fr, value: ac(fr.this.vars[low], value)
            if bk == "global":
                cell = b[1]
                return lambda fr, value: ac(cell, value)
            if bk == "proc":
                procs = b[1]
                same_class = b[2] is not None and b[2].is_class
                setp = [p for p in procs if p.kind in (("set", "let") if is_set else ("let", "set"))]
                if not setp:
                    compile_error("cannot assign to procedure " + target.name, target)
                proc = setp[0]
                return lambda fr, value: it.call_proc(proc, fr.this if same_class else None, [value], [None])
            if bk == "err":
                compile_error("cannot assign to Err", target)
            if bk in ("const", "builtin", "bconst"):
                compile_error("cannot assign to " + target.name, target)
            name = target.name

            def se(fr, value):
                cur = it.externals.get(low)
                if cur is None:
                    raise OracleUnbound(name, "assignment")
                it.externals[low] = value
            return se
        if k == "member":
            if target.obj is None:
                objfn = self.with_obj(target.withid)
                desc = ""
            else:
                objfn = self.cx(target.obj, ctx)
                desc = self.obj_desc(target.obj, ctx)
            low, name = target.low, target.name
            return lambda fr, value: self.member_set(objfn(fr), low, [], value, desc, name, is_set)
        if k == "call":
            t = target.target
            argfns = [self.carg(a, ctx) for _, a in target.args]
            if t.kind == "name":
                low = t.low
                b = self.resolve(low, ctx)
                bk = b[0]
                if bk in ("local", "clsvar", "global"):
                    base = self.name_value(t, ctx)

                    def sa(fr, value):
                        arr = base(fr)
                        if not isinstance(arr, VBArray):
                            raise VBError(13)
                        if not argfns:
                            if isinstance(value, VBArray):
                                arr.assign_from(value)
                                return
                            raise VBError(13)
                        arr.set(tuple(V.int_value(self.deref(a(fr))) for a in argfns), value)
                    return sa
                if bk == "proc":
                    procs = b[1]
                    same_class = b[2] is not None and b[2].is_class
                    setp = [p for p in procs if p.kind in ("let", "set")]
                    if not setp:
                        compile_error("cannot assign to call " + t.name, t)
                    proc = setp[0]

                    def sp(fr, value):
                        vals = [a(fr) for a in argfns] + [value]
                        it.call_proc(proc, fr.this if same_class else None, vals, [None] * len(vals))
                    return sp

                name = t.name

                def se2(fr, value):
                    o = it.externals.get(low)
                    if isinstance(o, ExtObject):
                        o.vb_set("", [self.deref(a(fr)) for a in argfns], value)
                        return
                    raise OracleUnbound(name, "indexed assignment")
                return se2
            if t.kind == "member":
                if t.obj is None:
                    objfn = self.with_obj(t.withid)
                    desc = ""
                else:
                    objfn = self.cx(t.obj, ctx)
                    desc = self.obj_desc(t.obj, ctx)
                low, name = t.low, t.name
                return lambda fr, value: self.member_set(objfn(fr), low, [a(fr) for a in argfns], value, desc,
                                                         name, is_set)
        if k == "bang":
            objfn = self.with_obj(target.withid) if target.obj is None else self.cx(target.obj, ctx)
            desc = self.describe(target.obj) if target.obj is not None else ""
            name = target.name

            def sb(fr, value):
                o = objfn(fr)
                if isinstance(o, ExtObject):
                    o.vb_set("", [name], value)
                    return
                if o is NOTHING:
                    raise OracleUnbound(desc or "?", "!" + name + " =")
                raise VBError(438)
            return sb
        compile_error("unsupported assignment target " + k, target)

    # ------------------------------------------------------------ statements
    def stmts(self, body, ctx):
        for st in body:
            self.stmt(st, ctx)

    def emit(self, ctx, fn):
        ctx.cp.instrs.append(fn)
        return len(ctx.cp.instrs) - 1

    def place(self, ctx, label):
        label.pc = len(ctx.cp.instrs)

    def label_for(self, ctx, name):
        lab = ctx.labels.get(name)
        if lab is None:
            lab = ctx.labels[name] = Label(name)
        return lab

    def stmt(self, st, ctx):
        m = getattr(self, "s_" + st.kind, None)
        if m is None:
            compile_error("cannot compile statement " + st.kind, st)
        m(st, ctx)

    def s_dim(self, st, ctx):
        pass

    def s_label(self, st, ctx):
        lab = self.label_for(ctx, st.low)
        if lab.pc is not None:
            compile_error("duplicate label " + st.name, st)
        self.place(ctx, lab)

    def s_assign(self, st, ctx):
        val = self.cx(st.expr, ctx)
        setter = self.setter(st.target, ctx, st.is_set)
        tgt = st.target
        # fast path for plain primitive local variables
        if tgt.kind == "name" and tgt.low in ctx.locals and ctx.locals[tgt.low] in V.COERCE:
            low = tgt.low
            f = V.COERCE[ctx.locals[low]]

            def fa(fr):
                fr.v[low].v = f(val(fr))
            self.emit(ctx, fa)
            return

        def a(fr):
            setter(fr, val(fr))
        self.emit(ctx, a)

    def s_callstmt(self, st, ctx):
        f = self.call_expr(st.target, st.args, ctx, True)

        def c(fr):
            f(fr)
        self.emit(ctx, c)

    def s_if(self, st, ctx):
        end = Label()
        for cond, body in st.clauses:
            nxt = Label()
            c = self.cx(cond, ctx)
            truth = V.truth

            def test(fr, c=c, nxt=nxt):
                r = c(fr)
                if r is True or (r is not False and truth(r)):
                    return None
                return nxt.pc
            self.emit(ctx, test)
            self.stmts(body, ctx)
            self.emit(ctx, lambda fr: end.pc)
            self.place(ctx, nxt)
        if st.els:
            self.stmts(st.els, ctx)
        self.place(ctx, end)

    def s_select(self, st, ctx):
        self.sel_counter += 1
        key = ("sel", self.sel_counter)
        e = self.cx(st.expr, ctx)

        def setsel(fr):
            fr.h[key] = e(fr)
        self.emit(ctx, setsel)
        end = Label()
        for tests, body in st.cases:
            nxt = Label()
            body_start = Label()
            matchers = []
            for t in tests:
                if t[0] == "val":
                    vf = self.cx(t[1], ctx)
                    matchers.append(lambda fr, s, vf=vf: V.compare(s, vf(fr)) == 0)
                elif t[0] == "range":
                    lo, hi = self.cx(t[1], ctx), self.cx(t[2], ctx)
                    matchers.append(lambda fr, s, lo=lo, hi=hi: V.compare(s, lo(fr)) >= 0 and V.compare(s, hi(fr)) <= 0)
                else:
                    op, vf = t[1], self.cx(t[2], ctx)
                    matchers.append(self.is_matcher(op, vf))

            def test(fr, matchers=matchers, nxt=nxt):
                s = fr.h[key]
                for mt in matchers:
                    if mt(fr, s):
                        return None
                return nxt.pc
            self.emit(ctx, test)
            self.stmts(body, ctx)
            self.emit(ctx, lambda fr: end.pc)
            self.place(ctx, nxt)
        if st.els:
            self.stmts(st.els, ctx)
        self.place(ctx, end)

    @staticmethod
    def is_matcher(op, vf):
        if op == "<":
            return lambda fr, s: V.compare(s, vf(fr)) < 0
        if op == ">":
            return lambda fr, s: V.compare(s, vf(fr)) > 0
        if op == "<=":
            return lambda fr, s: V.compare(s, vf(fr)) <= 0
        if op == ">=":
            return lambda fr, s: V.compare(s, vf(fr)) >= 0
        if op == "=":
            return lambda fr, s: V.compare(s, vf(fr)) == 0
        if op == "<>":
            return lambda fr, s: V.compare(s, vf(fr)) != 0
        raise VBSyntaxError("bad Case Is operator " + op)

    def s_for(self, st, ctx):
        self.sel_counter += 1
        key = ("for", self.sel_counter)
        get = self.cx(st.var, ctx)
        setv = self.setter(st.var, ctx)
        start, end, step = self.cx(st.start, ctx), self.cx(st.end, ctx), (self.cx(st.step, ctx) if st.step else None)
        top, cont, out = Label(), Label(), Label()

        def init(fr):
            s = start(fr)
            e = end(fr)
            sp = step(fr) if step is not None else I16(1)
            if V.compare(sp, I16(0)) == 0:
                fr.h[key] = (e, sp, 0)
            else:
                fr.h[key] = (e, sp, 1 if V.compare(sp, I16(0)) > 0 else -1)
            setv(fr, s)
        self.emit(ctx, init)
        self.place(ctx, top)

        def test(fr):
            e, sp, direction = fr.h[key]
            c = V.compare(get(fr), e)
            if direction >= 0:
                if c > 0:
                    return out.pc
            elif c < 0:
                return out.pc
            return None
        self.emit(ctx, test)
        ctx.loops.append(("for", out))
        self.stmts(st.body, ctx)
        ctx.loops.pop()
        self.place(ctx, cont)

        def incr(fr):
            e, sp, direction = fr.h[key]
            try:
                setv(fr, V.add(get(fr), sp))
            except VBError as ex:
                if ex.number == 6:
                    return out.pc          # counter would leave its type's range: loop is finished
                raise
            return top.pc
        self.emit(ctx, incr)
        self.place(ctx, out)

    def s_foreach(self, st, ctx):
        self.sel_counter += 1
        key = ("each", self.sel_counter)
        coll = self.cx(st.coll, ctx)
        setv = self.setter(st.var, ctx, True)
        top, out = Label(), Label()
        desc = self.describe(st.coll)

        def init(fr):
            c = coll(fr)
            if isinstance(c, VBArray):
                items = list(c.data or [])
            elif isinstance(c, ExtObject):
                items = list(c.vb_iter())
            elif c is NOTHING:
                raise OracleUnbound(desc, "For Each")
            else:
                raise VBError(438)
            fr.h[key] = [items, 0]
        self.emit(ctx, init)
        self.place(ctx, top)

        def nxt(fr):
            st_ = fr.h[key]
            if st_[1] >= len(st_[0]):
                return out.pc
            v = st_[0][st_[1]]
            st_[1] += 1
            setv(fr, v)
            return None
        self.emit(ctx, nxt)
        ctx.loops.append(("for", out))
        self.stmts(st.body, ctx)
        ctx.loops.pop()
        self.emit(ctx, lambda fr: top.pc)
        self.place(ctx, out)

    def s_do(self, st, ctx):
        top, out = Label(), Label()
        cond = self.cx(st.cond, ctx) if st.cond is not None else None
        until = st.until
        truth = V.truth
        self.place(ctx, top)
        if st.pre and cond is not None:
            def test(fr):
                c = truth(cond(fr))
                if c == until:
                    return out.pc
                return None
            self.emit(ctx, test)
        ctx.loops.append(("do", out))
        self.stmts(st.body, ctx)
        ctx.loops.pop()
        if not st.pre and cond is not None:
            def test2(fr):
                c = truth(cond(fr))
                if c == until:
                    return out.pc
                return top.pc
            self.emit(ctx, test2)
        else:
            self.emit(ctx, lambda fr: top.pc)
        self.place(ctx, out)

    def s_with(self, st, ctx):
        e = self.cx(st.expr, ctx)
        key = ("with", st.wid)

        def setw(fr):
            fr.h[key] = e(fr)
        self.emit(ctx, setw)
        self.stmts(st.body, ctx)

    def s_exit(self, st, ctx):
        w = st.what
        if w in ("sub", "function", "property"):
            self.emit(ctx, lambda fr: EXIT)
            return
        for kind, lab in reversed(ctx.loops):
            if kind == w:
                self.emit(ctx, lambda fr, lab=lab: lab.pc)
                return
        compile_error("Exit %s outside a loop" % w, st)

    def s_goto(self, st, ctx):
        lab = self.label_for(ctx, st.label)
        self.emit(ctx, lambda fr: lab.pc)

    def s_end(self, st, ctx):
        def f(fr):
            raise EndProgram()
        self.emit(ctx, f)

    def s_onerror(self, st, ctx):
        it = self.it
        mode = st.mode
        if mode == "goto":
            lab = self.label_for(ctx, st.label)

            def f(fr):
                fr.on_error = lab.pc
                fr.handling = False
                it.err.clear()
        elif mode == "next":
            def f(fr):
                fr.on_error = "next"
                fr.handling = False
                it.err.clear()
        else:
            def f(fr):
                fr.on_error = None
                fr.handling = False
                it.err.clear()
        self.emit(ctx, f)

    def s_resume(self, st, ctx):
        it = self.it
        t = st.target
        if t is None:
            def f(fr):
                fr.handling = False
                it.err.clear()
                return fr.err_pc
        elif t == "next":
            def f(fr):
                fr.handling = False
                it.err.clear()
                return fr.err_pc + 1
        else:
            lab = self.label_for(ctx, t)

            def f(fr):
                fr.handling = False
                it.err.clear()
                return lab.pc
        self.emit(ctx, f)

    def s_erase(self, st, ctx):
        gets = [self.cx(t, ctx) for t in st.targets]

        def f(fr):
            for g in gets:
                a = g(fr)
                if isinstance(a, VBArray):
                    a.erase()
        self.emit(ctx, f)

    def s_redim(self, st, ctx):
        it = self.it
        items = []
        for target, dims, ts in st.items:
            get = self.cx(target, ctx)
            setv = self.setter(target, ctx)
            code = it.type_code(ts, ctx.module) if ts is not None else None
            bounds = []
            base = ctx.module.option_base
            for lo, hi in dims:
                bounds.append((self.cx(lo, ctx) if lo is not None else None, self.cx(hi, ctx)))
            items.append((get, setv, code, bounds, base))
        preserve = st.preserve

        def f(fr):
            for get, setv, code, bounds, base in items:
                b = [((base if lo is None else V.int_value(lo(fr))), V.int_value(hi(fr))) for lo, hi in bounds]
                arr = get(fr)
                if not isinstance(arr, VBArray):
                    arr = VBArray(code or "variant")
                    arr.dynamic = True
                    setv(fr, arr)
                    arr = get(fr)
                if code is not None:
                    arr.elemtype = code
                    if code in it.udts:
                        arr.factory = lambda c=code: it.new_udt(c)
                arr.dynamic = True
                arr.allocate(b, preserve)
        self.emit(ctx, f)

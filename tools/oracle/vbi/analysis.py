"""Static analysis of loaded procedures: which names need shims, and which procedures run without."""
import signal

from .ast import Node
from .errors import VBError, OracleUnbound, EndProgram
from . import values as V
from .objects import VBArray, ClsInstance, UDTInstance
from .compiler import Ctx
from .interp import OMIT


def children(node):
    for k, v in node.__dict__.items():
        if k in ("kind", "line"):
            continue
        yield from _walk_value(v)


def _walk_value(v):
    if isinstance(v, Node):
        yield v
    elif isinstance(v, (list, tuple)):
        for x in v:
            yield from _walk_value(x)


def walk(node):
    yield node
    for c in children(node):
        yield from walk(c)


def proc_key(p):
    suffix = ":" + p.kind if p.kind in ("get", "let", "set") else ""
    return (p.module.name, p.name + suffix)


class Usage:
    def __init__(self):
        self.recordsets = {}      # global name -> {"type", "fields": set, "dynamic": set, "members": set}
        self.objects = {}         # other external-typed globals -> {"type", "members": set}
        self.forms = {}           # form name -> {"controls": {ctl: set(props)}, "members": set}
        self.globals_read = {}    # declared scalar globals -> type
        self.globals_written = {}
        self.other_procs = {}     # undeclared name used as procedure -> set of callers
        self.other_names = {}     # undeclared name used as value
        self.intrinsics = {}      # unsupported VB intrinsic (MsgBox, CreateObject, ...) -> procedures
        self.calls = {}           # (module, proc) -> set of (module, proc) loaded callees
        self.unbound_by_proc = {}  # (module, proc) -> set of names


def _lit_pieces(expr):
    """Literal text of a field-name expression; ('lit', s) or ('prefix', s) for "Abil-" & x."""
    if expr is None:
        return None
    if expr.kind == "lit" and isinstance(expr.value, str):
        return ("lit", expr.value)
    if expr.kind == "paren":
        return _lit_pieces(expr.e)
    if expr.kind == "bin" and expr.op == "&":
        left = _lit_pieces(expr.l)
        if left is not None:
            return ("prefix", left[1])
    return ("dynamic", "")


def analyze(it, module_names=None):
    it.ensure_ready()
    use = Usage()
    comp = it.compiler
    mods = [m for m in it.modules.values() if m.load_procs and (module_names is None or m.low in module_names)]
    for m in mods:
        for proc in m.ast.procs:
            _analyze_proc(it, comp, m, proc, use)
    return use


def _local_ctx(it, comp, m, proc):
    ctx = Ctx(m, proc)

    class _CP:
        static_cells = {}
        locals = []

    cp = _CP()
    ctx.cp = cp
    for p in proc.params:
        ctx.locals[p.low] = it.type_code(p.typespec, m)
    if proc.kind in ("function", "get"):
        ctx.locals[proc.low] = it.type_code(proc.rettype, m) if proc.rettype else "variant"
    comp.collect_decls(proc.body, ctx, cp, m)
    return ctx


def _chain(node):
    """Flatten a member/call chain into (root, [(kind, name, args)]) ."""
    items = []
    cur = node
    while cur is not None:
        if cur.kind == "member":
            items.append(("member", cur.name, None))
            cur = cur.obj
        elif cur.kind == "call":
            items.append(("call", None, cur.args))
            cur = cur.target
        elif cur.kind == "bang":
            items.append(("bang", cur.name, None))
            cur = cur.obj
        else:
            break
    items.reverse()
    return cur, items


def _analyze_proc(it, comp, m, proc, use):
    ctx = _local_ctx(it, comp, m, proc)
    key = proc_key(proc)
    callees = use.calls.setdefault(key, set())
    unb = use.unbound_by_proc.setdefault(key, set())
    written_roots = set()
    for st in walk_nodes(proc.body):
        if st.kind == "assign":
            r, _ = _chain(st.target) if st.target.kind != "name" else (st.target, [])
            if r is not None and r.kind == "name":
                written_roots.add(id(r))
        elif st.kind in ("for", "foreach"):
            r, _ = _chain(st.var) if st.var.kind != "name" else (st.var, [])
            if r is not None and r.kind == "name":
                written_roots.add(id(r))
    handled = set()
    for node in walk_nodes(proc.body):
        if node.kind in ("member", "call", "bang") and id(node) not in handled:
            root, items = _chain(node)
            for n2 in _inner_chain_nodes(node):
                handled.add(id(n2))
            if root is None or root.kind != "name":
                continue
            _classify(it, comp, m, ctx, root, items, use, key, callees, unb, written_roots, node)
        elif node.kind == "name" and id(node) not in handled:
            _classify(it, comp, m, ctx, node, [], use, key, callees, unb, written_roots, node)
        elif node.kind == "callstmt":
            root, items = _chain(node.target)
            for n2 in [node.target] + list(_inner_chain_nodes(node.target)):
                handled.add(id(n2))
            if root is not None and root.kind == "name":
                items = list(items) + [("call", None, node.args)] if not items else items + [("call", None, node.args)]
                _classify(it, comp, m, ctx, root, items, use, key, callees, unb, written_roots, node.target)


def walk_nodes(body):
    for st in body:
        yield from walk_stmt(st)


def walk_stmt(st):
    yield st
    for k, v in st.__dict__.items():
        if k in ("kind", "line"):
            continue
        yield from _walk_any(v)


def _walk_any(v):
    if isinstance(v, Node):
        yield from walk_stmt(v)
    elif isinstance(v, (list, tuple)):
        for x in v:
            yield from _walk_any(x)


def _inner_chain_nodes(node):
    cur = node
    while cur is not None and cur.kind in ("member", "call", "bang"):
        cur = cur.obj if cur.kind != "call" else cur.target
        if cur is not None and cur.kind in ("member", "call", "bang"):
            yield cur


def _classify(it, comp, m, ctx, root, items, use, key, callees, unb, written_roots, whole):
    low = root.low
    b = comp.resolve(low, ctx)
    k = b[0]
    if k in ("local", "clsvar"):
        return
    if k == "global":
        cell = b[1]
        t = cell.t
        if t.startswith("array:") or t in ("long", "integer", "single", "double", "currency", "string",
                                            "boolean", "byte", "variant", "date") or t in it.udts:
            if id(root) in written_roots:
                use.globals_written[low] = (root.name, t)
            else:
                use.globals_read[low] = (root.name, t)
            return
        if t == "recordset":
            rec = use.recordsets.setdefault(root.name, {"type": "Recordset", "fields": set(), "dynamic": set(),
                                                         "members": set()})
            unb.add(root.name)
            _record_members(rec, items)
            return
        if t in it.classes:
            return
        obj = use.objects.setdefault(root.name, {"type": t, "members": set()})
        unb.add(root.name)
        for kind, name, args in items:
            if kind in ("member", "bang"):
                obj["members"].add(name)
                break
        return
    if k == "proc":
        procs = b[1]
        p = procs[0]
        callees.add(proc_key(p))
        return
    if k == "builtin":
        un = getattr(b[1], "vb_unbound", None)
        if un:
            unb.add(un)
            use.intrinsics.setdefault(un, set()).add(key[1])
        return
    if k == "ext":
        unb.add(root.name)
        if low.startswith("frm"):
            f = use.forms.setdefault(root.name, {"controls": {}, "members": set()})
            first = None
            for kind, name, args in items:
                if kind == "member":
                    if first is None:
                        first = name
                        f["controls"].setdefault(name, set())
                    else:
                        f["controls"][first].add(name)
                        break
            if first is None:
                pass
            return
        is_call = bool(items) and items[0][0] == "call"
        if is_call:
            use.other_procs.setdefault(root.name, set()).add(key[1])
        else:
            use.other_names.setdefault(root.name, set()).add(key[1])
        return
    return


def _record_members(rec, items):
    i = 0
    n = len(items)
    while i < n:
        kind, name, args = items[i]
        if kind == "member" and name.lower() == "fields":
            if i + 1 < n and items[i + 1][0] == "call" and items[i + 1][2]:
                a = items[i + 1][2][0][1]
                p = _lit_pieces(a)
                if p[0] == "lit":
                    rec["fields"].add(p[1])
                elif p[0] == "prefix":
                    rec["dynamic"].add(p[1] + "*")
                else:
                    rec["dynamic"].add("?")
            i += 2
            rec["members"].add("Fields")
            continue
        if kind == "bang":
            rec["fields"].add(name)
            i += 1
            continue
        if kind == "member":
            rec["members"].add(name)
            if name.lower() == "fields":
                pass
            # members past the first are properties of the returned field/value; stop here
            if name.lower() not in ("fields",):
                break
        i += 1


# ----------------------------------------------------------------------------- runnable procedures

def transitive_clean(use, loaded_keys):
    """Procedures whose reachable call graph touches no unbound name."""
    memo = {}

    def clean(k, stack):
        if k in memo:
            return memo[k]
        if k in stack:
            return True
        if use.unbound_by_proc.get(k):
            memo[k] = False
            return False
        stack = stack | {k}
        ok = True
        for c in use.calls.get(k, ()):
            if c in use.calls and not clean(c, stack):
                ok = False
                break
        memo[k] = ok
        return ok

    return {k for k in loaded_keys if clean(k, frozenset())}


class _Timeout(Exception):
    pass


def _alarm(signum, frame):
    raise _Timeout()


def sample_args(it, proc, module):
    args = []
    names = []
    for i, p in enumerate(proc.params):
        code = it.type_code(p.typespec, module)
        if p.is_array:
            arr = VBArray(code if code != "variant" else "variant")
            arr.allocate([(0, 2)])
            args.append(arr)
        elif code == "integer":
            args.append(V.I16(2))
        elif code == "long":
            args.append(V.I32(2))
        elif code == "byte":
            args.append(V.U8(2))
        elif code in ("double", "single"):
            args.append(V.c_dbl(2.5) if code == "double" else V.c_sng(2.5))
        elif code == "currency":
            args.append(V.c_cur(2.5))
        elif code == "string":
            args.append("abc")
        elif code == "boolean":
            args.append(False)
        elif code in it.udts:
            args.append(it.new_udt(code))
        elif code == "variant":
            args.append(V.I16(1))
        else:
            args.append(OMIT if p.optional else V.NOTHING)
        names.append(None)
    return args, names


def smoke_run(it, proc, timeout=5):
    """Call a procedure with simple arguments. Returns (status, detail)."""
    module = proc.module.loaded
    this = None
    if module.is_class:
        this = it.instantiate(module)
    args, names = sample_args(it, proc, module)
    old = signal.signal(signal.SIGALRM, _alarm)
    signal.alarm(timeout)
    try:
        r = it.call_proc(proc, this, args, names)
        return "ok", r
    except OracleUnbound as e:
        return "unbound", e.name
    except VBError as e:
        return "vberror", e.number
    except EndProgram:
        return "end", None
    except _Timeout:
        return "timeout", None
    except RecursionError:
        return "recursion", None
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, old)

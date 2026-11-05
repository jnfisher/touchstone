"""AST node types. Deliberately plain: a Node has a kind and attributes."""


class Node:
    def __init__(self, kind, line=0, **kw):
        self.kind = kind
        self.line = line
        self.__dict__.update(kw)

    def __repr__(self):
        items = ", ".join("%s=%r" % (k, v) for k, v in self.__dict__.items() if k not in ("kind", "line"))
        return "%s(%s)" % (self.kind, items)


class TypeSpec:
    """A declared type: primitive code, or the name of a UDT/Enum/class/external object type."""

    def __init__(self, name, new=False, strlen=None):
        self.name = name                # original spelling, e.g. 'Long', 'Recordset', 'tCharacterProfile'
        self.low = name.lower()
        self.new = new
        self.strlen = strlen

    def __repr__(self):
        return "TypeSpec(%s)" % self.name


class Param:
    def __init__(self, name, byref, optional, default, typespec, is_array, paramarray=False):
        self.name = name
        self.low = name.lower()
        self.byref = byref
        self.optional = optional
        self.default = default
        self.typespec = typespec        # TypeSpec or None (Variant)
        self.is_array = is_array
        self.paramarray = paramarray


class VarDecl:
    def __init__(self, name, typespec, dims, scope, line, static=False, const_expr=None, is_const=False):
        self.name = name
        self.low = name.lower()
        self.typespec = typespec        # TypeSpec or None => Variant
        self.dims = dims                # None, [] (dynamic), or [(lo_expr|None, hi_expr), ...]
        self.scope = scope              # 'public' | 'private' | 'local'
        self.line = line
        self.static = static
        self.const_expr = const_expr
        self.is_const = is_const


class Proc:
    def __init__(self, name, kind, scope, params, rettype, ret_is_array, line, static=False):
        self.name = name
        self.low = name.lower()
        self.kind = kind                # sub | function | get | let | set
        self.scope = scope
        self.params = params
        self.rettype = rettype
        self.ret_is_array = ret_is_array
        self.line = line
        self.end_line = line
        self.static = static
        self.body = []
        self.module = None              # set by Module
        self.compiled = None
        self.locals = None

    @property
    def key(self):
        return self.low

    def __repr__(self):
        return "Proc(%s %s)" % (self.kind, self.name)


class Module:
    def __init__(self, name, kind):
        self.name = name
        self.low = name.lower()
        self.kind = kind                # bas | cls
        self.vars = []                  # VarDecl (module level, includes constants)
        self.types = {}                 # lname -> UDTDef-like (name, fields)
        self.enums = {}                 # lname -> [(member, expr)]
        self.procs = []                 # Proc
        self.declares = []              # (name, kind, lib)
        self.option_base = 0
        self.proc_map = {}              # lname -> [Proc] (property variants share a name)
        self.option_explicit = False

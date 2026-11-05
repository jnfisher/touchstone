"""Runtime containers: variables, arrays, user-defined-type instances, class instances."""
from .errors import VBError
from . import values as V


class Var:
    """A variable cell. ByRef passing shares the cell itself."""
    __slots__ = ("v", "t")

    def __init__(self, v, t):
        self.v = v
        self.t = t        # declared type code ('integer', ..., 'variant', 'object', or a type name)

    def get(self):
        return self.v

    def set(self, value):
        self.v = V.coerce_to(value, self.t) if self.t in V.COERCE else value


class ElemRef:
    """Reference to an array element (ByRef argument / assignment target)."""
    __slots__ = ("arr", "idx")

    def __init__(self, arr, idx):
        self.arr = arr
        self.idx = idx

    def get(self):
        return self.arr.get(self.idx)

    def set(self, value):
        self.arr.set(self.idx, value)

    @property
    def t(self):
        return self.arr.elemtype

    @property
    def v(self):
        return self.arr.get(self.idx)


class FieldRef:
    __slots__ = ("obj", "name")

    def __init__(self, obj, name):
        self.obj = obj
        self.name = name

    def get(self):
        return self.obj.fields[self.name]

    def set(self, value):
        self.obj.set_field(self.name, value)

    @property
    def t(self):
        return self.obj.defn.field_types[self.name]

    @property
    def v(self):
        return self.obj.fields[self.name]


class VBArray:
    """A (possibly unallocated) array with explicit bounds per dimension."""

    def __init__(self, elemtype, bounds=None, factory=None):
        self.elemtype = elemtype          # type code or type name
        self.factory = factory            # creates a default element (UDTs, Variants)
        self.bounds = None
        self.data = None
        if bounds is not None:
            self.allocate(bounds)

    def allocate(self, bounds, preserve=False):
        old_bounds, old_data = self.bounds, self.data
        n = 1
        for lo, hi in bounds:
            if hi < lo - 1:
                raise VBError(9)
            n *= (hi - lo + 1)
        data = [self._new() for _ in range(n)]
        if preserve and old_data is not None:
            if len(old_bounds) != len(bounds):
                raise VBError(9)
            # only the last dimension may change; copy overlapping elements
            self.bounds = bounds
            for idx, val in self._iter_old(old_bounds, old_data):
                if self._in_range(idx, bounds):
                    data[self._flat(idx, bounds)] = val
        self.bounds = bounds
        self.data = data

    @staticmethod
    def _iter_old(bounds, data):
        sizes = [hi - lo + 1 for lo, hi in bounds]
        n = len(data)
        for flat in range(n):
            rem = flat
            idx = []
            for d in range(len(bounds) - 1, -1, -1):
                idx.append(bounds[d][0] + rem % sizes[d])
                rem //= sizes[d]
            yield tuple(reversed(idx)), data[flat]

    @staticmethod
    def _in_range(idx, bounds):
        return all(lo <= i <= hi for i, (lo, hi) in zip(idx, bounds))

    @staticmethod
    def _flat(idx, bounds):
        flat = 0
        for i, (lo, hi) in zip(idx, bounds):
            flat = flat * (hi - lo + 1) + (i - lo)
        return flat

    def _new(self):
        if self.factory is not None:
            return self.factory()
        return V.default_for(self.elemtype)

    def erase(self):
        if self.bounds is not None and self.dynamic:
            self.bounds = None
            self.data = None
        elif self.bounds is not None:
            self.data = [self._new() for _ in self.data]

    dynamic = False

    def _index(self, idx):
        b = self.bounds
        if b is None or len(idx) != len(b):
            raise VBError(9)
        flat = 0
        for i, (lo, hi) in zip(idx, b):
            if i < lo or i > hi:
                raise VBError(9)
            flat = flat * (hi - lo + 1) + (i - lo)
        return flat

    def get(self, idx):
        return self.data[self._index(idx)]

    def set(self, idx, value):
        k = self._index(idx)
        if self.elemtype in V.COERCE:
            value = V.coerce_to(value, self.elemtype)
        elif isinstance(value, UDTInstance):
            value = value.copy()
        self.data[k] = value

    def lbound(self, dim=1):
        if self.bounds is None or dim < 1 or dim > len(self.bounds):
            raise VBError(9)
        return self.bounds[dim - 1][0]

    def ubound(self, dim=1):
        if self.bounds is None or dim < 1 or dim > len(self.bounds):
            raise VBError(9)
        return self.bounds[dim - 1][1]

    def copy(self):
        a = VBArray(self.elemtype, None, self.factory)
        a.dynamic = self.dynamic
        if self.bounds is not None:
            a.bounds = list(self.bounds)
            a.data = [x.copy() if isinstance(x, UDTInstance) else x for x in self.data]
        return a

    def assign_from(self, other):
        """`a() = b()`: take b's shape and a copy of its elements."""
        c = other.copy()
        self.bounds, self.data = c.bounds, c.data

    def __iter__(self):
        return iter(self.data or [])


class UDTDef:
    def __init__(self, name):
        self.name = name
        self.fields = []              # [(lname, display, typespec, bounds_nodes or None)]
        self.field_types = {}         # lname -> type code / name
        self.resolved = None


class UDTInstance:
    __slots__ = ("defn", "fields")

    def __init__(self, defn, fields):
        self.defn = defn
        self.fields = fields          # lname -> value

    def set_field(self, name, value):
        t = self.defn.field_types[name]
        cur = self.fields[name]
        if isinstance(cur, VBArray):
            if isinstance(value, VBArray):
                cur.assign_from(value)
                return
            raise VBError(13)
        if t in V.COERCE:
            value = V.coerce_to(value, t)
        elif isinstance(value, UDTInstance):
            value = value.copy()
        self.fields[name] = value

    def copy(self):
        return UDTInstance(self.defn, {k: (v.copy() if isinstance(v, (VBArray, UDTInstance)) else v)
                                       for k, v in self.fields.items()})


class ClsInstance:
    """An instance of a loaded class module."""
    __slots__ = ("cls", "vars", "__weakref__")

    def __init__(self, cls):
        self.cls = cls
        self.vars = {}


class ExtObject:
    """Base class for shims that stand in for COM objects, forms, recordsets.

    Subclasses override the hooks; the defaults raise OracleUnbound-free VB errors so a shim only
    implements what the formulas touch.
    """

    def vb_get(self, name, args):
        raise VBError(438)

    def vb_set(self, name, args, value):
        raise VBError(438)

    def vb_default(self, args):
        raise VBError(438)

    def vb_bang(self, name):
        return self.vb_default([name])

    def vb_iter(self):
        raise VBError(438)

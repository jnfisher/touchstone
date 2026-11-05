"""Recursive-descent parser for the VB6 subset (modules and class modules)."""
import re

from .ast import Node, TypeSpec, Param, VarDecl, Proc, Module
from .errors import VBSyntaxError
from .lexer import lex
from . import values as V

PRIMS = {"integer", "long", "single", "double", "currency", "string", "boolean", "byte", "variant", "date",
         "object"}

# identifiers that can never start a label or be treated as plain call targets
STMT_KW = {"dim", "redim", "if", "for", "do", "while", "select", "with", "exit", "goto", "on", "resume",
           "set", "let", "call", "const", "static", "public", "private", "global", "end", "erase", "stop",
           "gosub", "return", "option", "attribute", "declare", "type", "enum", "sub", "function",
           "property", "friend", "else", "elseif", "case", "next", "loop", "wend", "randomize", "event"}


def decode_source(raw):
    """MME sources are Windows-1252."""
    return raw.decode("cp1252", errors="replace") if isinstance(raw, bytes) else raw


class Parser:
    def __init__(self, text, name=None, kind="bas"):
        self.module_name = name
        self.toks = lex(text, name)
        self.p = 0
        self.kind = kind
        self.with_stack = []
        self.with_counter = 0
        self.spaced_paren_stop = False
        self.mod = Module(name or "?", kind)

    # ------------------------------------------------------------ token helpers
    @property
    def t(self):
        return self.toks[self.p]

    def peek(self, k=1):
        i = self.p + k
        return self.toks[i] if i < len(self.toks) else self.toks[-1]

    def err(self, msg, tok=None):
        tok = tok or self.t
        raise VBSyntaxError("%s (at %r)" % (msg, tok.text), tok.line, self.module_name)

    def next(self):
        t = self.toks[self.p]
        if self.p < len(self.toks) - 1:
            self.p += 1
        return t

    def is_op(self, s):
        t = self.t
        return t.kind == "op" and t.low == s

    def is_kw(self, s):
        t = self.t
        return t.kind == "id" and t.low == s

    def accept_op(self, s):
        if self.is_op(s):
            self.next()
            return True
        return False

    def accept_kw(self, s):
        if self.is_kw(s):
            self.next()
            return True
        return False

    def expect_op(self, s):
        if not self.accept_op(s):
            self.err("expected %r" % s)

    def expect_kw(self, s):
        if not self.accept_kw(s):
            self.err("expected %s" % s)

    def ident(self):
        t = self.t
        if t.kind != "id":
            self.err("expected identifier")
        self.next()
        return t

    def at_eol(self):
        return self.t.kind == "nl"

    def at_stmt_end(self):
        return self.t.kind == "nl" or self.is_op(":")

    def skip_eol(self):
        while self.t.kind == "nl" or self.is_op(":"):
            if self.p >= len(self.toks) - 1:
                break
            self.next()

    def skip_rest_of_line(self):
        while self.t.kind != "nl":
            self.next()

    # ------------------------------------------------------------ module level
    def parse_module(self):
        m = self.mod
        # skip class header (VERSION ... BEGIN ... END) handled by strip_header before lexing
        while True:
            self.skip_eol()
            if self.t.kind == "nl" and self.p >= len(self.toks) - 1:
                break
            t = self.t
            if t.kind == "nl":
                break
            line = t.line
            low = t.low if t.kind == "id" else ""
            if low == "attribute":
                self.parse_attribute()
            elif low == "option":
                self.parse_option()
            elif low == "implements":
                self.skip_rest_of_line()
            else:
                self.parse_decl_or_proc()
        return m

    def parse_attribute(self):
        self.next()
        if self.is_kw("vb_name"):
            self.next()
            self.expect_op("=")
            if self.t.kind == "str":
                self.mod.name = self.t.val
                self.mod.low = self.t.val.lower()
        self.skip_rest_of_line()

    def parse_option(self):
        self.next()
        if self.is_kw("base"):
            self.next()
            self.mod.option_base = int(self.next().val)
        elif self.is_kw("explicit"):
            self.mod.option_explicit = True
        self.skip_rest_of_line()

    def parse_scope(self):
        scope = None
        static = False
        while self.t.kind == "id" and self.t.low in ("public", "private", "global", "friend", "static"):
            k = self.next().low
            if k == "static":
                static = True
            elif k == "global":
                scope = "public"
            else:
                scope = k
        return scope, static

    def parse_decl_or_proc(self):
        start = self.p
        scope, static = self.parse_scope()
        t = self.t
        low = t.low if t.kind == "id" else ""
        if low in ("sub", "function", "property"):
            self.parse_proc(scope or "public", static)
        elif low == "declare":
            self.parse_declare()
        elif low == "type":
            self.parse_type()
        elif low == "enum":
            self.parse_enum(scope or "public")
        elif low == "const":
            self.next()
            self.parse_const_list(scope or "private", self.mod.vars)
        elif low == "event":
            self.skip_rest_of_line()
        elif low in ("dim", "withevents") or t.kind == "id":
            if low == "dim":
                self.next()
            if self.is_kw("withevents"):
                self.next()
            self.parse_var_list(scope or "private", self.mod.vars, static=static)
        else:
            self.err("unexpected token at module level")

    # ------------------------------------------------------------ types, enums, declares
    def parse_type(self):
        self.next()
        name = self.ident()
        self.skip_eol()
        fields = []
        while not (self.is_kw("end") and self.peek().low == "type"):
            if self.t.kind == "nl" and self.p >= len(self.toks) - 1:
                self.err("unterminated Type")
            ft = self.ident()
            dims = None
            if self.is_op("("):
                dims = self.parse_dims()
            ts = self.parse_as_type() if self.is_kw("as") else (self.suffix_type(ft) if ft.tc else None)
            fields.append(VarDecl(ft.text, ts, dims, "public", ft.line))
            self.skip_eol()
        self.next()
        self.next()
        self.mod.types[name.low] = (name.text, fields)

    def parse_enum(self, scope):
        self.next()
        name = self.ident()
        self.skip_eol()
        members = []
        while not (self.is_kw("end") and self.peek().low == "enum"):
            if self.t.kind == "nl" and self.p >= len(self.toks) - 1:
                self.err("unterminated Enum")
            mt = self.ident()
            expr = None
            if self.accept_op("="):
                expr = self.parse_expr()
            members.append((mt.text, expr))
            self.skip_eol()
        self.next()
        self.next()
        self.mod.enums[name.low] = (name.text, members)

    def parse_declare(self):
        self.next()
        kind = self.next().low           # sub / function
        name = self.ident()
        lib = None
        if self.is_kw("lib"):
            self.next()
            lib = self.next().val
        self.skip_rest_of_line()
        self.mod.declares.append((name.text, kind, lib))

    # ------------------------------------------------------------ variable / const declarations
    def suffix_type(self, tok):
        return TypeSpec({"integer": "Integer", "long": "Long", "single": "Single", "double": "Double",
                         "currency": "Currency", "string": "String"}[tok.tc])

    def parse_as_type(self):
        self.expect_kw("as")
        new = self.accept_kw("new")
        parts = [self.ident().text]
        while self.is_op(".") :
            self.next()
            parts.append(self.ident().text)
        ts = TypeSpec(parts[-1], new=new)
        if self.is_op("*"):
            self.next()
            ts.strlen = self.parse_expr_primary_const()
        return ts

    def parse_expr_primary_const(self):
        t = self.next()
        if t.kind == "num":
            return int(t.val)
        return t.text

    def parse_dims(self):
        self.expect_op("(")
        dims = []
        if self.accept_op(")"):
            return dims
        while True:
            a = self.parse_expr()
            if self.accept_kw("to"):
                b = self.parse_expr()
                dims.append((a, b))
            else:
                dims.append((None, a))
            if not self.accept_op(","):
                break
        self.expect_op(")")
        return dims

    def parse_var_list(self, scope, out, static=False):
        while True:
            nt = self.ident()
            dims = None
            if self.is_op("("):
                dims = self.parse_dims()
            ts = None
            if self.is_kw("as"):
                ts = self.parse_as_type()
            elif nt.tc:
                ts = self.suffix_type(nt)
            out.append(VarDecl(nt.text, ts, dims, scope, nt.line, static=static))
            if not self.accept_op(","):
                break

    def parse_const_list(self, scope, out):
        while True:
            nt = self.ident()
            ts = None
            if self.is_kw("as"):
                ts = self.parse_as_type()
            elif nt.tc:
                ts = self.suffix_type(nt)
            self.expect_op("=")
            expr = self.parse_expr()
            out.append(VarDecl(nt.text, ts, None, scope, nt.line, const_expr=expr, is_const=True))
            if not self.accept_op(","):
                break

    # ------------------------------------------------------------ procedures
    def parse_proc(self, scope, static):
        kw = self.next().low
        kind = kw
        if kw == "property":
            k = self.next().low
            kind = {"get": "get", "let": "let", "set": "set"}[k]
        nt = self.ident()
        params = []
        if self.accept_op("("):
            if not self.is_op(")"):
                while True:
                    params.append(self.parse_param())
                    if not self.accept_op(","):
                        break
            self.expect_op(")")
        rettype = None
        ret_arr = False
        if self.is_kw("as"):
            self.next()
            rettype = self.parse_as_type_after_as()
            if self.is_op("("):
                self.next()
                self.expect_op(")")
                ret_arr = True
        elif nt.tc:
            rettype = self.suffix_type(nt)
        proc = Proc(nt.text, kind, scope, params, rettype, ret_arr, nt.line, static=static)
        proc.module = self.mod
        self.with_stack = []
        proc.body = self.parse_block(("end", kw))
        end_tok = self.t
        self.next()
        self.next()
        proc.end_line = end_tok.line
        self.mod.procs.append(proc)
        self.mod.proc_map.setdefault(proc.low, []).append(proc)

    def parse_as_type_after_as(self):
        new = self.accept_kw("new")
        parts = [self.ident().text]
        while self.is_op("."):
            self.next()
            parts.append(self.ident().text)
        return TypeSpec(parts[-1], new=new)

    def parse_param(self):
        optional = False
        byref = True
        paramarray = False
        while self.t.kind == "id" and self.t.low in ("optional", "byval", "byref", "paramarray"):
            k = self.next().low
            if k == "optional":
                optional = True
            elif k == "byval":
                byref = False
            elif k == "byref":
                byref = True
            else:
                paramarray = True
        nt = self.ident()
        is_arr = False
        if self.is_op("("):
            self.next()
            self.expect_op(")")
            is_arr = True
        ts = None
        if self.is_kw("as"):
            self.next()
            ts = self.parse_as_type_after_as()
        elif nt.tc:
            ts = self.suffix_type(nt)
        default = None
        if self.accept_op("="):
            default = self.parse_expr()
        return Param(nt.text, byref, optional, default, ts, is_arr, paramarray)

    # ------------------------------------------------------------ statements
    def at_terminator(self, term):
        """term: tuple of lowercase keywords, e.g. ('end','if') or ('else',)."""
        for i, w in enumerate(term):
            t = self.peek(i)
            if t.kind != "id" or t.low != w:
                return False
        return True

    def parse_block(self, *terms):
        """Parse statements until one of the terminator keyword tuples (not consumed)."""
        terms = tuple(terms) if terms and isinstance(terms[0], tuple) else (tuple(terms),)
        out = []
        line_start = True
        while True:
            while self.t.kind == "nl" or self.is_op(":"):
                if self.t.kind == "nl":
                    line_start = True
                if self.p >= len(self.toks) - 1:
                    break
                self.next()
            if self.t.kind == "nl" and self.p >= len(self.toks) - 1:
                self.err("unexpected end of file in block")
            for tm in terms:
                if self.at_terminator(tm):
                    return out
            st = self.parse_statement(line_start)
            line_start = False
            if st is not None:
                out.append(st)
            # statement must end at eol or ':'
            if not (self.at_stmt_end() or self.is_kw("else")):
                self.err("expected end of statement")

    def parse_statement(self, line_start=False):
        t = self.t
        line = t.line
        if t.kind != "id" and not (t.kind == "op" and t.low == "."):
            self.err("unexpected token")
        low = t.low if t.kind == "id" else ""
        nxt = self.peek()
        # label
        if line_start and t.kind == "id" and nxt.kind == "op" and nxt.low == ":" and low not in STMT_KW:
            self.next()
            self.next()
            return Node("label", line, name=t.text, low=low)
        if line_start and t.kind == "id" and low == "error" and nxt.kind == "op" and nxt.low == ":":
            self.next()
            self.next()
            return Node("label", line, name=t.text, low=low)
        if low == "dim" or low == "static":
            return self.parse_dim_stmt()
        if low == "const":
            self.next()
            lst = []
            self.parse_const_list("local", lst)
            return Node("dim", line, decls=lst)
        if low == "redim":
            return self.parse_redim()
        if low == "erase":
            self.next()
            names = [self.parse_postfix_expr()]
            while self.accept_op(","):
                names.append(self.parse_postfix_expr())
            return Node("erase", line, targets=names)
        if low == "if":
            return self.parse_if()
        if low == "select":
            return self.parse_select()
        if low == "for":
            return self.parse_for()
        if low == "do":
            return self.parse_do()
        if low == "while":
            self.next()
            cond = self.parse_expr()
            self.skip_rest_of_line_ok()
            body = self.parse_block(("wend",))
            self.next()
            return Node("do", line, pre=True, until=False, cond=cond, body=body)
        if low == "with":
            return self.parse_with()
        if low == "exit":
            self.next()
            what = self.next().low
            return Node("exit", line, what=what)
        if low == "goto":
            self.next()
            lab = self.next()
            return Node("goto", line, label=lab.text.lower())
        if low == "gosub" or low == "return":
            self.err("GoSub/Return not supported")
        if low == "on":
            return self.parse_on()
        if low == "resume":
            self.next()
            if self.is_kw("next"):
                self.next()
                return Node("resume", line, target="next")
            if self.at_stmt_end():
                return Node("resume", line, target=None)
            lab = self.next()
            return Node("resume", line, target=lab.text.lower())
        if low == "set":
            self.next()
            tgt = self.parse_postfix_expr()
            self.expect_op("=")
            return Node("assign", line, target=tgt, expr=self.parse_expr(), is_set=True)
        if low == "let":
            self.next()
            tgt = self.parse_postfix_expr()
            self.expect_op("=")
            return Node("assign", line, target=tgt, expr=self.parse_expr(), is_set=False)
        if low == "call":
            self.next()
            e = self.parse_postfix_expr()
            return self.make_call_stmt(e, [], line)
        if low == "end":
            self.next()
            return Node("end", line)
        if low == "stop":
            self.next()
            return Node("end", line)
        if low in ("public", "private", "global"):
            self.err("declaration inside procedure")
        # assignment or call
        start = self.p
        e = self.parse_postfix_expr()
        if self.is_op(",") and e.kind == "call":
            # `Obj.Add (1), x, y`: the parenthesis belongs to the first bare argument
            self.p = start
            self.spaced_paren_stop = True
            try:
                e = self.parse_postfix_expr()
            finally:
                self.spaced_paren_stop = False
            return self.make_call_stmt(e, self.parse_arg_list_noparen(), line)
        if self.is_op("="):
            self.next()
            return Node("assign", line, target=e, expr=self.parse_expr(), is_set=False)
        # call with unparenthesised arguments
        if self.expr_starts():
            args = self.parse_arg_list_noparen()
            return self.make_call_stmt(e, args, line)
        return self.make_call_stmt(e, [], line)

    def skip_rest_of_line_ok(self):
        pass

    def expr_starts(self):
        t = self.t
        if t.kind in ("num", "str", "date"):
            return True
        if t.kind == "id":
            return t.low not in ("else", "then", "to", "step", "is", "and", "or", "xor", "eqv", "imp", "mod", "like")
        if t.kind == "op":
            return t.low in ("(", "-", "+", ".", "!", "#", ",")
        return False

    def make_call_stmt(self, e, extra_args, line):
        if e.kind == "call" and not extra_args:
            return Node("callstmt", line, target=e.target, args=e.args)
        if e.kind == "call" and extra_args:
            self.err("call with both parenthesised and bare arguments")
        if e.kind in ("name", "member", "bang"):
            return Node("callstmt", line, target=e, args=extra_args)
        self.err("not a statement")

    def parse_dim_stmt(self):
        line = self.t.line
        static = False
        if self.is_kw("static"):
            static = True
            self.next()
            if self.is_kw("dim"):
                self.next()
        else:
            self.next()
        lst = []
        self.accept_kw("withevents")
        self.parse_var_list("local", lst, static=static)
        return Node("dim", line, decls=lst)

    def parse_redim(self):
        line = self.t.line
        self.next()
        preserve = self.accept_kw("preserve")
        items = []
        while True:
            nt = self.ident()
            target = Node("name", nt.line, name=nt.text, low=nt.text.lower(), tc=nt.tc)
            dims = None
            while True:
                if self.is_op("."):
                    self.next()
                    mt = self.ident()
                    target = Node("member", mt.line, obj=target, name=mt.text, low=mt.text.lower(), withid=None)
                elif self.is_op("("):
                    save = self.p
                    dims = self.parse_dims()
                    if self.is_op("."):
                        self.p = save
                        self.next()
                        args = self.parse_call_args()
                        target = Node("call", nt.line, target=target, args=args)
                        dims = None
                    else:
                        break
                else:
                    break
            if dims is None:
                self.err("ReDim needs bounds")
            ts = self.parse_as_type() if self.is_kw("as") else (self.suffix_type(nt) if nt.tc else None)
            items.append((target, dims, ts))
            if not self.accept_op(","):
                break
        return Node("redim", line, preserve=preserve, items=items)

    def parse_inline_body(self):
        """Statements of a single-line If branch, up to Else / end of line."""
        out = []
        while True:
            while self.is_op(":"):
                self.next()
            if self.t.kind == "nl" or self.is_kw("else"):
                return out
            st = self.parse_statement(False)
            if st is not None:
                out.append(st)
            if not (self.at_stmt_end() or self.is_kw("else")):
                self.err("expected end of statement")

    def parse_if(self):
        line = self.t.line
        self.next()
        cond = self.parse_expr()
        self.expect_kw("then")
        if self.t.kind != "nl":
            # single-line If; a numeric line number or label after Then is a Goto
            if self.t.kind == "num":
                body = [Node("goto", line, label=str(self.next().val))]
            else:
                body = self.parse_inline_body()
            els = None
            if self.accept_kw("else"):
                els = self.parse_inline_body()
            return Node("if", line, clauses=[(cond, body)], els=els)
        clauses = []
        els = None
        body = self.parse_block(("elseif",), ("else",), ("end", "if"))
        clauses.append((cond, body))
        while True:
            if self.is_kw("elseif"):
                self.next()
                c = self.parse_expr()
                self.expect_kw("then")
                b = self.parse_block(("elseif",), ("else",), ("end", "if"))
                clauses.append((c, b))
            elif self.is_kw("else"):
                self.next()
                els = self.parse_block(("end", "if"))
            else:
                self.next()
                self.next()
                break
        return Node("if", line, clauses=clauses, els=els)

    def parse_select(self):
        line = self.t.line
        self.next()
        self.expect_kw("case")
        sel = self.parse_expr()
        self.skip_eol()
        cases = []
        els = None
        while True:
            self.skip_eol()
            if self.is_kw("end") and self.peek().low == "select":
                self.next()
                self.next()
                break
            if not self.is_kw("case"):
                self.err("expected Case")
            self.next()
            if self.is_kw("else"):
                self.next()
                els = self.parse_block(("case",), ("end", "select"))
                continue
            tests = []
            while True:
                if self.is_kw("is"):
                    self.next()
                    op = self.next().low
                    if op in ("<", ">", "=") and self.t.kind == "op" and self.t.low in ("=", ">"):
                        op += self.next().low
                    tests.append(("is", op, self.parse_expr()))
                else:
                    a = self.parse_expr()
                    if self.accept_kw("to"):
                        b = self.parse_expr()
                        tests.append(("range", a, b))
                    else:
                        tests.append(("val", a))
                if not self.accept_op(","):
                    break
            body = self.parse_block(("case",), ("end", "select"))
            cases.append((tests, body))
        return Node("select", line, expr=sel, cases=cases, els=els)

    def parse_for(self):
        line = self.t.line
        self.next()
        if self.is_kw("each"):
            self.next()
            var = self.parse_postfix_expr()
            self.expect_kw("in")
            coll = self.parse_expr()
            body = self.parse_block(("next",))
            self.next()
            if self.t.kind == "id" and self.t.low != "else" and not self.at_stmt_end():
                self.parse_postfix_expr()
            return Node("foreach", line, var=var, coll=coll, body=body)
        var = self.parse_postfix_expr()
        self.expect_op("=")
        start = self.parse_expr()
        self.expect_kw("to")
        end = self.parse_expr()
        step = None
        if self.accept_kw("step"):
            step = self.parse_expr()
        body = self.parse_block(("next",))
        self.next()
        extra = 0
        if not self.at_stmt_end() and not self.is_kw("else"):
            self.parse_postfix_expr()
            while self.accept_op(","):
                self.parse_postfix_expr()
                extra += 1
        node = Node("for", line, var=var, start=start, end=end, step=step, body=body)
        if extra:
            self.err("Next with several variables is not supported")
        return node

    def parse_do(self):
        line = self.t.line
        self.next()
        pre_cond = None
        until = False
        if self.is_kw("while") or self.is_kw("until"):
            until = self.next().low == "until"
            pre_cond = self.parse_expr()
        body = self.parse_block(("loop",))
        self.next()
        if pre_cond is not None:
            return Node("do", line, pre=True, until=until, cond=pre_cond, body=body)
        if self.is_kw("while") or self.is_kw("until"):
            until = self.next().low == "until"
            cond = self.parse_expr()
            return Node("do", line, pre=False, until=until, cond=cond, body=body)
        return Node("do", line, pre=True, until=False, cond=None, body=body)

    def parse_with(self):
        line = self.t.line
        self.next()
        e = self.parse_expr()
        self.with_counter += 1
        wid = self.with_counter
        self.with_stack.append(wid)
        body = self.parse_block(("end", "with"))
        self.with_stack.pop()
        self.next()
        self.next()
        return Node("with", line, expr=e, wid=wid, body=body)

    def parse_on(self):
        line = self.t.line
        self.next()
        if not self.is_kw("error"):
            self.err("only On Error is supported")
        self.next()
        if self.is_kw("resume"):
            self.next()
            self.expect_kw("next")
            return Node("onerror", line, mode="next", label=None)
        self.expect_kw("goto")
        t = self.next()
        if t.kind == "num" and t.val == 0:
            return Node("onerror", line, mode="off", label=None)
        if t.kind == "op" and t.low == "-":
            self.next()
            return Node("onerror", line, mode="off", label=None)
        return Node("onerror", line, mode="goto", label=t.text.lower())

    # ------------------------------------------------------------ expressions
    def parse_expr(self):
        return self.parse_imp()

    def _binary_chain(self, sub, ops):
        left = sub()
        while self.t.kind == "id" and self.t.low in ops:
            t = self.next()
            right = sub()
            left = Node("bin", t.line, op=t.low, l=left, r=right)
        return left

    def parse_imp(self):
        return self._binary_chain(self.parse_eqv, ("imp",))

    def parse_eqv(self):
        return self._binary_chain(self.parse_xor, ("eqv",))

    def parse_xor(self):
        return self._binary_chain(self.parse_or, ("xor",))

    def parse_or(self):
        return self._binary_chain(self.parse_and, ("or",))

    def parse_and(self):
        return self._binary_chain(self.parse_not, ("and",))

    def parse_not(self):
        if self.is_kw("not"):
            t = self.next()
            return Node("un", t.line, op="not", e=self.parse_not())
        return self.parse_cmp()

    def parse_cmp(self):
        left = self.parse_concat()
        while True:
            t = self.t
            if t.kind == "op" and t.low in ("=", "<>", "<", ">", "<=", ">="):
                self.next()
                right = self.parse_concat()
                left = Node("bin", t.line, op=t.low, l=left, r=right)
            elif t.kind == "id" and t.low == "like":
                self.next()
                right = self.parse_concat()
                left = Node("bin", t.line, op="like", l=left, r=right)
            elif t.kind == "id" and t.low == "is":
                self.next()
                if self.is_kw("nothing"):
                    self.next()
                    left = Node("isnothing", t.line, e=left)
                else:
                    right = self.parse_concat()
                    left = Node("bin", t.line, op="is", l=left, r=right)
            else:
                return left

    def parse_concat(self):
        left = self.parse_add()
        while self.is_op("&"):
            t = self.next()
            right = self.parse_add()
            left = Node("bin", t.line, op="&", l=left, r=right)
        return left

    def parse_add(self):
        left = self.parse_mod()
        while self.t.kind == "op" and self.t.low in ("+", "-"):
            t = self.next()
            right = self.parse_mod()
            left = Node("bin", t.line, op=t.low, l=left, r=right)
        return left

    def parse_mod(self):
        left = self.parse_intdiv()
        while self.is_kw("mod"):
            t = self.next()
            right = self.parse_intdiv()
            left = Node("bin", t.line, op="mod", l=left, r=right)
        return left

    def parse_intdiv(self):
        left = self.parse_mul()
        while self.is_op("\\"):
            t = self.next()
            right = self.parse_mul()
            left = Node("bin", t.line, op="\\", l=left, r=right)
        return left

    def parse_mul(self):
        left = self.parse_unary()
        while self.t.kind == "op" and self.t.low in ("*", "/"):
            t = self.next()
            right = self.parse_unary()
            left = Node("bin", t.line, op=t.low, l=left, r=right)
        return left

    def parse_unary(self):
        t = self.t
        if t.kind == "op" and t.low in ("-", "+"):
            self.next()
            e = self.parse_unary()
            if t.low == "+":
                return e
            return Node("un", t.line, op="neg", e=e)
        return self.parse_pow()

    def parse_pow(self):
        left = self.parse_primary()
        while self.is_op("^"):
            t = self.next()
            right = self.parse_pow_operand()
            left = Node("bin", t.line, op="^", l=left, r=right)
        return left

    def parse_pow_operand(self):
        t = self.t
        if t.kind == "op" and t.low in ("-", "+"):
            self.next()
            e = self.parse_pow_operand()
            return Node("un", t.line, op="neg", e=e) if t.low == "-" else e
        return self.parse_primary()

    def parse_primary(self):
        t = self.t
        k = t.kind
        if k == "num":
            self.next()
            return Node("lit", t.line, value=self.num_value(t))
        if k == "str":
            self.next()
            return Node("lit", t.line, value=t.val)
        if k == "date":
            self.err("date literals are not supported")
        if k == "op" and t.low == "(":
            self.next()
            e = self.parse_expr()
            self.expect_op(")")
            return self.postfix(Node("paren", t.line, e=e))
        if k == "id":
            if t.low == "new":
                self.next()
                ts = self.parse_as_type_after_as()
                return Node("new", t.line, typename=ts.name)
            if t.low == "typeof":
                self.next()
                e = self.parse_postfix_expr()
                self.expect_kw("is")
                ts = self.parse_as_type_after_as()
                return Node("typeof", t.line, e=e, typename=ts.name)
            return self.parse_postfix_expr()
        if k == "op" and t.low in (".", "!"):
            return self.parse_postfix_expr()
        self.err("expected expression")

    def num_value(self, t):
        tc, v = t.tc, t.val
        if tc == "integer":
            return V.mk_i16(int(v))
        if tc == "long":
            return V.mk_i32(int(v))
        if tc == "single":
            return V.f32(float(v))
        if tc == "double":
            return float(v)
        if tc == "currency":
            return V.c_cur(float(v)) if isinstance(v, float) else V.mk_cur(int(v) * 10000)
        if isinstance(v, int):
            if -32768 <= v <= 32767:
                return V.I16(v)
            if -2147483648 <= v <= 2147483647:
                return V.I32(v)
            return float(v)
        return float(v)

    def parse_postfix_expr(self):
        t = self.t
        if t.kind == "id":
            self.next()
            if t.low == "me":
                base = Node("me", t.line)
            else:
                base = Node("name", t.line, name=t.text, low=t.low, tc=t.tc)
        elif t.kind == "op" and t.low == ".":
            if not self.with_stack:
                self.err("member access without With")
            base = None
        elif t.kind == "op" and t.low == "!":
            if not self.with_stack:
                self.err("bang access without With")
            base = None
        else:
            self.err("expected expression")
        return self.postfix(base)

    def postfix(self, base):
        while True:
            t = self.t
            if t.kind == "op" and t.low == "(":
                if base is not None and base.kind == "lit":
                    return base
                if self.spaced_paren_stop and t.space_before and base is not None:
                    return base
                self.next()
                args = self.parse_call_args()
                base = Node("call", t.line, target=base, args=args)
            elif t.kind == "op" and t.low == ".":
                self.next()
                mt = self.t
                if mt.kind != "id":
                    self.err("expected member name")
                self.next()
                wid = None
                if base is None:
                    wid = self.with_stack[-1]
                base = Node("member", mt.line, obj=base, name=mt.text, low=mt.text.lower(), withid=wid)
            elif t.kind == "op" and t.low == "!" and base is not None or (
                    t.kind == "op" and t.low == "!" and base is None):
                self.next()
                mt = self.t
                if mt.kind != "id":
                    self.err("expected field name")
                self.next()
                wid = self.with_stack[-1] if base is None else None
                base = Node("bang", mt.line, obj=base, name=mt.text, withid=wid)
            else:
                return base

    def parse_call_args(self):
        """After '(' : arguments up to ')'. Each arg is (name|None, expr|None)."""
        args = []
        if self.accept_op(")"):
            return args
        while True:
            if self.is_op(",") :
                args.append((None, None))
                self.next()
                if self.is_op(")"):
                    args.append((None, None))
                    break
                continue
            name = None
            if self.t.kind == "id" and self.peek().kind == "op" and self.peek().low == ":=":
                name = self.next().low
                self.next()
            args.append((name, self.parse_arg_expr()))
            if not self.accept_op(","):
                break
            if self.is_op(")"):
                args.append((None, None))
                break
        self.expect_op(")")
        return args

    def parse_arg_list_noparen(self):
        args = []
        while True:
            if self.is_op(","):
                args.append((None, None))
                self.next()
                continue
            name = None
            if self.t.kind == "id" and self.peek().kind == "op" and self.peek().low == ":=":
                name = self.next().low
                self.next()
            args.append((name, self.parse_arg_expr()))
            if not self.accept_op(","):
                break
        return args

    def parse_arg_expr(self):
        if self.is_kw("byval") and self.peek().kind != "op":
            t = self.next()
            return Node("paren", t.line, e=self.parse_expr())
        return self.parse_expr()


def strip_class_header(text):
    """Remove the `VERSION 1.0 CLASS` / `BEGIN ... END` designer block of a class module."""
    lines = text.replace("\r\n", "\n").split("\n")
    if lines and lines[0].startswith("VERSION"):
        i = 0
        depth = 0
        while i < len(lines):
            s = lines[i].strip()
            if s == "BEGIN" or s.startswith("BEGIN "):
                depth += 1
            elif s == "END" and depth:
                depth -= 1
                if depth == 0:
                    i += 1
                    break
            i += 1
        return "\n" * i + "\n".join(lines[i:])
    return text


def parse_module(text, name=None, kind="bas"):
    if kind == "cls":
        text = strip_class_header(text)
    p = Parser(text, name, kind)
    return p.parse_module()


def parse_file(path, name=None):
    with open(path, "rb") as f:
        text = decode_source(f.read())
    kind = "cls" if path.lower().endswith(".cls") else "bas"
    m = parse_module(text, name, kind)
    return m

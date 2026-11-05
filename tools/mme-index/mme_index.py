#!/usr/bin/env python3
"""Split MME's VB6 source into one file per procedure plus an index.

Usage:
  python3 -I tools/mme-index/mme_index.py [--mme DIR] [--out DIR]
  python3 -I tools/mme-index/mme_index.py --deps NAME [--out DIR]

The MME sources are Windows-1252; output is UTF-8. Nothing is written inside the repository by
default: the output goes to <cache>/mme-index/<sha>/.
"""
import argparse
import codecs
import json
import os
import re
import shutil
import sys

PIN = "28d23d70caf4f971418aa7ea25471cdb0ab0888a"
EXTS = (".bas", ".cls", ".frm")

PROC_RE = re.compile(
    r"^\s*(?P<mods>(?:(?:Public|Private|Friend|Static)\s+)*)"
    r"(?P<kind>Function|Sub|Property\s+(?:Get|Let|Set))\s+"
    r"(?P<name>[A-Za-z_]\w*)[$%&!#@]?\s*(?P<rest>.*)$",
    re.I,
)
END_RE = re.compile(r"^\s*End\s+(Function|Sub|Property)\b", re.I)
# `Public Function F() As X: F = 1: End Function` and `Resume out: End Sub` end on the same line.
INLINE_END_RE = re.compile(r":\s*End\s+(Function|Sub|Property)\s*$", re.I)
BEGIN_RE = re.compile(r"^\s*Begin\s+([\w.]+)\s+(\w+)")
IDENT_RE = re.compile(r"[A-Za-z_]\w*|\.")
DIM_RE = re.compile(r"^\s*(?:Dim|Static|Const|ReDim(?:\s+Preserve)?)\s+(.*)$", re.I)
VAR_RE = re.compile(r"^\s*(?P<vis>Public|Global|Private|Dim)\s+(?P<rest>.*)$", re.I)
CONST_RE = re.compile(r"^\s*(?:Public|Global|Private)?\s*Const\s+(.*)$", re.I)


def _latin1_fallback(err):
    return ("".join(chr(b) for b in err.object[err.start:err.end]), err.end)


codecs.register_error("touchstone_latin1", _latin1_fallback)


def decode(data):
    try:
        text = data.decode("cp1252")
    except UnicodeDecodeError:
        # cp1252 leaves five bytes undefined; keep them as the matching Latin-1 characters.
        text = data.decode("cp1252", errors="touchstone_latin1")
    return re.split(r"\r\n|\r|\n", text)


def scrub(line):
    """Drop comments and blank string contents so token scans see only code."""
    out = []
    in_str = False
    i = 0
    n = len(line)
    while i < n:
        c = line[i]
        if in_str:
            if c == '"':
                if i + 1 < n and line[i + 1] == '"':
                    out.append("  ")
                    i += 2
                    continue
                in_str = False
                out.append(c)
            else:
                out.append(" ")
        elif c == '"':
            in_str = True
            out.append(c)
        elif c == "'":
            break
        else:
            out.append(c)
        i += 1
    code = "".join(out)
    if re.match(r"^\s*Rem\b", code, re.I):
        return ""
    return code


def split_top(s):
    """Split on commas outside parentheses."""
    parts, depth, cur = [], 0, []
    for c in s:
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        if c == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(c)
    parts.append("".join(cur))
    return [p.strip() for p in parts if p.strip()]


def logical_lines(lines, first=0):
    """Yield (start, end, text, code) with `_` continuations joined; start/end are 1-based."""
    i = first
    n = len(lines)
    while i < n:
        start = i
        text = lines[i]
        code = scrub(lines[i])
        while re.search(r"(^|\s)_\s*$", code) and i + 1 < n:
            i += 1
            code = re.sub(r"\s*_\s*$", " ", code) + scrub(lines[i]).lstrip()
            text = text + " " + lines[i].strip()
        yield start + 1, i + 1, text, code
        i += 1


def parse_params(rest):
    """Return (params, return_type) from the text after the procedure name."""
    params, ret = [], None
    m = re.match(r"\((.*)\)\s*(?:As\s+(.+?))?\s*$", rest.strip(), re.I)
    if m:
        for p in split_top(m.group(1)):
            pm = re.match(
                r"(?P<opt>Optional\s+)?(?P<pass>ByVal\s+|ByRef\s+)?(?P<pa>ParamArray\s+)?"
                r"(?P<name>[A-Za-z_]\w*)[$%&!#@]?(?P<arr>\(\s*\))?\s*(?:As\s+(?P<type>[\w.]+))?"
                r"\s*(?:=\s*(?P<def>.*))?$",
                p,
                re.I,
            )
            if not pm:
                params.append({"name": p, "type": None, "byval": False, "optional": False})
                continue
            params.append(
                {
                    "name": pm.group("name"),
                    "type": (pm.group("type") or "Variant") + ("()" if pm.group("arr") else ""),
                    "byval": bool(pm.group("pass") and pm.group("pass").strip().lower() == "byval"),
                    "optional": bool(pm.group("opt")),
                }
            )
        ret = m.group(2).strip() if m.group(2) else None
    return params, ret


class Module:
    def __init__(self, name, kind, path):
        self.name = name
        self.kind = kind  # bas | cls | frm
        self.path = path
        self.lines = []
        self.procs = []
        self.decl_lines = []  # (lineno, text)
        self.decl_code = []  # scrubbed logical decl lines
        self.controls = {}  # lower -> (name, type)
        self.vars = {}  # lower -> (name, type, public)
        self.warnings = []


def parse_module(name, kind, lines, path=""):
    mod = Module(name, kind, path)
    mod.lines = lines
    n = len(lines)

    # Header: designer block (frm) or VERSION/BEGIN block (cls), ends before Attribute VB_Name.
    code_start = 0
    depth = 0
    for i, ln in enumerate(lines):
        if re.match(r"^\s*Attribute\s+VB_Name\b", ln, re.I):
            code_start = i
            vm = re.match(r'^\s*Attribute\s+VB_Name\s*=\s*"([^"]+)"', ln, re.I)
            if vm:
                # Code refers to modules by VB_Name, which can differ from the file name.
                mod.name = vm.group(1)
            break
        bm = BEGIN_RE.match(ln)
        if bm and kind == "frm":
            if depth >= 1:
                mod.controls[bm.group(2).lower()] = (bm.group(2), bm.group(1))
            depth += 1
        elif depth > 0 and re.match(r"^\s*End\s*$", ln, re.I):
            depth -= 1
    # skip Attribute lines
    i = code_start
    while i < n and re.match(r"^\s*Attribute\s", lines[i], re.I):
        i += 1
    body_start = i

    cur = None
    for start, end, text, code in logical_lines(lines, body_start):
        if cur is None:
            m = PROC_RE.match(code)
            if m and not re.match(r"^\s*(Public\s+|Private\s+|Global\s+)?Declare\b", code, re.I):
                mods = m.group("mods").lower().split()
                vis = next((v.capitalize() for v in ("public", "private", "friend") if v in mods), "Public")
                kindname = re.sub(r"\s+", " ", m.group("kind")).title()
                params, ret = parse_params(m.group("rest"))
                cur = {
                    "module": mod.name,
                    "name": m.group("name"),
                    "kind": kindname,
                    "visibility": vis,
                    "static": "static" in mods,
                    "params": params,
                    "returns": ret,
                    "start": start,
                    "signature": re.sub(r"\s+", " ", text.strip()),
                    "body": [],
                }
                if cur["kind"] == "Sub":
                    cur["returns"] = None
                if INLINE_END_RE.search(code):
                    cur["end"] = end
                    mod.procs.append(cur)
                    cur = None
                continue
            mod.decl_lines.append((start, end))
            mod.decl_code.append(code)
        else:
            em = END_RE.match(code) or INLINE_END_RE.search(code)
            if em:
                cur["end"] = end
                mod.procs.append(cur)
                cur = None
                continue
            pm = PROC_RE.match(code)
            if pm and not re.match(r"^\s*(Exit|End)\b", code, re.I) and not re.match(
                r"^\s*(Public\s+|Private\s+|Global\s+)?Declare\b", code, re.I
            ) and re.match(r"^\s*(Public|Private|Friend|Static|Function|Sub|Property)\b", code, re.I):
                mod.warnings.append(
                    "%s: procedure %s starts at line %d inside %s" % (name, pm.group("name"), start, cur["name"])
                )
            cur["body"].append(code)
    if cur is not None:
        mod.warnings.append("%s: procedure %s (line %d) has no End" % (name, cur["name"], cur["start"]))
        cur["end"] = n
        mod.procs.append(cur)

    # Stray End Sub outside a procedure shows up as a declaration line.
    for code in mod.decl_code:
        if END_RE.match(code):
            mod.warnings.append("%s: End %s outside a procedure" % (name, END_RE.match(code).group(1)))

    # Module-level variables (for global/table detection).
    for code in mod.decl_code:
        cm = CONST_RE.match(code)
        if cm:
            vis = "public" if re.match(r"^\s*(Public|Global)\b", code, re.I) or not re.match(r"^\s*Private\b", code, re.I) else "private"
            for part in split_top(cm.group(1)):
                nm = re.match(r"([A-Za-z_]\w*)", part)
                if nm:
                    mod.vars[nm.group(1).lower()] = (nm.group(1), "Const", vis == "public")
            continue
        vm = VAR_RE.match(code)
        if vm and not re.match(r"^\s*(Public|Global|Private)\s+(Declare|Type|Enum|Sub|Function|Property|Const)\b", code, re.I):
            public = vm.group("vis").lower() in ("public", "global")
            rest = re.sub(r"^\s*WithEvents\s+", "", vm.group("rest"), flags=re.I)
            for part in split_top(rest):
                nm = re.match(r"(?:WithEvents\s+)?([A-Za-z_]\w*)[$%&!#@]?(?:\s*\(.*?\))?\s*(?:As\s+(?:New\s+)?([\w.]+))?", part, re.I)
                if nm:
                    mod.vars[nm.group(1).lower()] = (nm.group(1), nm.group(2) or "Variant", public)
    for p in mod.procs:
        p["module"] = mod.name
    return mod


def local_names(proc):
    names = {p["name"].lower() for p in proc["params"]}
    for code in proc["body"]:
        m = DIM_RE.match(code)
        if m:
            for part in split_top(m.group(1)):
                nm = re.match(r"([A-Za-z_]\w*)", part)
                if nm:
                    names.add(nm.group(1).lower())
    return names


def proc_id(p):
    if p["kind"].startswith("Property"):
        return "%s.%s.%s" % (p["module"], p["name"], p["kind"].split()[1].lower())
    return "%s.%s" % (p["module"], p["name"])


TOKEN_RE = re.compile(r"[A-Za-z_]\w*|\.")


def is_table(typ):
    return typ.lower() in ("recordset", "dao.recordset")


class Resolver:
    """Name tables shared by every procedure scan."""

    def __init__(self, modules):
        self.mods = {m.name.lower(): m for m in modules}
        self.own = {}  # module name -> lower proc name -> [ids]
        self.global_procs = {}  # lower name -> [ids], public procs of .bas modules
        self.global_vars = {}  # lower name -> [(module, name, type)]
        self.warnings = []
        self.ids = {}
        for m in modules:
            for p in m.procs:
                pid = proc_id(p)
                if pid in self.ids:
                    k = 2
                    while "%s~%d" % (pid, k) in self.ids:
                        k += 1
                    self.warnings.append("%s: duplicate procedure %s (kept as %s~%d)" % (m.name, p["name"], pid, k))
                    pid = "%s~%d" % (pid, k)
                p["id"] = pid
                self.ids[pid] = p
                self.own.setdefault(m.name, {}).setdefault(p["name"].lower(), []).append(pid)
                if m.kind == "bas" and p["visibility"] != "Private":
                    self.global_procs.setdefault(p["name"].lower(), []).append(pid)
            if m.kind == "bas":
                for low, (nm, typ, public) in m.vars.items():
                    if public:
                        self.global_vars.setdefault(low, []).append((m.name, nm, typ))


def scan_proc(res, m, p):
    locs = local_names(p)
    calls, tables, forms, globs = set(), set(), set(), set()
    with_stack = []

    def member(target, ident):
        low = ident.lower()
        if low in res.own.get(target.name, {}):
            calls.update(res.own[target.name][low])
        elif low in target.controls:
            forms.add("%s.%s" % (target.name, target.controls[low][0]))
        elif target.kind == "bas" and low in target.vars and target.vars[low][2]:
            nm, typ, _ = target.vars[low]
            if is_table(typ):
                tables.add(nm)
            else:
                globs.add("%s.%s" % (target.name, nm))

    for code in p["body"]:
        wm = re.match(r"^\s*With\s+([A-Za-z_]\w*)\s*$", code, re.I)
        if wm:
            q = wm.group(1).lower()
            with_stack.append(m if q == "me" else (res.mods.get(q) if q not in locs else None))
            continue
        if re.match(r"^\s*With\b", code, re.I):
            with_stack.append(None)
            continue
        if re.match(r"^\s*End\s+With\b", code, re.I):
            if with_stack:
                with_stack.pop()
            continue
        toks = [(t.group(0), t.start(), t.end()) for t in TOKEN_RE.finditer(code)]
        is_member = set()
        for j, (tok, s, e) in enumerate(toks):
            if tok == "." and j + 1 < len(toks) and toks[j + 1][0] != ".":
                is_member.add(j + 1)
                target = None
                before = code[:s].rstrip()
                if before.endswith(")"):
                    target = None
                elif before and (before[-1].isalnum() or before[-1] == "_"):
                    # qualified by an identifier; chained members (a.b.c) are not followed
                    if j > 0 and toks[j - 1][0] != "." and (j - 1) not in is_member:
                        ql = toks[j - 1][0].lower()
                        if ql == "me":
                            target = m
                        elif ql not in locs:
                            target = res.mods.get(ql)
                else:
                    target = with_stack[-1] if with_stack else None
                if target is not None:
                    member(target, toks[j + 1][0])
        for j, (tok, s, e) in enumerate(toks):
            if tok == "." or j in is_member:
                continue
            low = tok.lower()
            if low in locs:
                continue
            if low in res.own.get(m.name, {}):
                calls.update(res.own[m.name][low])
            elif low in m.controls:
                forms.add("%s.%s" % (m.name, m.controls[low][0]))
            elif low in res.global_procs:
                calls.update(res.global_procs[low])
            elif low in res.global_vars:
                for gm, nm, typ in res.global_vars[low]:
                    if gm == m.name:
                        continue
                    if is_table(typ):
                        tables.add(nm)
                    else:
                        globs.add("%s.%s" % (gm, nm))
    calls.discard(p["id"])
    p["calls"] = sorted(calls)
    p["tables"] = sorted(tables)
    p["forms"] = sorted(forms)
    p["globals"] = sorted(globs)


def build_index(modules):
    res = Resolver(modules)
    for m in modules:
        for p in m.procs:
            scan_proc(res, m, p)
    warnings = [w for m in modules for w in m.warnings] + res.warnings
    return res.ids, warnings


def load_modules(mme_dir):
    mods = []
    for fn in sorted(os.listdir(mme_dir)):
        stem, ext = os.path.splitext(fn)
        if ext.lower() not in EXTS:
            continue
        with open(os.path.join(mme_dir, fn), "rb") as f:
            lines = decode(f.read())
        mods.append(parse_module(stem, ext.lower()[1:], lines, fn))
    return mods


def index_record(p):
    return {
        "id": p["id"],
        "module": p["module"],
        "name": p["name"],
        "kind": p["kind"],
        "visibility": p["visibility"],
        "static": p["static"],
        "params": p["params"],
        "returns": p["returns"],
        "start": p["start"],
        "end": p["end"],
        "lines": p["end"] - p["start"] + 1,
        "signature": p["signature"],
        "file": "%s/%s.vb" % (p["module"], file_stem(p)),
        "calls": p["calls"],
        "tables": p["tables"],
        "forms": p["forms"],
        "globals": p["globals"],
    }


def file_stem(p):
    stem = p["name"]
    if p["kind"].startswith("Property"):
        stem += "." + p["kind"].split()[1].lower()
    if "~" in p["id"]:
        stem += "~" + p["id"].split("~")[1]
    return stem


def write_output(modules, ids, warnings, out, pin):
    if os.path.isdir(out) and os.listdir(out) and not os.path.exists(os.path.join(out, "index.json")):
        sys.exit("refusing to overwrite non-index directory %s" % out)
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(out)
    index = {"pin": pin, "modules": {}, "procedures": [], "warnings": warnings}
    md = ["# MME procedure index (pin %s)" % pin, ""]
    for m in modules:
        d = os.path.join(out, m.name)
        os.makedirs(d)
        with open(os.path.join(d, "_declarations.vb"), "w", encoding="utf-8", newline="\n") as f:
            f.write("' module %s: module-level declarations\n" % m.name)
            for (s, e) in m.decl_lines:
                for k in range(s - 1, e):
                    f.write(m.lines[k] + "\n")
        ctl = sorted(m.controls.values(), key=lambda c: c[0].lower())
        total = sum(p["end"] - p["start"] + 1 for p in m.procs)
        index["modules"][m.name] = {
            "file": m.path,
            "kind": m.kind,
            "file_lines": len(m.lines),
            "procedures": len(m.procs),
            "procedure_lines": total,
            "controls": [{"name": n, "type": t} for n, t in ctl],
        }
        md.append("## %s (%s, %d procedures, %d procedure lines, %d file lines)" % (m.name, m.kind, len(m.procs), total, len(m.lines)))
        md.append("")
        for p in m.procs:
            with open(os.path.join(d, file_stem(p) + ".vb"), "w", encoding="utf-8", newline="\n") as f:
                f.write("' module %s, lines %d-%d: %s\n" % (m.name, p["start"], p["end"], p["signature"]))
                for k in range(p["start"] - 1, p["end"]):
                    f.write(m.lines[k] + "\n")
            rec = index_record(p)
            index["procedures"].append(rec)
            md.append("- %s | %s | %d | %s" % (p["name"], p["kind"], rec["lines"], p["signature"]))
        md.append("")
    with open(os.path.join(out, "index.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(index, f, indent=1)
        f.write("\n")
    with open(os.path.join(out, "index.md"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(md) + "\n")
    return index


def deps(index, name):
    procs = {p["id"]: p for p in index["procedures"]}
    cands = [i for i in procs if i.lower() == name.lower()]
    if not cands:
        cands = [i for i, p in procs.items() if p["name"].lower() == name.lower()]
    if not cands:
        sys.exit("no procedure named %s" % name)
    if len(cands) > 1:
        sys.exit("ambiguous, use one of: " + ", ".join(sorted(cands)))
    seen, stack = [], [cands[0]]
    while stack:
        cur = stack.pop()
        if cur in seen:
            continue
        seen.append(cur)
        stack.extend(reversed(procs[cur]["calls"]))
    total = 0
    for i in seen:
        print("%-60s %6d" % (i, procs[i]["lines"]))
        total += procs[i]["lines"]
    print("%-60s %6d  (%d procedures)" % ("TOTAL", total, len(seen)))


def default_cache():
    return os.environ.get("TOUCHSTONE_CACHE") or os.path.join(os.path.expanduser("~"), ".cache", "touchstone")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sha = os.environ.get("MME_SHA", PIN)
    cache = default_cache()
    ap.add_argument("--mme", default=os.path.join(cache, "mme", sha))
    ap.add_argument("--out", default=os.path.join(cache, "mme-index", sha))
    ap.add_argument("--deps", metavar="NAME")
    a = ap.parse_args(argv)
    if a.deps:
        with open(os.path.join(a.out, "index.json"), encoding="utf-8") as f:
            deps(json.load(f), a.deps)
        return 0
    if not os.path.isdir(a.mme):
        sys.exit("MME source not found at %s; run tools/mme-fetch.sh" % a.mme)
    modules = load_modules(a.mme)
    ids, warnings = build_index(modules)
    index = write_output(modules, ids, warnings, a.out, sha)
    print("%d modules, %d procedures, %d warnings -> %s" % (len(modules), len(index["procedures"]), len(warnings), a.out))
    for w in warnings:
        print("WARNING: " + w, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())

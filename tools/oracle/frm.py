"""Pieces of a form module (frmMain.frm) for the oracle.

frmMain is 43k lines of event code and Win32 plumbing the interpreter cannot parse as a whole, and its
formulas are procedures of the form itself (CalcCharacterStats and friends). A scenario names the
procedures and module-level variables it needs; `build_source` cuts exactly those lines out of the
form's code section (MME's text, unchanged) and returns one module. Controls the code names without
`frmMain.` resolve to the frmMain shim (`control_names`, `ControlProxy`).

A procedure can be cut short: `{"cut": "<line>", "append": ["<vb line>", ...]}` drops everything from
the first line equal to `cut` (after trimming) up to the procedure's end and puts the appended lines in
its place. That is how a scenario ends CalcCharacterStats where a later card takes over, and how it
copies a local the form never stores (nEncumPCT) into a probe control. After the appended lines comes
`Exit Sub`, an error label that reports through HandleError, and `End Sub`; a cut may give its own
`"tail"` lines instead (the pipeline family keeps the `out:` section of CalcCharacterStats that way).

`{"subs": {proc: {"<line>": "<vb line>"}}}` replaces whole lines of a procedure (matched after trimming) with
other text, e.g. an InputBox call with a read of a control; a line that is not found is an error. Only the
first matching line is replaced; `"subs_all"` has the same shape and replaces every matching line. The
replacement may hold several lines (joined with newlines).
"""
import re

from vbi.parser import decode_source


def _code_lines(raw):
    lines = decode_source(raw).split("\n")
    start = next(i for i, l in enumerate(lines) if l.startswith("Attribute VB_Name"))
    return lines[:start], lines[start:]


def control_names(raw):
    """Names of the form's controls (`Begin VB.Label lblFoo`), from the designer block."""
    head, _ = _code_lines(raw)
    return sorted(set(re.findall(r"^\s*Begin\s+\S+\s+(\w+)", "\n".join(head), re.M)))


def _proc_lines(code, name):
    pat = re.compile(r"^(Public |Private |Friend )?(Static )?(Sub|Function) %s\(" % re.escape(name), re.I)
    for i, line in enumerate(code):
        if pat.match(line):
            kind = pat.match(line).group(3)
            for j in range(i, len(code)):
                if re.match(r"^End %s\s*$" % kind, code[j].rstrip("\r"), re.I):
                    return [l.rstrip("\r") for l in code[i:j + 1]]
            raise ValueError("no End %s for %s" % (kind, name))
    raise ValueError("procedure %s not found in the form" % name)


def _decl_line(code, name):
    pat = re.compile(r"^(Public|Private|Dim)\s+%s\b" % re.escape(name), re.I)
    for line in code:
        if pat.match(line):
            return pat.sub("Public " + name, line.rstrip("\r"), count=1)
    raise ValueError("declaration of %s not found in the form" % name)


def _enum_lines(code, name):
    pat = re.compile(r"^(Public |Private )?Enum %s\s*$" % re.escape(name), re.I)
    for i, line in enumerate(code):
        if pat.match(line.rstrip("\r")):
            for j in range(i, len(code)):
                if re.match(r"^End Enum\s*$", code[j].rstrip("\r"), re.I):
                    return [l.rstrip("\r") for l in code[i:j + 1]]
    raise ValueError("enum %s not found in the form" % name)


def build_source(raw, spec):
    """spec: {"decls": [name], "enums": [name], "procs": [name], "cuts": {proc: {"cut": line, "append": [lines]}}, "subs": {proc: {line: new}}}"""
    _, code = _code_lines(raw)
    out = ["Option Explicit", "Option Base 0"]
    for n in spec.get("enums", []):
        out += _enum_lines(code, n)
    out += [_decl_line(code, n) for n in spec.get("decls", [])]
    out += spec.get("extra", [])        # declarations of a module the scenario does not load (list-sort enum)
    for name in spec["procs"]:
        body = _proc_lines(code, name)
        for old, new in ((spec.get("subs") or {}).get(name) or {}).items():
            idx = next(i for i, l in enumerate(body) if l.strip() == old)
            body[idx] = new
        for old, new in ((spec.get("subs_all") or {}).get(name) or {}).items():
            hits = [i for i, l in enumerate(body) if l.strip() == old]
            if not hits:
                raise ValueError("line %r not found in %s" % (old, name))
            for idx in hits:
                body[idx] = new
        cut = (spec.get("cuts") or {}).get(name)
        if cut:
            idx = next(i for i, l in enumerate(body) if l.strip() == cut["cut"])
            default_tail = ["Exit Sub", "error:", 'Call HandleError("%s")' % name, "End Sub"]
            body = body[:idx] + list(cut.get("append", [])) + list(cut.get("tail", default_tail))
        if spec.get("me_is_form"):
            # `Me.hWnd`, `Me.Enabled`: in a module `Me` is nothing, so name the form (its shim takes any property)
            body = [re.sub(r"\bMe\.", "frmMain.", l) for l in body]
        out += body
    return "\n".join(out) + "\n"

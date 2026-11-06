"""Pieces of a form module (frmMain.frm) for the oracle.

frmMain is 43k lines of event code and Win32 plumbing the interpreter cannot parse as a whole, and its
formulas are procedures of the form itself (CalcCharacterStats and friends). A scenario names the
procedures and module-level variables it needs; `build_source` cuts exactly those lines out of the
form's code section (MME's text, unchanged) and returns one module. Controls the code names without
`frmMain.` resolve to the frmMain shim (`control_names`, `ControlProxy`).

A procedure can be cut short: `{"cut": "<line>", "append": ["<vb line>", ...]}` drops everything from
the first line equal to `cut` (after trimming) up to the procedure's end and puts the appended lines in
its place. That is how a scenario ends CalcCharacterStats where a later card takes over, and how it
copies a local the form never stores (nEncumPCT) into a probe control.

`{"subs": {proc: {"<line>": "<vb line>"}}}` replaces whole lines of a procedure (matched after trimming) with
other text, e.g. an InputBox call with a read of a control; a line that is not found is an error.
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


def build_source(raw, spec):
    """spec: {"decls": [name], "procs": [name], "cuts": {proc: {"cut": line, "append": [lines]}}, "subs": {proc: {line: new}}}"""
    _, code = _code_lines(raw)
    out = ["Option Explicit", "Option Base 0"]
    out += [_decl_line(code, n) for n in spec.get("decls", [])]
    for name in spec["procs"]:
        body = _proc_lines(code, name)
        for old, new in ((spec.get("subs") or {}).get(name) or {}).items():
            idx = next(i for i, l in enumerate(body) if l.strip() == old)
            body[idx] = new
        cut = (spec.get("cuts") or {}).get(name)
        if cut:
            idx = next(i for i, l in enumerate(body) if l.strip() == cut["cut"])
            body = body[:idx] + list(cut.get("append", [])) + ["Exit Sub", "error:", 'Call HandleError("%s")' % name, "End Sub"]
        out += body
    return "\n".join(out) + "\n"

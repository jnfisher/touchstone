"""Tests over small invented VB snippets (not MME code)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import mme_index as mi  # noqa: E402


def lines(text):
    return text.replace("\r\n", "\n").split("\n")


BAS = """Attribute VB_Name = "modDemo"
Option Explicit
Public gCounter As Long
Global gName As String, gList(1 To 3) As Integer
Public tabThings As Recordset
Private m_hidden As Long
Public Const LIMIT = 5
Declare Function Beep Lib "kernel32" (ByVal x As Long) As Long

Public Function Add(ByVal a As Long, _
                    Optional ByRef b As Long = 2) As Long
    Add = a + b: gCounter = gCounter + 1
End Function

Private Sub Helper()
    Dim x As Long
    x = Add(1, 2)  ' Add in a comment: Missing
    Call Other
End Sub

Sub Other()
    Dim gCounter As Long   ' local shadows the global
    gCounter = 1
    s = "Helper( not a call )"
    Exit Sub
End Sub

Public Function One() As Long: One = 1: End Function

Static Function Counter() As Long
    Counter = tabThings.RecordCount
    With frmDemo
        .cmdGo.Caption = "x"
        .Refresh2
    End With
End Function
"""

CLS = """VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "clsThing"
Option Explicit
Private m_v As Long

Public Property Get Value() As Long
    Value = m_v
End Property

Public Property Let Value(ByVal v As Long)
    m_v = v
End Property

Public Property Set Obj(ByVal o As Object)
End Property
"""

FRM = """VERSION 5.00
Begin VB.Form frmDemo
   Caption         =   "Demo"
   Begin VB.Frame fraBox
      Begin VB.CommandButton cmdGo
         Caption         =   "Go"
         BeginProperty Font
            Name            =   "Tahoma"
         EndProperty
      End
   End
   Begin VB.Menu mnuFile
      Caption         =   "&File"
   End
End
Attribute VB_Name = "frmDemo"
Attribute VB_PredeclaredId = True
Option Explicit

Private Sub cmdGo_Click()
    Call Refresh2
    fraBox.Visible = False
    modDemo.Add 1, 2
    gName = "caf\xe9 \x97 dash"
End Sub

Public Sub Refresh2()
    Call Helper
End Sub
"""


def build(*specs):
    mods = []
    for name, kind, text in specs:
        mods.append(mi.parse_module(name, kind, lines(text)))
    ids, warnings = mi.build_index(mods)
    return mods, ids, warnings


class ParseTests(unittest.TestCase):
    def setUp(self):
        self.mods, self.ids, self.warnings = build(
            ("modDemo", "bas", BAS), ("clsThing", "cls", CLS), ("frmDemo", "frm", FRM)
        )

    def test_no_warnings(self):
        self.assertEqual(self.warnings, [])

    def test_procedure_names_and_kinds(self):
        self.assertEqual(
            sorted(self.ids),
            sorted([
                "modDemo.Add", "modDemo.Helper", "modDemo.Other", "modDemo.One", "modDemo.Counter",
                "clsThing.Value.get", "clsThing.Value.let", "clsThing.Obj.set",
                "frmDemo.cmdGo_Click", "frmDemo.Refresh2",
            ]),
        )
        self.assertEqual(self.ids["clsThing.Value.get"]["kind"], "Property Get")
        self.assertEqual(self.ids["clsThing.Obj.set"]["kind"], "Property Set")
        self.assertTrue(self.ids["modDemo.Counter"]["static"])
        self.assertEqual(self.ids["modDemo.Helper"]["visibility"], "Private")
        self.assertEqual(self.ids["modDemo.Other"]["visibility"], "Public")

    def test_continuation_signature_and_range(self):
        add = self.ids["modDemo.Add"]
        self.assertEqual(add["start"], 10)
        self.assertEqual(add["end"], 13)
        self.assertEqual(add["returns"], "Long")
        self.assertEqual([p["name"] for p in add["params"]], ["a", "b"])
        self.assertTrue(add["params"][0]["byval"])
        self.assertTrue(add["params"][1]["optional"])
        self.assertIn("Optional ByRef b As Long = 2", add["signature"])

    def test_inline_end(self):
        one = self.ids["modDemo.One"]
        self.assertEqual(one["start"], one["end"])
        # the procedure after the one-liner must still be found
        self.assertIn("modDemo.Counter", self.ids)

    def test_calls_ignore_comments_strings_and_self(self):
        helper = self.ids["modDemo.Helper"]
        self.assertEqual(helper["calls"], ["modDemo.Add", "modDemo.Other"])
        self.assertNotIn("modDemo.Helper", self.ids["modDemo.Helper"]["calls"])

    def test_cross_module_calls_and_with(self):
        self.assertEqual(self.ids["frmDemo.cmdGo_Click"]["calls"], ["frmDemo.Refresh2", "modDemo.Add"])
        # Helper is Private to modDemo, so a form cannot be calling it.
        self.assertEqual(self.ids["frmDemo.Refresh2"]["calls"], [])
        counter = self.ids["modDemo.Counter"]
        self.assertEqual(counter["tables"], [])  # same module: not "other"
        self.assertEqual(counter["forms"], ["frmDemo.cmdGo"])
        self.assertEqual(counter["calls"], ["frmDemo.Refresh2"])

    def test_globals_tables_forms_from_other_modules(self):
        click = self.ids["frmDemo.cmdGo_Click"]
        self.assertEqual(click["globals"], ["modDemo.gName"])
        self.assertEqual(click["forms"], ["frmDemo.fraBox"])
        # Local variable shadows the global.
        self.assertEqual(self.ids["modDemo.Other"]["globals"], [])

    def test_tables_from_other_module(self):
        src = 'Attribute VB_Name = "modUse"\nSub U()\n    tabThings.MoveFirst\nEnd Sub\n'
        mods, ids, w = build(("modDemo", "bas", BAS), ("modUse", "bas", src))
        self.assertEqual(ids["modUse.U"]["tables"], ["tabThings"])
        self.assertEqual(w, [])

    def test_designer_controls_skip_properties(self):
        frm = next(m for m in self.mods if m.name == "frmDemo")
        names = sorted(c[0] for c in frm.controls.values())
        self.assertEqual(names, ["cmdGo", "fraBox", "mnuFile"])
        self.assertEqual(frm.controls["cmdgo"][1], "VB.CommandButton")

    def test_declarations_exclude_procedures(self):
        bas = next(m for m in self.mods if m.name == "modDemo")
        text = "\n".join(bas.lines[s - 1] for s, _ in bas.decl_lines)
        self.assertIn("Declare Function Beep", text)
        self.assertIn("Public Const LIMIT", text)
        self.assertNotIn("End Function", text)
        self.assertIn("gname", bas.vars)
        self.assertEqual(bas.vars["gname"][1], "String")
        self.assertEqual(bas.vars["glist"][1], "Integer")
        self.assertFalse(bas.vars["m_hidden"][2])

    def test_unterminated_procedure_warns(self):
        _, _, w = build(("modBad", "bas", 'Attribute VB_Name = "modBad"\nSub A()\n x = 1\n'))
        self.assertEqual(len(w), 1)
        self.assertIn("no End", w[0])


class CodecTests(unittest.TestCase):
    def test_cp1252_decoding(self):
        data = "' café — “q”\r\nSub A()\r\nEnd Sub\r\n".encode("cp1252")
        got = mi.decode(data)
        self.assertEqual(got[0], "' café — “q”")
        self.assertEqual(got[1], "Sub A()")

    def test_undefined_cp1252_byte_does_not_fail(self):
        got = mi.decode(b"' a\x81b \x97\r\n")
        self.assertEqual(got[0], "' a\x81b —")

    def test_crlf_only_line_split(self):
        # Form feed / vertical tab inside a line must not split it.
        got = mi.decode(b"a\x0cb\r\nc")
        self.assertEqual(got, ["a\x0cb", "c"])


class OutputTests(unittest.TestCase):
    def test_write_and_deps(self):
        import io
        import json
        import tempfile
        from contextlib import redirect_stdout

        mods, ids, w = build(("modDemo", "bas", BAS), ("clsThing", "cls", CLS), ("frmDemo", "frm", FRM))
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "o")
            idx = mi.write_output(mods, ids, w, out, "testpin")
            self.assertTrue(os.path.isfile(os.path.join(out, "modDemo", "_declarations.vb")))
            self.assertTrue(os.path.isfile(os.path.join(out, "clsThing", "Value.get.vb")))
            with open(os.path.join(out, "modDemo", "Add.vb"), encoding="utf-8") as f:
                first = f.readline()
            self.assertEqual(first.split(":")[0], "' module modDemo, lines 10-13")
            with open(os.path.join(out, "frmDemo", "cmdGo_Click.vb"), encoding="utf-8") as f:
                self.assertIn("Call Refresh2", f.read())
            self.assertEqual(idx["modules"]["frmDemo"]["procedures"], 2)
            self.assertTrue(os.path.isfile(os.path.join(out, "index.md")))
            buf = io.StringIO()
            with open(os.path.join(out, "index.json"), encoding="utf-8") as f:
                loaded = json.load(f)
            with redirect_stdout(buf):
                mi.deps(loaded, "modDemo.Helper")
            text = buf.getvalue()
            self.assertIn("modDemo.Add", text)
            self.assertIn("modDemo.Other", text)
            self.assertIn("TOTAL", text)
            # re-running over an existing index directory replaces it
            mi.write_output(mods, ids, w, out, "testpin")


if __name__ == "__main__":
    unittest.main()

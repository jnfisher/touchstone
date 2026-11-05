"""Small invented VB programs, one per language construct (no MME code)."""
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from vbi.interp import Interpreter  # noqa: E402
from vbi.errors import VBError, OracleUnbound, VBSyntaxError  # noqa: E402
from vbi.lexer import lex  # noqa: E402
from vbi.parser import parse_module  # noqa: E402
from vbi.objects import ExtObject  # noqa: E402
from vbi import values as V  # noqa: E402


def run(src, name, *args, kind="bas", setup=None):
    it = Interpreter()
    it.load_source('Attribute VB_Name = "modT"\nOption Explicit\n' + src, "modT", kind)
    if setup:
        setup(it)
    return it.call(name, *args), it


def val(src, name, *args, **kw):
    r, _ = run(src, name, *args, **kw)
    return r


class LexerTests(unittest.TestCase):
    def kinds(self, text):
        return [(t.kind, t.text) for t in lex(text) if t.kind != "nl"]

    def test_continuation_comment_and_separator(self):
        toks = self.kinds("x = 1 + _\n  2 ' note\ny = 3: z = 4")
        self.assertEqual([t[1] for t in toks], ["x", "=", "1", "+", "2", "y", "=", "3", ":", "z", "=", "4"])

    def test_comment_continuation(self):
        toks = self.kinds("a = 1 ' comment _\n   still comment\nb = 2")
        self.assertEqual([t[1] for t in toks], ["a", "=", "1", "b", "=", "2"])

    def test_string_with_doubled_quote(self):
        t = [x for x in lex('s = "a""b"') if x.kind == "str"][0]
        self.assertEqual(t.val, 'a"b')

    def test_numeric_literals(self):
        nums = {t.text: (t.val, t.tc) for t in lex("x = &HFF + &H8000 + &HFFFF& + 5# + 1.5E3 + 7% + 100000") if t.kind == "num"}
        self.assertEqual(nums["&HFF"], (255, "integer"))
        self.assertEqual(nums["&H8000"], (-32768, "integer"))
        self.assertEqual(nums["&HFFFF&"], (65535, "long"))
        self.assertEqual(nums["5"], (5, "double"))
        self.assertEqual(nums["1.5E3"], (1500.0, "double"))

    def test_type_suffix_and_bang(self):
        toks = lex("a = Left$(s, 1) & rs!Name")
        ids = [t.text for t in toks if t.kind == "id"]
        self.assertIn("Left", ids)
        self.assertEqual([t.low for t in toks if t.kind == "op"].count("!"), 1)

    def test_conditional_compilation(self):
        text = "#Const DEV = 0\n#If DEV Then\nx = 1\n#Else\nx = 2\n#End If\n"
        self.assertEqual([t.text for t in lex(text) if t.kind == "num"], ["2"])


class ParserTests(unittest.TestCase):
    def test_module_shapes(self):
        m = parse_module('''Option Explicit
Option Base 1
Public Const A = 1, B As Long = 2
Private x As Integer, y(1 To 3) As String, z() As Long
Public Type T
    a As Integer
    b(0 To 2) As Double
End Type
Public Enum E
    e1 = 1
    e2
End Enum
Declare Function Beep Lib "kernel32" (ByVal n As Long) As Long
Public Sub S(ByRef a As Long, Optional b As String = "q", c() As Integer)
End Sub
''', "modX")
        self.assertEqual(m.option_base, 1)
        self.assertEqual([v.name for v in m.vars], ["A", "B", "x", "y", "z"])
        self.assertIn("t", m.types)
        self.assertEqual(len(m.enums["e"][1]), 2)
        self.assertEqual(m.declares[0][0], "Beep")
        p = m.procs[0]
        self.assertEqual([q.name for q in p.params], ["a", "b", "c"])
        self.assertTrue(p.params[1].optional)
        self.assertTrue(p.params[2].is_array)

    def test_single_line_if_and_colon_forms(self):
        src = "Public Function F(n As Long) As Long\n If n > 1 Then F = 1 Else F = 2\n If n > 5 Then: F = 9\nEnd Function\n"
        m = parse_module(src, "m")
        self.assertEqual(m.procs[0].body[0].kind, "if")

    def test_unterminated_block_is_an_error(self):
        with self.assertRaises(VBSyntaxError):
            parse_module("Public Sub S()\n If True Then\nEnd Sub\n", "m")


class ControlFlow(unittest.TestCase):
    def test_if_elseif_else(self):
        src = '''Public Function F(n As Long) As String
    If n < 0 Then
        F = "neg"
    ElseIf n = 0 Then
        F = "zero"
    Else
        F = "pos"
    End If
End Function
'''
        self.assertEqual([val(src, "F", n) for n in (-1, 0, 1)], ["neg", "zero", "pos"])

    def test_single_line_if_else(self):
        src = "Public Function F(n As Long) As Long\n If n > 1 Then F = 10 Else F = 20\nEnd Function\n"
        self.assertEqual((val(src, "F", 5), val(src, "F", 0)), (10, 20))

    def test_select_case_forms(self):
        src = '''Public Function F(s As String) As Long
    Select Case s
        Case "a", "b"
            F = 1
        Case "c" To "e"
            F = 2
        Case Is >= "x"
            F = 3
        Case Else
            F = 0
    End Select
End Function
'''
        self.assertEqual([val(src, "F", s) for s in ("a", "d", "z", "m")], [1, 2, 3, 0])

    def test_for_step_and_exit(self):
        src = '''Public Function F() As Long
    Dim i As Long, t As Long
    For i = 10 To 1 Step -3
        t = t + i
    Next i
    For i = 1 To 100
        If i > 3 Then Exit For
        t = t + 100
    Next
    F = t
End Function
'''
        self.assertEqual(val(src, "F"), 10 + 7 + 4 + 1 + 300)

    def test_for_with_double_step(self):
        src = "Public Function F() As Double\n Dim x As Double, t As Double\n For x = 0 To 1 Step 0.25\n t = t + x\n Next x\n F = t\nEnd Function\n"
        self.assertEqual(val(src, "F"), 2.5)

    def test_do_loop_forms(self):
        src = '''Public Function F() As Long
    Dim i As Long, n As Long
    Do While i < 5
        i = i + 1: n = n + 1
    Loop
    Do
        i = i - 1: n = n + 10
    Loop Until i <= 2
    Do Until i = 0
        i = i - 1: n = n + 100
    Loop
    While n < 1000
        n = n + 500
    Wend
    F = n
End Function
'''
        self.assertEqual(val(src, "F"), 5 + 30 + 200 + 1000)

    def test_goto_and_labels(self):
        src = '''Public Function F() As Long
    Dim n As Long
    n = 1
    GoTo skip
    n = 100
skip:
    n = n + 1
    If n < 5 Then GoTo skip
    F = n
End Function
'''
        self.assertEqual(val(src, "F"), 5)

    def test_with_block_on_udt(self):
        src = '''Public Type P
    a As Long
    b As String
End Type
Public Function F() As String
    Dim x As P
    With x
        .a = 7
        .b = "q"
    End With
    F = x.b & x.a
End Function
'''
        self.assertEqual(val(src, "F"), "q7")

    def test_for_each_over_array(self):
        src = '''Public Function F() As Long
    Dim a As Variant, v As Variant, t As Long
    a = Array(1, 2, 3)
    For Each v In a
        t = t + v
    Next
    F = t
End Function
'''
        self.assertEqual(val(src, "F"), 6)


class ErrorHandling(unittest.TestCase):
    def test_on_error_goto_and_resume_label(self):
        src = '''Public Function F() As String
    On Error GoTo bad
    Dim a As Integer
    a = 30000
    a = a + a
    F = "no"
out:
    Exit Function
bad:
    F = "err " & Err.Number
    Resume out
End Function
'''
        self.assertEqual(val(src, "F"), "err 6")

    def test_resume_next_skips_the_failing_statement(self):
        src = '''Public Function F() As Long
    Dim n As Long
    On Error Resume Next
    n = 1
    n = n \\ 0
    n = n + 10
    F = n
End Function
'''
        self.assertEqual(val(src, "F"), 11)

    def test_error_propagates_to_callers_handler(self):
        src = '''Private Function Boom() As Long
    Boom = 1 / 0
End Function
Public Function F() As Long
    On Error GoTo h
    F = Boom()
    Exit Function
h:
    F = -Err.Number
End Function
'''
        self.assertEqual(val(src, "F"), -11)

    def test_resume_retries_the_statement(self):
        src = '''Public Function F() As Long
    Dim d As Long, tries As Long
    d = 0
    On Error GoTo fix
    tries = tries + 1
    F = 10 \\ d
    F = F + tries * 1000
    Exit Function
fix:
    d = 5
    Resume
End Function
'''
        self.assertEqual(val(src, "F"), 2 + 1000)

    def test_unhandled_error_escapes(self):
        with self.assertRaises(VBError) as cm:
            val("Public Function F() As Long\n F = 1 \\ 0\nEnd Function\n", "F")
        self.assertEqual(cm.exception.number, 11)

    def test_err_raise(self):
        src = '''Public Function F() As Long
    On Error GoTo h
    Err.Raise 53, , "nope"
    Exit Function
h:
    F = Err.Number
End Function
'''
        self.assertEqual(val(src, "F"), 53)


class DataModel(unittest.TestCase):
    def test_byref_and_byval(self):
        src = '''Private Sub Bump(a As Long, ByVal b As Long)
    a = a + 1: b = b + 1
End Sub
Public Function F() As String
    Dim x As Long, y As Long
    x = 1: y = 1
    Bump x, y
    Bump x, y
    F = x & "," & y
End Function
'''
        self.assertEqual(val(src, "F"), "3,1")

    def test_byref_array_element_and_field(self):
        src = '''Public Type P
    n As Integer
End Type
Private Sub Inc(a As Integer)
    a = a + 1
End Sub
Public Function F() As Long
    Dim arr(1 To 2) As Integer, p As P
    Inc arr(2)
    Inc arr(2)
    Inc p.n
    F = arr(2) * 10 + p.n
End Function
'''
        self.assertEqual(val(src, "F"), 21)

    def test_parenthesised_argument_is_by_value(self):
        src = '''Private Sub Bump(a As Long)
    a = a + 1
End Sub
Public Function F() As Long
    Dim x As Long
    x = 5
    Bump (x)
    F = x
End Function
'''
        self.assertEqual(val(src, "F"), 5)

    def test_optional_and_named_arguments(self):
        src = '''Private Function G(a As Long, Optional b As Long = 7, Optional c As Variant) As String
    G = a & "/" & b & "/" & IsMissing(c)
End Function
Public Function F() As String
    F = G(1) & " " & G(1, 2) & " " & G(a:=3, c:="z")
End Function
'''
        self.assertEqual(val(src, "F"), "1/7/True 1/2/True 3/7/False")

    def test_udt_copy_semantics_and_nested_arrays(self):
        src = '''Public Type Pt
    v(1 To 2) As Long
    s As String
End Type
Public Function F() As String
    Dim a As Pt, b As Pt
    a.v(1) = 5: a.s = "x"
    b = a
    b.v(1) = 9
    F = a.v(1) & b.v(1) & a.s
End Function
'''
        self.assertEqual(val(src, "F"), "59x")

    def test_multi_dimensional_arrays_and_bounds(self):
        src = '''Public Function F() As Long
    Dim m(1 To 2, 0 To 2) As Long, i As Long, j As Long
    For i = 1 To 2
        For j = 0 To 2
            m(i, j) = i * 10 + j
        Next j
    Next i
    F = m(2, 2) + UBound(m, 2) * 1000 + LBound(m, 1) * 100000
End Function
'''
        self.assertEqual(val(src, "F"), 22 + 2000 + 100000)

    def test_subscript_out_of_range(self):
        src = "Public Function F() As Long\n Dim a(1 To 3) As Long\n F = a(4)\nEnd Function\n"
        with self.assertRaises(VBError) as cm:
            val(src, "F")
        self.assertEqual(cm.exception.number, 9)

    def test_whole_array_arguments(self):
        src = '''Private Function Total(a() As Long) As Long
    Dim i As Long
    For i = LBound(a) To UBound(a)
        Total = Total + a(i)
    Next
End Function
Private Sub Fill(a() As Long)
    ReDim a(1 To 3)
    a(1) = 4: a(2) = 5: a(3) = 6
End Sub
Public Function F() As Long
    Dim x() As Long
    Fill x
    F = Total(x()) * 100 + UBound(x())
End Function
'''
        self.assertEqual(val(src, "F"), 1503)

    def test_redim_preserve_and_erase(self):
        src = '''Public Function F() As Long
    Dim a() As Long
    ReDim a(0 To 2)
    a(1) = 5
    ReDim Preserve a(0 To 4)
    F = a(1) * 100 + UBound(a)
    Erase a
End Function
'''
        self.assertEqual(val(src, "F"), 504)

    def test_option_base_one(self):
        src = "Option Base 1\nPublic Function F() As Long\n Dim a(3) As Long\n F = LBound(a) * 10 + UBound(a)\nEnd Function\n"
        # the preamble puts Option Explicit first; Option Base may follow
        it = Interpreter()
        it.load_source('Attribute VB_Name = "m"\nOption Explicit\n' + src, "m")
        self.assertEqual(it.call("F"), 13)

    def test_static_variable_persists(self):
        src = "Public Function F() As Long\n Static n As Long\n n = n + 1\n F = n\nEnd Function\n"
        it = Interpreter()
        it.load_source('Attribute VB_Name = "m"\n' + src, "m")
        self.assertEqual([it.call("F") for _ in range(3)], [1, 2, 3])

    def test_enum_and_const_expressions(self):
        src = '''Public Enum Lvl
    lo = 1
    mid
    hi = 10
    top
End Enum
Private Const Base As Long = 100
Public Const Limit = Base * 2
Public Function F() As Long
    F = mid + top + Lvl.hi + Limit
End Function
'''
        self.assertEqual(val(src, "F"), 2 + 11 + 10 + 200)

    def test_module_variables_and_globals(self):
        src = '''Public gCount As Long
Private Sub Hit()
    gCount = gCount + 1
End Sub
Public Function F() As Long
    Hit
    Hit
    F = gCount
End Function
'''
        r, it = run(src, "F")
        self.assertEqual(r, 2)
        self.assertEqual(it.get_global("gCount"), 2)
        it.set_global("gCount", 40)
        self.assertEqual(it.call("F"), 42)

    def test_integer_overflow_and_long_widening(self):
        src = '''Public Function F() As Long
    Dim a As Integer, b As Long
    a = 200
    b = a * 200&
    F = b
End Function
Public Function G() As Long
    Dim a As Integer, b As Long
    a = 200
    b = a * a
    G = b
End Function
'''
        self.assertEqual(val(src, "F"), 40000)
        with self.assertRaises(VBError):
            val(src, "G")

    def test_implicit_conversion_rounds_half_even(self):
        src = "Public Function F(d As Double) As Long\n Dim i As Long\n i = d\n F = i\nEnd Function\n"
        self.assertEqual([val(src, "F", x) for x in (2.5, 3.5, -2.5, 0.5)], [2, 4, -2, 0])

    def test_currency_arithmetic_is_exact(self):
        src = '''Public Function F() As Currency
    Dim a As Currency
    a = 0.1
    F = a + 0.2
End Function
Public Function G() As String
    Dim a As Currency
    a = 10
    G = CStr(a / 3)
End Function
'''
        self.assertEqual(int(val(src, "F")), 3000)
        self.assertEqual(val(src, "G"), "3.3333")

    def test_string_functions(self):
        src = '''Public Function F() As String
    F = UCase(Mid("hello world", 7, 3)) & "|" & Replace("a-b-c", "-", "+") & "|" & Trim("  x ") & "|" & _
        Right$("abcdef", 2) & "|" & InStr(1, "Hello", "L", vbTextCompare) & "|" & Join(Split("a,b,c", ","), ";")
End Function
'''
        self.assertEqual(val(src, "F"), "WOR|a+b+c|x|ef|3|a;b;c")

    def test_like_and_comparison_operators(self):
        src = '''Public Function F() As String
    F = IIf("abc" Like "a*c", "y", "n") & IIf("abc" Like "a?", "y", "n") & IIf(3 <> 4, "y", "n") & IIf("a" < "b", "y", "n")
End Function
'''
        self.assertEqual(val(src, "F"), "ynyy")

    def test_boolean_logic_is_bitwise(self):
        src = '''Public Function F() As String
    Dim t As Boolean
    t = True
    F = (5 And 3) & "," & (5 Or 3) & "," & (Not 5) & "," & (t And False) & "," & (-t)
End Function
'''
        self.assertEqual(val(src, "F"), "1,7,-6,False,1")

    def test_function_name_assignment_returns_default(self):
        src = "Public Function F(ByVal a As Long) As Long\n If a > 0 Then F = 5\nEnd Function\nPublic Function S() As String\nEnd Function\n"
        self.assertEqual(val(src, "F", 0), 0)
        self.assertEqual(val(src, "S"), "")


class Classes(unittest.TestCase):
    CLS = '''Option Explicit
Private m_n As Integer
Private m_arr(0 To 2) As Long
Public Total As Currency
Private Sub Class_Initialize()
    m_n = 5
End Sub
Public Property Get N() As Integer
    N = m_n
End Property
Public Property Let N(ByVal v As Integer)
    m_n = v
End Property
Public Property Get Item(ByVal i As Integer) As Long
    Item = m_arr(i)
End Property
Public Property Let Item(ByVal i As Integer, ByVal v As Long)
    m_arr(i) = v
End Property
Public Function Twice() As Integer
    Twice = Me.N * 2
End Function
Public Sub AddTo(x As Long)
    x = x + m_n
End Sub
'''
    MOD = '''Public Function F() As Long
    Dim c As clsThing, k As Long
    Set c = New clsThing
    c.N = 21
    c.Item(1) = 8
    k = 100
    c.AddTo k
    c.Total = c.Total + 2.5
    F = c.Twice() + c.Item(1) * 1000 + k * 100000 + c.Total * 10000000
End Function
Public Function G() As Long
    Dim c As clsThing
    G = c.N
End Function
'''

    def make(self):
        it = Interpreter()
        it.load_source('Attribute VB_Name = "clsThing"\n' + self.CLS, "clsThing", "cls")
        it.load_source('Attribute VB_Name = "modUse"\nOption Explicit\n' + self.MOD, "modUse")
        return it

    def test_class_properties_methods_and_byref(self):
        it = self.make()
        # Twice 42 + Item 8000 + k 121*100000 + Total 2.5*10000000
        self.assertEqual(it.call("F"), 42 + 8000 + 12100000 + 25000000)

    def test_unset_loaded_class_variable_is_vb_error_91(self):
        it = self.make()
        with self.assertRaises(VBError) as cm:
            it.call("G")
        self.assertEqual(cm.exception.number, 91)

    def test_class_initialize_runs(self):
        it = self.make()
        inst = it.instantiate(it.classes["clsthing"])
        self.assertEqual(it.call("Twice", this=inst, module="clsThing"), 10)


class Unbound(unittest.TestCase):
    def test_unknown_name_raises_oracle_unbound_not_vberror(self):
        src = "Public Function F() As Long\n On Error Resume Next\n F = Missing + 1\nEnd Function\n"
        with self.assertRaises(OracleUnbound) as cm:
            val(src, "F")
        self.assertEqual(cm.exception.name, "Missing")

    def test_member_of_unbound_form(self):
        src = "Public Function F() As String\n F = frmMain.txtName.Text\nEnd Function\n"
        with self.assertRaises(OracleUnbound) as cm:
            val(src, "F")
        self.assertEqual(cm.exception.name, "frmMain")

    def test_declared_recordset_global_is_unbound(self):
        src = 'Public tabItems As Recordset\nPublic Function F() As String\n F = tabItems.Fields("Name")\nEnd Function\n'
        with self.assertRaises(OracleUnbound) as cm:
            val(src, "F")
        self.assertEqual(cm.exception.name, "tabItems")

    def test_unloaded_procedure_call(self):
        src = "Public Function F() As Long\n F = GetThing(3)\nEnd Function\n"
        with self.assertRaises(OracleUnbound) as cm:
            val(src, "F")
        self.assertEqual(cm.exception.name, "GetThing")

    def test_bound_shims_satisfy_references(self):
        class Ctl(ExtObject):
            def vb_get(self, name, args):
                return "text!" if name.lower() == "text" else 0

        class Form(ExtObject):
            def vb_get(self, name, args):
                return Ctl()

        src = "Public Function F() As String\n F = frmMain.txtName.Text & GetThing(3)\nEnd Function\n"

        def setup(it):
            it.bind("frmMain", Form())
            it.bind_proc("GetThing", lambda it, n: V.mk_i32(int(n) * 2))
        self.assertEqual(val(src, "F", setup=setup), "text!6")

    def test_end_statement_stops_the_program(self):
        from vbi.errors import EndProgram
        with self.assertRaises(EndProgram):
            val("Public Function F() As Long\n End\nEnd Function\n", "F")


class RndTests(unittest.TestCase):
    def test_rnd_sequence_matches_vb6_default_seed(self):
        src = "Public Function F() As Double\n F = Rnd\nEnd Function\n"
        r, it = run(src, "F")
        self.assertAlmostEqual(float(r), 0.7055475, places=6)

    def test_rnd_idiom_with_integer_arguments(self):
        src = "Public Function R(a As Integer, b As Integer) As Integer\n R = Int(((b - a + 1) * Rnd) + a)\nEnd Function\n"
        it = Interpreter()
        it.load_source('Attribute VB_Name = "m"\n' + src, "m")
        self.assertEqual([int(it.call("R", 1, 100)) for _ in range(3)], [71, 54, 58])


if __name__ == "__main__":
    unittest.main()

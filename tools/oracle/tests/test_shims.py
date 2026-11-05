"""Shim, scenario and fixture-writer tests on invented data (no MME source, no stock database)."""
import os
import sqlite3
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from vbi.interp import Interpreter  # noqa: E402
from vbi import values as V  # noqa: E402
from vbi.errors import VBError  # noqa: E402
import shims  # noqa: E402
import scenario  # noqa: E402
import harness  # noqa: E402

VB = '''Option Explicit
Public tabItems As Recordset
Public gLevel As Long
Public Function ItemName(ByVal n As Long) As String
    tabItems.Index = "pkItems"
    tabItems.Seek "=", n
    If tabItems.NoMatch Then
        ItemName = "?"
        tabItems.MoveFirst
    Else
        ItemName = tabItems.Fields("Name") & ":" & tabItems!Max
    End If
End Function
Public Function CountAll() As Long
    tabItems.MoveFirst
    Do While Not tabItems.EOF
        CountAll = CountAll + tabItems.Fields("Max")
        tabItems.MoveNext
    Loop
End Function
Public Function Lvl() As Long
    Lvl = Val(frmMain.txtGlobalLevel.Text) + frmMain.lblArr(2).Caption
End Function
Public Function AskUser() As Integer
    AskUser = MsgBox("hi", vbYesNo + vbDefaultButton2)
End Function
Public Function BadField() As Long
    BadField = tabItems.Fields("nope")
End Function
'''


def make_db(path):
    con = sqlite3.connect(path)
    con.execute('CREATE TABLE "Items" ("Number" INTEGER, "Name" TEXT, "Max" INTEGER)')
    con.executemany('INSERT INTO "Items" VALUES (?,?,?)', [(30, "axe", 7), (10, "bow", 3), (20, None, 5)])
    con.commit()
    con.close()


class ShimTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.dbp = os.path.join(cls.tmp.name, "t.sqlite")
        make_db(cls.dbp)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def env(self):
        it = Interpreter()
        it.load_source(VB, "modT", "bas")
        it.ensure_ready()
        db = shims.Database(self.dbp)
        it.global_cell("tabItems").v = db.open_recordset("Items")
        it.bind("frmMain", shims.FormShim({"txtGlobalLevel": {"Text": "12"},
                                           "lblArr": {"$array": {"2": {"Caption": "3"}}}}, harness.decode))
        it.intrinsic_hooks["msgbox"] = lambda it_, *a: shims.msgbox_result(int(a[1]))
        return it

    def test_seek_finds_by_index_order_and_fields(self):
        it = self.env()
        self.assertEqual(it.call("ItemName", 10), "bow:3")
        self.assertEqual(it.call("ItemName", 30), "axe:7")
        self.assertEqual(it.call("ItemName", 20), ":5")      # NULL text reads as ""
        self.assertEqual(it.call("ItemName", 99), "?")

    def test_move_walks_physical_order_without_index(self):
        self.assertEqual(int(self.env().call("CountAll")), 15)

    def test_record_count_and_eof_after_failed_seek(self):
        rs = shims.Database(self.dbp).open_recordset("Items")
        self.assertEqual(int(rs.vb_get("RecordCount", [])), 3)
        rs.vb_set("Index", [], "pkItems")
        rs.vb_get("Seek", ["=", V.I32(11)])
        self.assertTrue(rs.vb_get("NoMatch", []))
        self.assertTrue(rs.vb_get("EOF", []))
        with self.assertRaises(VBError) as e:
            rs.vb_get("Fields", ["Name"])
        self.assertEqual(e.exception.number, 3021)

    def test_seek_without_index_and_unknown_field_and_index(self):
        rs = shims.Database(self.dbp).open_recordset("Items")
        with self.assertRaises(VBError) as e:
            rs.vb_get("Seek", ["=", V.I32(10)])
        self.assertEqual(e.exception.number, 3019)
        with self.assertRaises(VBError) as e:
            rs.vb_set("Index", [], "pkSpells")
        self.assertEqual(e.exception.number, 3015)
        with self.assertRaises(VBError) as e:
            rs.vb_get("Fields", ["nope"])
        self.assertEqual(e.exception.number, 3265)

    def test_form_controls_are_value_holders(self):
        self.assertEqual(int(self.env().call("Lvl")), 15)

    def test_unset_control_reads_as_default(self):
        c = shims.ControlShim()
        self.assertEqual(c.vb_get("Text", []), "")
        self.assertEqual(int(c.vb_get("ListIndex", [])), -1)
        self.assertEqual(int(c.vb_get("Value", [])), 0)

    def test_msgbox_returns_the_default_button(self):
        self.assertEqual(int(self.env().call("AskUser")), 7)   # Yes/No, second button is No
        self.assertEqual(int(shims.msgbox_result(0)), 1)


class ScenarioTests(unittest.TestCase):
    def test_grid_is_cartesian_and_ids_are_stable(self):
        s = {"grids": [{"name": "g", "axes": {"a": [1, 2], "b": ["x", "y"]}, "case": {"proc": "P", "args": ["$a", "$b"]}}]}
        cs = scenario.expand(s)
        self.assertEqual([c["id"] for c in cs], ["g-0000", "g-0001", "g-0002", "g-0003"])
        self.assertEqual(cs[1]["args"], [1, "y"])

    def test_sample_is_deterministic_and_ops_work(self):
        s = {"samples": [{"name": "s", "seed": 5, "count": 3,
                          "draw": {"lo": {"int": [0, 9], "n": 2}, "sp": {"int": [1, 3], "n": 2}},
                          "case": {"mn": "$lo", "mx": {"$add": ["$lo", "$sp"]}, "m": {"$merge": [{"a": 1}, {"b": 2}]}}}]}
        a, b = scenario.expand(s), scenario.expand(s)
        self.assertEqual(a, b)
        self.assertTrue(all(y > x for c in a for x, y in zip(c["mn"], c["mx"])))
        self.assertEqual(a[0]["m"], {"a": 1, "b": 2})

    def test_splitmix_known_values(self):
        r = scenario.SplitMix(0)
        self.assertEqual(r.next(), 0xE220A8397B1DCDAF)

    def test_duplicate_ids_rejected(self):
        with self.assertRaises(ValueError):
            scenario.expand({"cases": [{"id": "a"}, {"id": "a"}]})


class EncodeTests(unittest.TestCase):
    def test_types(self):
        self.assertEqual(harness.encode(V.I32(5)), 5)
        self.assertEqual(harness.encode(V.c_cur(12.5)), {"cur": 125000, "s": "12.5"})
        self.assertEqual(harness.encode(V.c_cur(-0.0001)), {"cur": -1, "s": "-0.0001"})
        self.assertEqual(harness.encode(V.f32(0.1)), {"single": float(V.f32(0.1)), "s": "0.1"})
        self.assertIsInstance(harness.encode(3.0), float)
        self.assertEqual(harness.encode(True), True)

    def test_decode_typed(self):
        self.assertIs(type(harness.decode({"t": "cur", "v": 1.5})), V.Cur)
        self.assertIs(type(harness.decode(7)), V.I16)
        self.assertIs(type(harness.decode(70000)), V.I32)


if __name__ == "__main__":
    unittest.main()

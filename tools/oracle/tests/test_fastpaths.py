"""The interpreter's arithmetic fast paths keep the general path's results and errors."""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from vbi import values as V  # noqa: E402
from vbi.errors import VBError  # noqa: E402


class FastPaths(unittest.TestCase):
    def test_double_with_integer_types(self):
        for i in (V.I16(3), V.I32(70000), V.U8(5)):
            self.assertEqual(V.add(1.5, i), 1.5 + float(i))
            self.assertEqual(V.sub(i, 0.25), float(i) - 0.25)
            self.assertEqual(V.mul(i, 2.0), float(i) * 2.0)
            self.assertIs(type(V.mul(i, 2.0)), float)

    def test_integer_overflow_still_raises(self):
        with self.assertRaises(VBError) as e:
            V.add(V.I16(32767), V.I16(1))
        self.assertEqual(e.exception.number, 6)
        self.assertIs(type(V.add(V.I16(1), V.I32(1))), V.I32)

    def test_double_overflow_raises(self):
        with self.assertRaises(VBError) as e:
            V.mul(1e308, 10.0)
        self.assertEqual(e.exception.number, 6)

    def test_divide_by_zero(self):
        with self.assertRaises(VBError) as e:
            V.div(1.0, V.I16(0))
        self.assertEqual(e.exception.number, 11)
        self.assertEqual(V.div(V.I16(1), 4.0), 0.25)

    def test_compare_mixed(self):
        self.assertEqual(V.compare(V.I32(2), 2.0), 0)
        self.assertEqual(V.compare(1.5, V.I16(2)), -1)
        self.assertEqual(V.compare(V.U8(3), V.I16(2)), 1)


if __name__ == "__main__":
    unittest.main()

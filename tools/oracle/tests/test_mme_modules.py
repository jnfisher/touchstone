"""Parses and compiles MME's real formula modules from the pinned cache (skipped when the cache is
absent: tools/mme-fetch.sh fills it; CI does not run the oracle)."""
import glob
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from vbi.interp import Interpreter  # noqa: E402
from vbi.parser import parse_file  # noqa: E402
from vbi import analysis, values as V  # noqa: E402

FORMULA = ["modMMudFunc.bas", "modSyntaxsFunc.bas", "clsMonsterAttackSim.cls", "modExpPerHour.bas"]
DECLS = ["modMain.bas", "modMMudDatabase.bas"]
# procedure counts from the T-002 index at the pin
COUNTS = {"modMMudFunc.bas": 88, "modSyntaxsFunc.bas": 40, "clsMonsterAttackSim.cls": 136,
          "modExpPerHour.bas": 33, "modMain.bas": 94, "modMMudDatabase.bas": 97}


def mme_dir():
    d = os.environ.get("MME_DIR")
    if d and os.path.isdir(d):
        return d
    cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
    hits = glob.glob(os.path.join(cache, "mme", "*", "modMMudFunc.bas"))
    return os.path.dirname(hits[0]) if hits else None


@unittest.skipUnless(mme_dir(), "MME source not in the cache (run tools/mme-fetch.sh)")
class MmeModules(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = mme_dir()
        cls.it = Interpreter()
        for f in DECLS:
            cls.it.load_file(os.path.join(cls.dir, f), load_procs=False)
        for f in FORMULA:
            cls.it.load_file(os.path.join(cls.dir, f))
        cls.it.ensure_ready()

    def test_every_module_parses_with_the_indexed_procedure_count(self):
        for f, n in COUNTS.items():
            m = parse_file(os.path.join(self.dir, f))
            self.assertEqual(len(m.procs), n, f)

    def test_every_formula_procedure_compiles(self):
        for m in self.it.modules.values():
            if not m.load_procs:
                continue
            for p in m.ast.procs:
                self.it.compiler.compiled(p)

    def test_declaration_globals_are_loaded_but_their_procedures_are_not(self):
        self.assertIn("bgreatermud", self.it.globals)
        self.assertIsNone(self.it.find_proc("PullMonsterDetail"))

    def test_stock_dodge_vs_accuracy(self):
        # by hand from CalcDodgeVSAccuracy's stock branch: (30 * 10) \ (40 \ 8) = 60, under the cap of 95
        self.assertEqual(int(self.it.call("CalcDodgeVSAccuracy", 30, 40)), 60)

    def test_currency_energy_adjustment_rounds_division_to_four_decimals(self):
        # AdjustEnergyUsedWithSpeed: Fix((EU * Speed) / 100); 1000 * 120 / 100 = 1200
        r = self.it.call("AdjustEnergyUsedWithSpeed", V.c_cur(1000.0), V.c_cur(120.0))
        self.assertEqual((type(r), int(r)), (V.Cur, 12000000))

    def test_attack_simulation_is_deterministic_for_a_seed(self):
        # RunSim calls Randomize with no argument; the oracle leaves the generator alone then, so a
        # scenario controls the draws through it.rnd. No MME value is pinned, only reproducibility.
        def run_once():
            it = self.it
            it.rnd.state = 0x50000
            sim = it.instantiate(it.classes["clsmonsterattacksim"])
            comp = it.compiler
            for name, args in [("nNumberOfRounds", [V.I32(300)]), ("nEnergyPerRound", [V.I32(1000)]),
                               ("nAtkType", [V.I16(0), V.I16(1)]), ("nAtkChance", [V.I16(0), V.I16(100)]),
                               ("nAtkEnergy", [V.I16(0), V.I16(1000)]), ("nAtkMin", [V.I16(0), V.I16(5)]),
                               ("nAtkMax", [V.I16(0), V.I16(15)]), ("nAtkSuccess", [V.I16(0), V.I16(80)]),
                               ("nUserAC", [V.I32(40)])]:
                comp.member_set(sim, name.lower(), args[:-1], args[-1], "", name, False)
            it.call("RunSim", this=sim, module="clsMonsterAttackSim")
            return int(comp.member_get(sim, "ntotaldamage", [], "sim", "nTotalDamage"))
        a, b = run_once(), run_once()
        self.assertEqual(a, b)
        self.assertGreater(a, 0)

    def test_analysis_finds_the_recordsets_and_forms(self):
        use = analysis.analyze(self.it, {"modmmudfunc", "modsyntaxsfunc", "clsmonsterattacksim", "modexpperhour"})
        self.assertIn("tabItems", use.recordsets)
        self.assertIn("Name", use.recordsets["tabItems"]["fields"])
        self.assertIn("frmMain", use.forms)
        clean = analysis.transitive_clean(use, list(use.calls))
        self.assertGreater(len(clean), 100)


if __name__ == "__main__":
    unittest.main()

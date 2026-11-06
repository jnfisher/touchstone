"""Writes fixtures/scenarios/mega_stat.json (FAM-22, the MegaMUD stat-line parser of frmSwingCalc):
    python3 -I tools/oracle/gen/mega_stat.py
The procedures are cut out of frmSwingCalc.frm (scenario key frm). cmdPasteMega_Click reads the
clipboard in MME; here that one line is replaced by a read of a control, so a case supplies the
"clipboard" text as frmMain.txtClip.Text. All texts are invented stat lines; deterministic (splitmix64)."""
import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
OUT = os.path.join(ROOT, "fixtures", "scenarios", "mega_stat.json")
M64 = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def next(self):
        self.s = (self.s + 0x9E3779B97F4A7C15) & M64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
        return z ^ (z >> 31)

    def int(self, lo, hi):
        return lo + self.next() % (hi - lo + 1)

    def chance(self, pct):
        return self.int(1, 100) <= pct

    def pick(self, xs):
        return xs[self.int(0, len(xs) - 1)]


PCTS = ["0%", "1%", "43%", "100%", "7%", "99%", "12.5%", "0.5%", "100.0%", "05%", "-4%", "43 %", "%", "4%%",
        "1e2%", "&H10%", "1,5%", "250%"]
PLACE = ["---", "N/A", "n/a", "N/a", "--", "----", "?", "-", "N/ A", "NA", "n/a.", "--- ", " ---"]
AVGS = ["0", "34", "181", "1,234", "7", "12,345,678", "00", "007", "1,2,3", ",5", "5,", ",", "3.5", "-2", "+4",
        "1e3", "&H1F", "abc", "12a", "34%", "2147483647", "2147483648", "99999999999", "1 234"]
RNGS = ["11-58", "0-0", "---", "5-9", "1,000-2,000", "N/A"]
LABELS = ["Hit:", "Extra:", "Crit:"]


def line(rng, label, kind):
    """One stat line in the old or the new layout, with random spacing."""
    sp = lambda: rng.pick([" ", "  ", "   ", "\t", " \t", "     "])
    pct = rng.pick(PLACE) if rng.chance(20) else rng.pick(PCTS)
    avg = rng.pick(AVGS) if rng.chance(55) else str(rng.int(0, 400))
    rg = rng.pick(RNGS)
    if kind == 0:
        return "%s%s%s%sRng:%s%sAvg:%s" % (label, sp(), pct, sp(), rg, sp(), avg)
    if kind == 1:
        return "%s%s%s%s%s%s%s" % (label, sp(), pct, sp(), rg, sp(), avg)
    if kind == 2:
        return "%s%s%s" % (label, sp(), pct)
    if kind == 3:
        return "%s%s%s%sAVG:%s" % (label, sp(), pct, sp(), avg)
    if kind == 4:
        return "%s%s%s%savg:  %s%s" % (label, sp(), pct, sp(), avg, sp())
    return "%s%s%s%s%s" % (label, sp(), pct, sp(), avg)


NOISE = ["MegaMUD Statistics", "-----------------", "Attacks: 120   Misses: 4", "Dodge: 12%", "Spell: 3%  Rng:1-9  Avg:5",
         "", "   ", "Hits: 50% Rng:1-2 Avg:3", "hit: 99% Rng:1-2 Avg:77", "Thit: 10% Rng:1-2 Avg:3", " Crit: 3%",
         "Exp/hr: 12,345", "Hit chance: 90%"]


def block(rng):
    lines = []
    for label in LABELS:
        if label == "Extra:" and rng.chance(25):
            continue
        if rng.chance(8):
            continue
        k = rng.int(0, 5) if rng.chance(35) else rng.pick([0, 0, 1, 1])
        l = line(rng, label, k)
        if rng.chance(15):
            l = rng.pick([" ", "  ", "\t"]) + l
        if rng.chance(8):
            l = l.upper() if rng.chance(50) else l.lower()
        lines.append(l)
        while rng.chance(25):
            lines.append(rng.pick(NOISE))
    for _ in range(rng.int(0, 2)):
        lines.insert(rng.int(0, len(lines)), rng.pick(NOISE))
    sep = rng.pick(["\n", "\r\n", "\r", "\n", "\r\n"])
    t = sep.join(lines)
    if rng.chance(20):
        t += sep
    return t


GOOD = "Hit:   43%   Rng:11-58      Avg:34\nExtra: ---   Rng:---        Avg:0\nCrit:  5%    Rng:30-80      Avg:55"
GOOD_NEW = "Hit:   43%   11-58          34\nExtra: 12%   2-9      6\nCrit:  5%    30-80      55"

HAND_TEXTS = [
    GOOD, GOOD_NEW, GOOD.replace("\n", "\r\n"), GOOD.replace("\n", "\r"), GOOD_NEW.replace("\n", "\r\n") + "\r\n",
    "Hit: 43%\nCrit: 5%", "Hit: 43% 11-58 34\nCrit: 5%", "Hit: 43% 11-58 34\nCrit: --- 0-0 0",
    "Hit: --- Avg:0\nCrit: N/A Avg:0", "Hit: --- 0-0 0\nExtra: 33% 1-2 3\nCrit: --- 0-0 0",
    "hit: 43% rng:1-2 avg:12\ncrit: 5% rng:1-2 avg:3", "HIT: 43% RNG:1-2 AVG:12\nCRIT: 5% RNG:1-2 AVG:3",
    "Hit: 1,234% Rng:1-2 Avg:1,234\nCrit: 5% Rng:1-2 Avg:2,345",
    "Hit: 43% Rng:11-58 Avg:99999999999\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3\nExtra: 5% Rng:1-2 Avg:99999999999\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:2147483648",
    "Hit: 43% Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:2147483647",
    "Hit: 43% Rng:11-58 Avg:3\nExtra: 17% Rng:1-2 Avg:x\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3\nExtra: 17%\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3\nExtra: ? Rng:1-2 Avg:4\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3\nExtra: -- Rng:1-2 Avg:4\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: ? Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:3", "Hit: -- Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg:3", "Crit: 5% Rng:1-2 Avg:3", "Extra: 5% Rng:1-2 Avg:3",
    "Hit:43% Rng:11-58 Avg:3\nCrit:5% Rng:1-2 Avg:3", "Hit:\nCrit:", "Hit: \nCrit: ", "Hit:   \t  \nCrit:\t",
    "Hits: 43% Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:3", "Hit : 43% Rng:11-58 Avg:3\nCrit: 5% Rng:1-2 Avg:3",
    "  Hit: 43% Rng:11-58 Avg:3\n\t Crit: 5% Rng:1-2 Avg:3", "Hit: 43% Rng:11-58 Avg:3 \nCrit: 5% Rng:1-2 Avg:3 ",
    "Hit: 43% Rng:11-58 Avg:3\nHit: 99% Rng:11-58 Avg:77\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: bad\nHit: 99% Rng:11-58 Avg:77\nCrit: 5% Rng:1-2 Avg:3",
    "Hit: 43% Rng:11-58 Avg: 3\nCrit: 5% Rng:1-2 Avg:   4", "Hit: 43% Rng:11-58 Avg:\nCrit: 5% Rng:1-2 Avg:4",
    "Hit: 43% Avg:3 Avg:9\nCrit: 5% Avg:4", "Hit: 43% Rng:1-2 AvgAvg:3\nCrit: 5% Avg:4",
    "Hit: 43% Rng:1-2 Avg:3x\nCrit: 5% Avg:4", "Hit: 43% Rng:1-2 3\nCrit: 5% 4",
    "Hit: 43% Rng:1-2 Avg:3\nCrit: 5%", "Hit: 43% Rng:1-2 11-58\nCrit: 5% 1-2 3", "Hit: 43% 1,234\nCrit: 5% 2",
    "Hit: 43%\t11-58\t34\nCrit: 5%\t1-2\t3", "Hit: 43%  11-58  34  \nCrit: 5%  1-2  3  ",
    "Hit: 43%  11-58  34\t\nCrit: 5%  1-2  3", "Hit: 43.5% 11-58 34\nCrit: 5.25% 1-2 3",
    "Hit: .5% 11-58 34\nCrit: 5.% 1-2 3", "Hit: 4 3% 11-58 34\nCrit: 5% 1-2 3",
    "Hit: 43%a 11-58 34\nCrit: 5% 1-2 3", "Hit: 4e1% 11-58 34\nCrit: 5% 1-2 3",
    "Hit: &H10% 11-58 34\nCrit: 5% 1-2 3", "Hit: -5% 11-58 34\nCrit: 5% 1-2 3",
    "Hit: % 11-58 34\nCrit: 5% 1-2 3", "Hit: 0% 0-0 0\nExtra: 0% 0-0 0\nCrit: 0% 0-0 0",
    "Hit: 100% 11-58 34\nExtra: 100% 11-58 34\nCrit: 100% 11-58 34",
    "Hit: 43% 11-58 34\nExtra: 12% 2-9 6\nCrit: 5% 30-80 55\nHit: 1% 1-1 1",
    "Hit: 43% 11-58 34\nExtra: 12% 2-9 6\nExtra: 99% 2-9 60\nCrit: 5% 30-80 55",
    "", " ", "\n", "\r\n", "garbage", "Hit", "Hit:Crit:", "Crit: 5% 1-2 3\nHit: 43% 11-58 34",
    "Crit: 5% 1-2 3\nExtra: 7% 1-2 9\nHit: 43% 11-58 34",
    "Hit: 43% 11-58 34\n\n\n\nCrit: 5% 1-2 3", "Hit: 43% 11-58 34\rCrit: 5% 1-2 3\rExtra: 1% 1-2 3",
    "Hit: 43% 11-58 34\n\rCrit: 5% 1-2 3", "Hit: 43% 11-58 34\r\n\r\nCrit: 5% 1-2 3\r\n",
    "Hít: 43% 11-58 34\nCrit: 5% 1-2 3",
]

TOKENS = ["", " ", "  ", "a", " a", "a ", " a ", "a b", "a  b", "a\tb", "\ta", "a\t", "\t", " \t ", "a \t b", "43% 11-58 34",
          "43%\t11-58\t34", "  43%   11-58   34  ", "---", " --- ", "N/A", "n/a", "n/a ", "N/A.", "", "x\ty z",
          "a\t\tb", "a b\t", "a b \t", "\ta b", "word"]
NUMS = ["", " ", "0", "34", "1,234", " 1,234 ", "1,2,3", ",", ",,", "12 ", " 12", "1 2", "12a", "-1", "+1", "1.5", "1e3",
        "\t5", "5\t", "00", "٣", "0x1F", "&H1F", "99999999999", "1,000,000", "5,", ",5", "a,b", "12,345,678,901"]
LABEL_ARGS = ["Hit:", "Extra:", "Crit:", "hit:", "CRIT:", "H", "", " ", "Hit", "Hit:  ", "Rng:", "Hit: 43%", "Avg:"]
LINE_TEXTS = ["Hit: 43% Rng:11-58 Avg:34\nCrit: 5%", "  Hit: 1 \r\nHIT: 2\rhit: 3", "x\nHit:\nHit: 2", "\n\nExtra: 4", "",
              "abc", "Hit: 1\n\nCrit: 3", " \t Crit: 5% \t\nExtra: 9"]


def build():
    cases = []
    n = 0

    def add(prefix, **kw):
        nonlocal n
        kw["id"] = "%s%03d" % (prefix, n)
        n += 1
        cases.append(kw)

    def parse(prefix, text, label):
        add(prefix, steps=[{"call": "ParseMegaStatLine", "as": "p", "args": [
            text, label, {"ref": "double", "value": 0.0}, {"ref": "long", "value": 0}]}])

    for t in HAND_TEXTS:
        for label in LABELS:
            parse("h", t, label)
    rng = Rng(0x39E6A)
    for _ in range(100):
        t = block(rng)
        parse("r", t, rng.pick(LABELS))
    for t in TOKENS:
        for fn in ("FirstMegaToken", "LastMegaToken", "IsMegaPlaceholder"):
            add("t", proc=fn, args=[t])
    for t in NUMS:
        for fn in ("CleanMegaNumber", "IsMegaWholeNumberToken"):
            add("n", proc=fn, args=[t])
    for t in LINE_TEXTS:
        for lab in LABEL_ARGS:
            add("l", proc="GetMegaStatLine", args=[t, lab])
    return cases, rng


def paste_cases(rng, start):
    cases = []
    texts = [GOOD, GOOD_NEW, "", " ", "\r\n"] + HAND_TEXTS[:45] + [block(rng) for _ in range(80)]
    for i, t in enumerate(texts):
        cases.append({"id": "p%03d" % (start + i), "forms": {"frmMain": {"txtClip": {"Text": t}}},
                      "steps": [{"call": "cmdPasteMega_Click"}],
                      "read": {"box": "%c.txtTrueAVG.Text[0..5]"}})
    return cases


def main():
    cases, rng = build()
    cases += paste_cases(rng, len(cases))
    scn = {
        "family": "mega_stat",
        "description": "FAM-22 MegaMUD stat-line parser (frmSwingCalc): ParseMegaStatLine, GetMegaStatLine, "
                       "FirstMegaToken, LastMegaToken, IsMegaPlaceholder, IsMegaWholeNumberToken, CleanMegaNumber, "
                       "and cmdPasteMega_Click over a clipboard text.",
        "notes": [
            "Procedures are cut out of frmSwingCalc.frm (scenario key frm). ParseMegaStatLine is called with ByRef "
            "Double/Long outputs (outputs p.refs = [percent, average]) and returns its Boolean.",
            "cmdPasteMega_Click's Clipboard.GetText line is replaced by a read of frmMain.txtClip.Text; its result is the "
            "six txtTrueAVG(0..5) texts (empty when nothing was assigned) and a msgbox event for the incomplete-statistics "
            "message. An error (Long overflow of an average) is a handled_error event for cmdPasteMega_Click.",
            "Texts are invented stat lines; generated by tools/oracle/gen/mega_stat.py.",
        ],
        "database": False,
        "forms": {"frmMain": {"txtTrueAVG": {"$array": {str(i): {} for i in range(6)}}}},
        "frm": [{
            "file": "frmSwingCalc.frm",
            "procs": ["ParseMegaStatLine", "GetMegaStatLine", "FirstMegaToken", "LastMegaToken", "IsMegaPlaceholder",
                      "IsMegaWholeNumberToken", "CleanMegaNumber", "cmdPasteMega_Click"],
            "subs": {"cmdPasteMega_Click": {"sClipText = Clipboard.GetText": "sClipText = frmMain.txtClip.Text"}},
        }],
        "tolerance": {"default": 0},
        "cases": cases,
    }
    with open(OUT, "w", newline="\n") as f:
        json.dump(scn, f, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        f.write("\n")
    print("wrote", OUT, len(cases), "cases")


if __name__ == "__main__":
    main()

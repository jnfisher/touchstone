"""Writes fixtures/scenarios/item_value.json (FAM-18) from the stock database.

Run from the repository root: python3 -I tools/oracle/gen/item_value.py
Deterministic: fixed strides, no randomness. Needs the converted stock SQLite
(tools/mdb-fetch.sh). Each case carries the few row fields the Rust test needs under "rows".
"""
import glob, json, os, sqlite3, sys

cache = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))
dbs = sorted(glob.glob(os.path.join(cache, "mdb", "*.sqlite")))
if not dbs:
    sys.exit("no converted database; run tools/mdb-fetch.sh")
c = sqlite3.connect(dbs[0])
cur = c.execute("select * from Shops order by Number")
shops = cur.fetchall()
ix = {d[0]: i for i, d in enumerate(cur.description)}

SH = {}
for r in shops:
    SH[r[ix["Number"]]] = {
        "type": r[ix["ShopType"]], "markup": r[ix["Markup%"]],
        "stock": [[r[ix[f"{k}-{x}"]] for k in ("Item", "Max", "Time", "Amount")] + [r[ix[f"%-{x}"]]]
                  for x in range(20) if r[ix[f"Item-{x}"]]]}
IT = {n: {"price": p, "currency": cu}
      for n, p, cu in c.execute("select Number,Price,Currency from Items order by Number")}
items = sorted(IT)

pairs, seen = [], set()
for s in sorted(SH):
    for slot in SH[s]["stock"]:
        if (s, slot[0]) not in seen:
            seen.add((s, slot[0]))
            pairs.append((s, slot[0]))

cases = []


def add(name, proc, args):
    d = {"id": f"{name}_{len(cases)}", "proc": proc, "args": args}
    rows = {}
    if proc == "GetShopMarkup":
        if args[0] in SH:
            rows["shop"] = {"markup": SH[args[0]]["markup"]}
    elif proc == "GetItemShopRegenPCT":
        if args[0] in SH:
            rows["shop"] = SH[args[0]]
    else:
        if args[0] in IT:
            rows["item"] = IT[args[0]]
        if proc == "GetItemValue" and args[3] in SH:
            rows["shop"] = {"markup": SH[args[3]]["markup"]}
    d["rows"] = rows
    cases.append(d)


# One shop per shop type and per distinct markup, plus missing and out-of-Integer numbers.
pick, kinds = [], set()
for s in sorted(SH):
    k = (SH[s]["type"], SH[s]["markup"])
    if SH[s]["type"] not in kinds or SH[s]["markup"] not in kinds:
        pick.append(s)
    kinds.update((SH[s]["type"], SH[s]["markup"]))
for n in sorted(set(pick) | {0, 1, 2, 183, 185, -1, 5000, 32767, 32768, 40000}):
    add("markup", "GetShopMarkup", [n])

# Regen: every 6th stocked pair, slots without regen data, items stocked twice, type 8 shops.
multi = {}
for s in sorted(SH):
    cnt = {}
    for slot in SH[s]["stock"]:
        cnt[slot[0]] = cnt.get(slot[0], 0) + 1
    multi.update({(s, i): 1 for i, n in cnt.items() if n > 1})
nodata = [(s, sl[0]) for s in sorted(SH) for sl in SH[s]["stock"]
          if sl[1] > 0 and not (sl[2] > 0 and sl[3] > 0 and sl[4] > 0)][:25]
t8 = [(s, SH[s]["stock"][0][0]) for s in sorted(SH) if SH[s]["type"] == 8 and SH[s]["stock"]][:4]
sel = sorted(set(pairs[::6]) | set(nodata) | set(multi) | set(t8))
for s, i in sel:
    add("regen", "GetItemShopRegenPCT", [s, i])
for s in sorted(SH)[:12]:
    add("regen_miss", "GetItemShopRegenPCT", [s, 2047])
    add("regen_miss", "GetItemShopRegenPCT", [s, 0])
for s, i in [(0, 5), (-1, 5), (5, -1), (999, 5), (2, 1), (40000, 3)]:
    add("regen_edge", "GetItemShopRegenPCT", [s, i])

# Copper cost: every 8th item, the extremes of each currency, and missing rows.
ext = set()
for cu in range(5):
    ns = [n for n in items if IT[n]["currency"] == cu]
    if ns:
        ext |= {ns[0], ns[-1], max(ns, key=lambda n: IT[n]["price"])}
ext |= set([n for n in items if IT[n]["price"] == 0][:3])
for n in sorted(set(items[::8]) | ext | {-1, 0, 2047, 2048, 2050}):
    add("cost", "GetItemCopperCost", [n])

# Value at shops: every 12th stocked pair at charm 0, 50, 100.
for s, i in pairs[::12]:
    for ch in (0, 50, 100):
        add("val_shop", "GetItemValue", [i, ch, 0, s, False])
for s, i in pairs[::40]:
    add("val_shop_nobuy", "GetItemValue", [i, 37, 0, s, True])

# Value by item with a markup and charm grid, including the largest prices (overflow loop).
hi = [n for n in items if IT[n]["currency"] >= 3][::12]
base = sorted(set(items[::60]) | set(ext) | set(hi))
for i in items[::12]:
    for ch in (0, 50, 100):
        add("val_item", "GetItemValue", [i, ch, 0, 0, False])
for i in base:
    for mk in (0, 50, 200, 32760):
        for ch, nb in ((0, False), (100, False), (37, False), (37, True)):
            add("val_mk", "GetItemValue", [i, ch, mk, 0, nb])
for i in (-1, 0, 2047, 2048, 5000):
    add("val_edge", "GetItemValue", [i, 50, 100, 0, False])
for ch in (-5, 1, 2, 4, 5, 9, 10, 49, 51, 99, 101, 150, 255, 1000, 32767):
    for i in (3, 200, 1000, 2000):
        add("val_charm", "GetItemValue", [i, ch, 100, 0, False])
for s in (0, -1, 5, 999, 32768, 40000):
    for ch in (0, 50):
        add("val_shopedge", "GetItemValue", [10, ch, 100, s, False])

doc = {"family": "item_value", "database": True,
       "description": "Item value and shop price (FAM-18, stock): GetShopMarkup, GetItemShopRegenPCT, GetItemCopperCost, GetItemValue.",
       "notes": ["Generated by tools/oracle/gen/item_value.py. Cases are strided samples of the stock Items and Shops; 'rows' carries the fields the Rust port needs."],
       "tolerance": {"default": 0}, "cases": cases}
with open("fixtures/scenarios/item_value.json", "w") as f:
    json.dump(doc, f, separators=(",", ":"))
    f.write("\n")
print(len(cases), "cases", file=sys.stderr)

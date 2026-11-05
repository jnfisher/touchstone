import { describe, expect, it } from "vitest";
import { findNext, fitWidth, loadWidths, saveWidths, toTsv } from "./helpers";

const measure = (s: string) => s.length * 7;

describe("fitWidth", () => {
  it("fits the widest value, not just the header", () => {
    expect(fitWidth("Exp", ["1", "3,000,000"], measure)).toBeGreaterThanOrEqual(
      9 * 7 + 40,
    );
  });
  it("leaves room for the sort and filter icons on long headers", () => {
    expect(fitWidth("Lair Name", ["a"], measure)).toBeGreaterThanOrEqual(
      9 * 7 + 84,
    );
  });
  it("is capped and has a floor", () => {
    expect(fitWidth("x", ["y".repeat(500)], measure)).toBe(420);
    expect(fitWidth("", [], () => 0)).toBe(92);
  });
  it("samples big columns but still sees the first row", () => {
    const vals = Array.from({ length: 50000 }, (_, i) =>
      i === 0 ? "123456789" : "1",
    );
    expect(fitWidth("a", vals, measure)).toBeGreaterThanOrEqual(63 + 40);
  });
});

describe("widths storage", () => {
  it("round-trips per section and survives junk", () => {
    const m = new Map<string, string>();
    const st = {
      getItem: (k: string) => m.get(k) ?? null,
      setItem: (k: string, v: string) => void m.set(k, v),
    };
    saveWidths(st, "monsters", { name: 200 });
    expect(loadWidths(st, "monsters")).toEqual({ name: 200 });
    expect(loadWidths(st, "weapons")).toEqual({});
    m.set("touchstone.grid.widths.bad", '{"a":"x","b":-1,"c":5}');
    expect(loadWidths(st, "bad")).toEqual({ c: 5 });
    m.set("touchstone.grid.widths.bad", "not json");
    expect(loadWidths(st, "bad")).toEqual({});
    expect(loadWidths(undefined, "x")).toEqual({});
  });
});

describe("toTsv", () => {
  it("joins headers and rows with tabs and flattens embedded separators", () => {
    expect(
      toTsv(
        ["Name", "Exp"],
        [
          ["Sword", "3,000"],
          ["A\tB\nC", "1"],
        ],
      ),
    ).toBe("Name\tExp\r\nSword\t3,000\r\nA B C\t1");
  });
});

describe("findNext", () => {
  const names = ["Axe", "Bow", "Broad sword", "Club", "Bat"];
  it("finds a prefix match from a start index and wraps", () => {
    expect(findNext(names, "b", 0)).toBe(1);
    expect(findNext(names, "b", 2)).toBe(2);
    expect(findNext(names, "b", 3)).toBe(4);
    expect(findNext(names, "b", 5)).toBe(1);
    expect(findNext(names, "BR", 0)).toBe(2);
  });
  it("returns -1 for no match or empty query", () => {
    expect(findNext(names, "z", 0)).toBe(-1);
    expect(findNext(names, "", 0)).toBe(-1);
    expect(findNext([], "a", 0)).toBe(-1);
  });
});

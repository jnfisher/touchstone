import { describe, expect, it, vi } from "vitest";
import type { Item } from "./api";
import {
  boxNumber,
  createCharacterClient,
  CharacterError,
  derivedStatName,
  EQUIP_SLOTS,
  evalRequest,
  filterChoices,
  fromServer,
  exportUrl,
  importUrl,
  isMmecName,
  nameProblem,
  newCharacter,
  QUESTS,
  toChoices,
  equippableChoices,
  flaggedSlots,
} from "./character";

const item = (o: Partial<Item>): Item =>
  ({
    number: 1,
    name: "x",
    item_type: 0,
    worn: 0,
    in_game: true,
    ...o,
  }) as Item;

describe("character mapping", () => {
  it("has 20 slots with the weapon at 16 and 12 quests", () => {
    expect(EQUIP_SLOTS).toHaveLength(20);
    expect(EQUIP_SLOTS[16].label).toBe("Weapon");
    expect(QUESTS).toHaveLength(12);
    expect(
      QUESTS.filter((q) => q.option !== undefined).map((q) => q.index),
    ).toEqual([5, 9, 10, 11]);
  });

  it("does not evaluate until class and race are picked", () => {
    const c = newCharacter("A");
    const o = { hide_class_race: false, min_item_level: 0 };
    expect(evalRequest(c, o)).toBeUndefined();
    c.class = 1;
    expect(evalRequest(c, o)).toBeUndefined();
    c.race = 2;
    expect(evalRequest(c, o)).toMatchObject({ class: 1, race: 2, options: o });
  });

  it("pads short stored lists and keeps the rest", () => {
    const c = fromServer({
      name: "B",
      equipped: [5],
      bless: [7, 8],
      quests: [true],
    });
    expect(c.equipped).toHaveLength(20);
    expect(c.equipped[0]).toBe(5);
    expect(c.bless.slice(0, 3)).toEqual([7, 8, 0]);
    expect(c.quests[0]).toBe(true);
    expect(c.quests).toHaveLength(12);
    expect(c.attack.heal_rounds).toBe(30);
  });

  it("mirrors the server name rule", () => {
    for (const ok of ["Bob", "Mr. Bob", "Zoë"])
      expect(nameProblem(ok)).toBeUndefined();
    for (const bad of [
      "",
      ".x",
      "a/b",
      "a\\b",
      "a:b",
      "a.",
      "a ",
      " a",
      "a\nb",
      "x".repeat(65),
    ])
      expect(nameProblem(bad), JSON.stringify(bad)).toBeDefined();
  });

  it("reads boxes like Val and clamps to three digits", () => {
    expect(boxNumber("12abc")).toBe(12);
    expect(boxNumber("abc")).toBe(0);
    expect(boxNumber("5000")).toBe(999);
    expect(boxNumber("-4")).toBe(0);
  });

  it("names the derived stats", () => {
    expect(derivedStatName(2)).toBe("Armour Class");
    expect(derivedStatName(45)).toBe("Stat 45");
  });

  it("turns server pick lists into name-sorted combobox rows", () => {
    const rows = toChoices([
      { number: 7, label: "zed helm (7)" },
      { number: 2, label: "Alpha helm (2)" },
    ]);
    expect(rows.map((r) => r.number)).toEqual([2, 7]);
    expect(filterChoices(rows, "HEL ALP")).toHaveLength(1);
  });

  it("offers in-game armour with a slot and weapons to the equip-anywhere search", () => {
    const items = [
      item({ number: 1, name: "helm", worn: 2 }),
      item({ number: 2, name: "nowhere pants", worn: 0 }),
      item({ number: 3, name: "sword", item_type: 1 }),
      item({ number: 4, name: "hidden helm", worn: 2, in_game: false }),
      item({ number: 5, name: "charm", item_type: 10, worn: 3 }),
    ];
    expect(equippableChoices(items).map((c) => c.name)).toEqual([
      "helm",
      "sword",
    ]);
  });

  it("names the flagged slots", () => {
    const flags = Array<boolean>(20).fill(false);
    flags[15] = flags[16] = true;
    expect(flaggedSlots(flags)).toEqual(["Off-Hand", "Weapon"]);
  });
});

describe("character client", () => {
  const ok = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status });

  it("posts evaluate and surfaces the server reason", async () => {
    const f = vi
      .fn()
      .mockResolvedValueOnce(ok({ ok: 1 }))
      .mockResolvedValueOnce(
        new Response("pick a class and race", { status: 422 }),
      );
    const c = createCharacterClient(f);
    await c.evaluate({ a: 1 });
    expect(f.mock.calls[0][0]).toBe("/api/character/evaluate");
    expect(f.mock.calls[0][1].method).toBe("POST");
    await expect(c.evaluate({})).rejects.toMatchObject({
      status: 422,
      message: "pick a class and race",
    });
    await expect(c.evaluate({})).rejects.toBeInstanceOf(Error);
    void CharacterError;
  });

  it("posts equip and equip-state", async () => {
    const f = vi.fn().mockImplementation(() => Promise.resolve(ok({})));
    const c = createCharacterClient(f);
    await c.equip({ character: newCharacter("a"), item: 5, slot: 3 });
    expect(f.mock.calls[0][0]).toBe("/api/character/equip");
    expect(JSON.parse(f.mock.calls[0][1].body)).toMatchObject({
      item: 5,
      slot: 3,
    });
    await c.equipState(newCharacter("a"));
    expect(f.mock.calls[1][0]).toBe("/api/character/equip-state");
  });

  it("builds the bless query and encodes names", async () => {
    const f = vi.fn().mockImplementation(() => Promise.resolve(ok([])));
    const c = createCharacterClient(f);
    await c.blessSpells({
      class: 3,
      level: 10,
      alignment: 1,
      all: true,
      include: [0, 5, 6],
    });
    expect(f.mock.calls[0][0]).toBe(
      "/api/character/bless-spells?class=3&level=10&alignment=1&all=true&include=5%2C6",
    );
    await c.get("Mr. Bob/x");
    expect(f.mock.calls[1][0]).toBe("/api/characters/Mr.%20Bob%2Fx");
  });

  it("treats 204 as empty", async () => {
    const f = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    await expect(createCharacterClient(f).remove("a")).resolves.toBeUndefined();
  });
});

describe("mmec import and export", () => {
  it("builds urls", () => {
    expect(exportUrl("Zed the Bold")).toBe(
      "/api/characters/Zed%20the%20Bold/export.mmec",
    );
    expect(importUrl("C:\\a b.mmec")).toBe(
      "/api/characters/import?file=C%3A%5Ca+b.mmec",
    );
    expect(importUrl("x.mmec", true)).toBe(
      "/api/characters/import?file=x.mmec&overwrite=true",
    );
  });

  it("accepts only .mmec names", () => {
    expect(isMmecName("Char.MMEC")).toBe(true);
    expect(isMmecName("Char.json")).toBe(false);
  });

  it("posts the raw bytes and maps the answer; a taken name is a 409", async () => {
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    const fetchFn = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ name: "Imp", level: 9, extra: [] }), {
          status: 200,
        }),
      )
      .mockResolvedValueOnce(
        new Response('a character named "Imp" already exists', { status: 409 }),
      );
    const client = createCharacterClient(fetchFn);
    const file = { name: "Imp.mmec", arrayBuffer: async () => bytes };
    const doc = await client.importMmec(file);
    expect(doc.name).toBe("Imp");
    expect(doc.equipped).toHaveLength(20);
    expect(fetchFn.mock.calls[0][0]).toBe(
      "/api/characters/import?file=Imp.mmec",
    );
    expect(fetchFn.mock.calls[0][1]).toMatchObject({
      method: "POST",
      body: bytes,
    });
    await expect(client.importMmec(file)).rejects.toMatchObject({
      status: 409,
    });
  });
});

describe("find best menu", () => {
  it("lists MME's entries in the designer's order", async () => {
    const { FIND_BEST_MENU, findBestLabel, toggleExcluded } =
      await import("./character");
    expect(FIND_BEST_MENU.map((g) => g.label)).toEqual([
      "Armour",
      "Attacking",
      "Attributes",
      "Resistances",
      "Stats",
    ]);
    const all = FIND_BEST_MENU.flatMap((g) => [
      ...g.entries,
      ...(g.sub?.entries ?? []),
    ]);
    // 45 entries in MME's menu, minus the GreaterMUD-only VileWard
    expect(all).toHaveLength(44);
    expect(all[0]).toMatchObject({
      label: "AC/DR Combo",
      criterion: 0,
      index: 0,
    });
    expect(findBestLabel({ index: 5, criterion: 4 })).toBe(
      "Mystic Attacks: Punch DMG",
    );
    expect(findBestLabel({ index: 6, criterion: 0 })).toBe("");
    expect(toggleExcluded(null, 5)).toEqual([5]);
    expect(toggleExcluded([5, 6], 5)).toEqual([6]);
  });

  it("posts a find best to the server", async () => {
    const fetchMock = vi.fn(
      async () => new Response(JSON.stringify({ ok: 1 })),
    );
    const c = createCharacterClient(fetchMock as unknown as typeof fetch);
    await c.findBest({
      character: newCharacter("A"),
      next: true,
      excluded: null,
      last: { index: 0, criterion: 0 },
    });
    const first = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(first[0]).toBe("/api/character/find-best");
    expect(JSON.parse(first[1].body as string).next).toBe(true);
  });
});

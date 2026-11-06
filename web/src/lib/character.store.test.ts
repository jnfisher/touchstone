import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CharacterError,
  type CharacterClient,
  type CharacterDoc,
} from "./character";
import { newCharacter as newDoc } from "./character";
import {
  createCharacterStore,
  getCharacterStore,
  openLastCharacter,
} from "./character.svelte";

function fakeClient() {
  const files = new Map<string, CharacterDoc>();
  const client = {
    evaluate: vi.fn().mockResolvedValue({ hp: { caption: "ok" } }),
    blessSpells: vi.fn().mockResolvedValue([]),
    list: vi.fn(async () =>
      [...files.keys()].map((name) => ({
        name,
        race: null,
        class: null,
        level: 1,
      })),
    ),
    get: vi.fn(async (n: string) => structuredClone(files.get(n)!)),
    put: vi.fn(async (n: string, d: CharacterDoc) => {
      files.set(n, structuredClone({ ...d, name: n }));
      return d;
    }),
    remove: vi.fn(async (n: string) => void files.delete(n)),
    importMmec: vi.fn(),
  };
  return { client: client as unknown as CharacterClient, files, raw: client };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("character store", () => {
  it("debounces evaluation and skips it without class and race", async () => {
    const { client, raw } = fakeClient();
    const s = createCharacterStore(client);
    s.edit((d) => (d.level = 5));
    await vi.advanceTimersByTimeAsync(500);
    expect(raw.evaluate).not.toHaveBeenCalled();
    s.edit((d) => ((d.class = 1), (d.race = 1)));
    s.edit((d) => (d.level = 6));
    s.edit((d) => (d.level = 7));
    await vi.advanceTimersByTimeAsync(200);
    expect(raw.evaluate).toHaveBeenCalledTimes(1);
    expect(raw.evaluate.mock.calls[0][0]).toMatchObject({
      level: 7,
      class: 1,
      race: 1,
    });
    expect(s.evaluated).toEqual({ hp: { caption: "ok" } });
  });

  it("shows the server error inline and clears it on success", async () => {
    const { client, raw } = fakeClient();
    raw.evaluate.mockRejectedValueOnce(new Error("level must be 0 to 999"));
    const s = createCharacterStore(client);
    s.edit((d) => ((d.class = 1), (d.race = 1)));
    await vi.advanceTimersByTimeAsync(200);
    expect(s.evalError).toBe("level must be 0 to 999");
    s.edit((d) => (d.level = 3));
    await vi.advanceTimersByTimeAsync(200);
    expect(s.evalError).toBeUndefined();
  });

  it("does not save without a valid name, then autosaves under it", async () => {
    const { client, raw, files } = fakeClient();
    const s = createCharacterStore(client);
    s.edit((d) => (d.level = 9));
    await vi.advanceTimersByTimeAsync(2000);
    expect(raw.put).not.toHaveBeenCalled();
    s.edit((d) => (d.name = "Bob"));
    await vi.advanceTimersByTimeAsync(1000);
    expect(raw.put).toHaveBeenCalledTimes(1);
    expect(files.get("Bob")?.level).toBe(9);
    expect(s.saveStatus).toBe("saved");
    expect(s.saved.map((x) => x.name)).toEqual(["Bob"]);
  });

  it("removes the old file on rename", async () => {
    const { client, files } = fakeClient();
    const s = createCharacterStore(client);
    s.edit((d) => (d.name = "Bob"));
    await s.flush();
    s.edit((d) => (d.name = "Rob"));
    await s.flush();
    expect([...files.keys()]).toEqual(["Rob"]);
  });

  it("reports a failed save", async () => {
    const { client, raw } = fakeClient();
    raw.put.mockRejectedValueOnce(new Error("disk full"));
    const s = createCharacterStore(client);
    s.edit((d) => (d.name = "Bob"));
    await s.flush();
    expect(s.saveStatus).toBe("error");
    expect(s.saveError).toBe("disk full");
  });

  it("loads, starts new and deletes", async () => {
    const { client, files } = fakeClient();
    const s = createCharacterStore(client);
    s.edit((d) => ((d.name = "Bob"), (d.level = 4)));
    await s.flush();
    s.startNew();
    expect(s.doc.name).toBe("");
    await s.load("Bob");
    expect(s.doc.level).toBe(4);
    expect(s.storedName).toBe("Bob");
    await s.remove();
    expect(files.size).toBe(0);
    expect(s.doc.name).toBe("");
    expect(s.saved).toEqual([]);
  });
});

describe("applying a pasted character", () => {
  it("keeps the old file when the pasted name differs and saves the new one beside it", async () => {
    const { client, files, raw } = fakeClient();
    const s = createCharacterStore(client);
    s.set({ name: "Old", level: 3 });
    await s.flush();
    expect([...files.keys()]).toEqual(["Old"]);
    await s.applyPasted({ ...newDoc("Zed"), level: 14 });
    await vi.advanceTimersByTimeAsync(1000);
    expect([...files.keys()].sort()).toEqual(["Old", "Zed"]);
    expect(raw.remove).not.toHaveBeenCalled();
    expect(s.doc.level).toBe(14);
  });

  it("overwrites the same character when the name matches", async () => {
    const { client, files } = fakeClient();
    const s = createCharacterStore(client);
    s.set({ name: "Zed", level: 3 });
    await s.flush();
    await s.applyPasted({ ...newDoc("Zed"), level: 20 });
    await vi.advanceTimersByTimeAsync(1000);
    expect([...files.keys()]).toEqual(["Zed"]);
    expect(files.get("Zed")?.level).toBe(20);
  });
});

describe("replace epoch (the calculated monster damage is cleared when it moves)", () => {
  it("moves on a load, new and paste, not on an edit", async () => {
    const { client, raw } = fakeClient();
    raw.get.mockResolvedValue({ ...newDoc("Zed"), level: 3 });
    const s = createCharacterStore(client);
    const start = s.replaceEpoch;
    s.edit((d) => (d.level = 9));
    expect(s.replaceEpoch).toBe(start);
    s.startNew();
    expect(s.replaceEpoch).toBe(start + 1);
    await s.load("Zed");
    expect(s.replaceEpoch).toBe(start + 2);
    await s.applyPasted({ ...newDoc("Amy"), level: 4 });
    expect(s.replaceEpoch).toBe(start + 3);
  });
});

describe("shared store and opening the last character", () => {
  it("hands out one store", () => {
    expect(getCharacterStore()).toBe(getCharacterStore());
  });

  it("opens the remembered character once, and not over one being edited", async () => {
    const { client, files, raw } = fakeClient();
    files.set("Brisa", { name: "Brisa" } as CharacterDoc);
    files.set("Cog", { name: "Cog" } as CharacterDoc);
    vi.stubGlobal("localStorage", {
      getItem: () => "Cog",
      setItem: () => undefined,
    });
    try {
      const s = createCharacterStore(client);
      const [a, b] = await Promise.all([
        openLastCharacter(s),
        openLastCharacter(s),
      ]);
      expect(a).toBe("Cog");
      expect(b).toBe("Cog");
      expect(s.storedName).toBe("Cog");
      expect(raw.get).toHaveBeenCalledTimes(1);

      const editing = createCharacterStore(client);
      editing.edit((d) => (d.name = "Mine"));
      expect(await openLastCharacter(editing)).toBeUndefined();
      expect(editing.doc.name).toBe("Mine");
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("mmec import in the store", () => {
  const file = {
    name: "Imp.mmec",
    arrayBuffer: async () => new ArrayBuffer(0),
  };

  it("opens the imported character as saved", async () => {
    const { client, raw } = fakeClient();
    raw.importMmec.mockResolvedValue({ ...newDoc(), name: "Imp", level: 9 });
    const s = createCharacterStore(client);
    expect(await s.importMmec(file)).toBe("ok");
    expect(s.doc.name).toBe("Imp");
    expect(s.storedName).toBe("Imp");
    expect(s.saveStatus).toBe("saved");
  });

  it("reports a taken name and a failure", async () => {
    const { client, raw } = fakeClient();
    const s = createCharacterStore(client);
    raw.importMmec.mockRejectedValueOnce(new CharacterError(409, "exists"));
    expect(await s.importMmec(file)).toBe("exists");
    raw.importMmec.mockRejectedValueOnce(
      new CharacterError(422, "not an MMUD Explorer character file"),
    );
    expect(await s.importMmec(file)).toBe("error");
    expect(s.saveError).toContain("not an MMUD");
    raw.importMmec.mockResolvedValueOnce({ ...newDoc(), name: "Imp" });
    expect(await s.importMmec(file, true)).toBe("ok");
    expect(raw.importMmec).toHaveBeenLastCalledWith(file, true);
  });
});

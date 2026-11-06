import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createCharacterStore, openLastCharacter } from "./character.svelte";
import { newCharacter, type CharacterClient } from "./character";
import { autoLoadLastCharEnabled } from "./settings.svelte";

// Own file: the settings store the gate reads is shared for the page, so it loads once per file.
function characters() {
  return {
    evaluate: vi.fn().mockResolvedValue({}),
    blessSpells: vi.fn().mockResolvedValue([]),
    list: vi.fn(async () => [{ name: "Bob", race: null, class: null, level: 1 }]),
    get: vi.fn(async () => ({ ...newCharacter(), name: "Bob" })),
    put: vi.fn(),
    remove: vi.fn(),
    importMmec: vi.fn(),
  } as unknown as CharacterClient;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      Response.json({
        exp_per_hour: { global: {}, by_character: false },
        monster_sim_rounds: 500,
        route_waypoint_interval: 25,
        auto_load_last_char: false,
      }),
    ),
  );
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("auto-load setting", () => {
  it("keeps the last character closed when it is off", async () => {
    expect(await autoLoadLastCharEnabled()).toBe(false);
    const client = characters();
    const store = createCharacterStore(client);
    expect(await openLastCharacter(store)).toBeUndefined();
    expect(client.get).not.toHaveBeenCalled();
    // The saved list is still fetched so the Character tab can offer it.
    expect(store.saved.map((s) => s.name)).toEqual(["Bob"]);
  });
});

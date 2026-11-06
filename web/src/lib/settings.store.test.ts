import { describe, expect, it, vi } from "vitest";
import { createSettingsStore } from "./settings.svelte";
import {
  defaultBlock,
  defaultSettings,
  toDraft,
  type Settings,
  type SettingsClient,
} from "./settings";

function fake(initial: Settings = defaultSettings()) {
  let server = structuredClone(initial);
  const client = {
    get: vi.fn(async () => structuredClone(server)),
    getFor: vi.fn(),
    // The server answers with what it stored; the fake keeps the body as is.
    put: vi.fn(async (s: Settings) => {
      server = structuredClone(s);
      return structuredClone(s);
    }),
  };
  return { client: client as unknown as SettingsClient, raw: client };
}

describe("settings store", () => {
  it("shows MME's defaults until loaded, then the server's values", async () => {
    const s = defaultSettings();
    s.monster_sim_rounds = 750;
    const { client, raw } = fake(s);
    const store = createSettingsStore(client);
    expect(store.loaded).toBe(false);
    expect(store.current.monster_sim_rounds).toBe(500);
    await store.load();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.current.monster_sim_rounds).toBe(750);
    expect(raw.get).toHaveBeenCalledTimes(1);
    await store.reload();
    expect(raw.get).toHaveBeenCalledTimes(2);
  });

  it("keeps the defaults and the reason when the server cannot be read", async () => {
    const { client, raw } = fake();
    raw.get.mockRejectedValueOnce(new Error("HTTP 500"));
    const store = createSettingsStore(client);
    await store.load();
    expect(store.loaded).toBe(false);
    expect(store.error).toBe("HTTP 500");
    await store.load(); // a later call tries again
    expect(store.loaded).toBe(true);
    expect(store.error).toBeUndefined();
  });

  it("saves the global block when by-character is off", async () => {
    const { client, raw } = fake();
    const store = createSettingsStore(client);
    await store.load();
    const d = toDraft(store.current);
    d.block.model_d = false;
    d.knobs.knob_move = "2.5";
    const r = await store.save(d, true);
    expect(r).toEqual({ ok: true });
    const sent = raw.put.mock.calls[0][0];
    expect(sent.exp_per_hour.global.model_d).toBe(false);
    expect(sent.exp_per_hour.global.knob_move).toBe(2.5);
    expect(store.current.exp_per_hour.global.knob_move).toBe(2.5);
  });

  it("sends the block to the character and leaves the global one alone", async () => {
    const { client, raw } = fake();
    const store = createSettingsStore(client);
    await store.load();
    const d = toDraft(store.current);
    d.byCharacter = true;
    d.knobs.knob_dmg = "3";
    const r = await store.save(d, true);
    expect(r.ok).toBe(true);
    expect(r.characterBlock?.knob_dmg).toBe(3);
    const sent = raw.put.mock.calls[0][0];
    expect(sent.exp_per_hour.by_character).toBe(true);
    expect(sent.exp_per_hour.global).toEqual(defaultBlock());
  });

  it("writes the global block when by-character is on but no character is open", async () => {
    const { client, raw } = fake();
    const store = createSettingsStore(client);
    await store.load();
    const d = toDraft(store.current);
    d.byCharacter = true;
    d.knobs.knob_dmg = "3";
    const r = await store.save(d, false);
    expect(r).toEqual({ ok: true });
    expect(raw.put.mock.calls[0][0].exp_per_hour.global.knob_dmg).toBe(3);
  });

  it("reports a failed save and keeps the old values", async () => {
    const { client, raw } = fake();
    raw.put.mockRejectedValueOnce(new Error("settings: disk full"));
    const store = createSettingsStore(client);
    await store.load();
    const d = toDraft(store.current);
    d.simRounds = "900";
    expect(await store.save(d, false)).toEqual({ ok: false });
    expect(store.error).toBe("settings: disk full");
    expect(store.current.monster_sim_rounds).toBe(500);
    expect(store.saving).toBe(false);
  });
});

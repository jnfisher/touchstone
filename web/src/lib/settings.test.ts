import { describe, expect, it, vi } from "vitest";
import {
  SettingsError,
  clampKnob,
  clampSimRounds,
  clampWaypoint,
  createSettingsClient,
  defaultBlock,
  defaultSettings,
  draftBlock,
  draftSettings,
  filterNumberText,
  noModelTicked,
  shownModels,
  modelsHidden,
  QUICKSILVER_MODEL_NOTE,
  normalizeModels,
  predictKnob,
  predictSimRounds,
  predictWaypoint,
  resetExpFields,
  round2,
  toDraft,
  vbVal,
} from "./settings";

describe("VB number rules", () => {
  it("Val reads the leading number and ignores the rest", () => {
    expect(vbVal("1.5")).toBe(1.5);
    expect(vbVal("  2abc")).toBe(2);
    expect(vbVal(".5")).toBe(0.5);
    expect(vbVal("abc")).toBe(0);
    expect(vbVal("")).toBe(0);
    expect(vbVal("1e2")).toBe(100);
  });

  it("rounds half to even on two places", () => {
    expect(round2(1.234)).toBe(1.23);
    expect(round2(1.235)).toBe(1.24);
    expect(round2(1.245)).toBe(1.24);
    expect(round2(2.5)).toBe(2.5);
  });

  it("clamps like cmdSave_Click", () => {
    expect(clampKnob(0.004)).toBe(0.01);
    expect(clampKnob(0)).toBe(0.01);
    expect(clampKnob(12)).toBe(10);
    expect(clampKnob(1.234)).toBe(1.23);
    expect(clampSimRounds(5)).toBe(100);
    expect(clampSimRounds(99999)).toBe(10000);
    expect(clampSimRounds(500.9)).toBe(500);
    expect(clampWaypoint(0)).toBe(25);
    expect(clampWaypoint(-3)).toBe(25);
    expect(clampWaypoint(5000)).toBe(1000);
    expect(clampWaypoint(40)).toBe(40);
  });

  it("number boxes take digits and one point", () => {
    expect(filterNumberText("1.2.3a", true)).toBe("1.23");
    expect(filterNumberText("-4.5", false)).toBe("45");
  });
});

describe("live validation", () => {
  it("says nothing for a good value", () => {
    expect(predictKnob("1.5")).toEqual({ value: 1.5 });
    expect(predictSimRounds("500")).toEqual({ value: 500 });
    expect(predictWaypoint("25")).toEqual({ value: 25 });
  });

  it("explains what will be saved", () => {
    expect(predictKnob("12").note).toContain("saved as 10");
    expect(predictKnob("0").value).toBe(0.01);
    expect(predictKnob("").note).toContain("blank");
    expect(predictKnob("1.234").note).toContain("rounded");
    expect(predictSimRounds("20").note).toContain("saved as 100");
    expect(predictWaypoint("").note).toContain("25");
    expect(predictWaypoint("2000").note).toContain("1000");
  });
});

describe("models", () => {
  it("shows only Model D under the Quicksilver rules", () => {
    const stored = { ...defaultBlock(), model_a: true, model_b: true, model_d: false };
    const q = shownModels(true, stored);
    expect([q.model_a, q.model_b, q.model_c, q.model_d]).toEqual([
      false,
      false,
      false,
      true,
    ]);
    // stock shows what is stored, and nothing stored is changed
    expect(shownModels(false, stored)).toBe(stored);
    expect(stored.model_a).toBe(true);
    expect(QUICKSILVER_MODEL_NOTE).toContain("Model D");
  });

  it("says when the Quicksilver rules hide a ticked model", () => {
    const stored = defaultBlock();
    expect(modelsHidden(true, { ...stored, model_a: true })).toBe(true);
    expect(modelsHidden(true, { ...stored, model_a: false, model_b: false, model_c: false })).toBe(false);
    expect(modelsHidden(false, { ...stored, model_a: true })).toBe(false);
  });

  it("turns A, B and C on when none is ticked", () => {
    const none = {
      ...defaultBlock(),
      model_a: false,
      model_b: false,
      model_c: false,
      model_d: false,
    };
    expect(noModelTicked(none)).toBe(true);
    const n = normalizeModels(none);
    expect([n.model_a, n.model_b, n.model_c, n.model_d]).toEqual([
      true,
      true,
      true,
      false,
    ]);
    const d = { ...none, model_d: true };
    expect(normalizeModels(d)).toBe(d);
  });
});

describe("draft", () => {
  it("round-trips the defaults", () => {
    const s = defaultSettings();
    const d = toDraft(s);
    expect(draftBlock(d)).toEqual(s.exp_per_hour.global);
    expect(draftSettings(d, s.exp_per_hour.global)).toEqual(s);
  });

  it("carries Hide record numbers (off until ticked)", () => {
    const d = toDraft(defaultSettings());
    expect(d.hideNumbers).toBe(false);
    d.hideNumbers = true;
    expect(draftSettings(d, draftBlock(d)).hide_record_numbers).toBe(true);
  });

  it("applies the clamps on the way out", () => {
    const d = toDraft(defaultSettings());
    d.knobs.knob_dmg = "0.004";
    d.knobs.knob_xp = "";
    d.simRounds = "20";
    d.waypoint = "";
    const out = draftSettings(d, draftBlock(d));
    expect(out.exp_per_hour.global.knob_dmg).toBe(0.01);
    expect(out.exp_per_hour.global.knob_xp).toBe(0.01);
    expect(out.monster_sim_rounds).toBe(100);
    expect(out.route_waypoint_interval).toBe(25);
  });

  it("resets the exp/hr fields only", () => {
    const d = toDraft(defaultSettings());
    d.block.model_d = false;
    d.block.show_all = true;
    d.knobs.knob_move = "3";
    d.simRounds = "900";
    resetExpFields(d);
    expect(d.block).toEqual(defaultBlock());
    expect(d.knobs.knob_move).toBe("1");
    expect(d.simRounds).toBe("900");
  });
});

describe("client", () => {
  it("puts JSON and reports the server's reason", async () => {
    const fetchFn = vi.fn(async (_u: string, init?: RequestInit) =>
      init?.method === "PUT"
        ? new Response(init.body as string, { status: 200 })
        : new Response("settings: damaged", { status: 500 }),
    );
    const c = createSettingsClient(fetchFn as unknown as typeof fetch);
    const s = defaultSettings();
    expect(await c.put(s)).toEqual(s);
    expect(fetchFn.mock.calls[0][1]?.headers).toEqual({
      "content-type": "application/json",
    });
    await expect(c.get()).rejects.toMatchObject({
      status: 500,
      message: "settings: damaged",
    });
    await expect(c.get()).rejects.toBeInstanceOf(SettingsError);
  });
});

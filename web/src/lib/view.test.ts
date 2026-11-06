import { describe, expect, it } from "vitest";
import { VIEW_KEY, pageTitle, readView, writeView } from "./view";
import { createViewStore } from "./view.svelte";

const memory = (init: Record<string, string> = {}) => {
  const data = { ...init };
  return {
    data,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => void (data[k] = v),
  };
};

describe("view options", () => {
  it("default off, and ignore damaged storage", () => {
    expect(readView(undefined)).toEqual({ name_in_title: false });
    expect(readView(memory())).toEqual({ name_in_title: false });
    expect(readView(memory({ [VIEW_KEY]: "{oops" }))).toEqual({
      name_in_title: false,
    });
    expect(readView(memory({ [VIEW_KEY]: "7" }))).toEqual({
      name_in_title: false,
    });
    expect(readView(memory({ [VIEW_KEY]: '{"name_in_title":"yes"}' }))).toEqual(
      { name_in_title: false },
    );
  });

  it("round-trips through storage", () => {
    const m = memory();
    writeView(m, { name_in_title: true });
    expect(readView(m)).toEqual({ name_in_title: true });
  });

  it("survives storage that throws", () => {
    const boom = {
      getItem() {
        throw new Error("blocked");
      },
      setItem() {
        throw new Error("blocked");
      },
    };
    expect(readView(boom)).toEqual({ name_in_title: false });
    expect(() => writeView(boom, { name_in_title: true })).not.toThrow();
    const store = createViewStore(boom);
    store.set({ name_in_title: true });
    expect(store.options.name_in_title).toBe(true);
  });

  it("builds the page title", () => {
    expect(pageTitle({ name_in_title: true }, "Bob")).toBe("Bob - touchstone");
    expect(pageTitle({ name_in_title: true }, "  ")).toBe("touchstone");
    expect(pageTitle({ name_in_title: false }, "Bob")).toBe("touchstone");
  });
});

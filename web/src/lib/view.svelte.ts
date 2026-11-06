import { DEFAULT_VIEW, readView, writeView, type ViewOptions } from "./view";

export function createViewStore(
  storage: Pick<Storage, "getItem" | "setItem"> | undefined,
) {
  let options = $state<ViewOptions>(readView(storage));
  return {
    get options() {
      return options;
    },
    set(change: Partial<ViewOptions>) {
      options = { ...DEFAULT_VIEW, ...options, ...change };
      writeView(storage, options);
    },
  };
}

export type ViewStore = ReturnType<typeof createViewStore>;

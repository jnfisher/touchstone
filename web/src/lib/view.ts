// Options that only change how the page looks. They live in this browser's local storage (the
// theme has its own key, see theme.svelte.ts); every access is wrapped because storage can be
// blocked, and the page works without it.
export const VIEW_KEY = "touchstone.view";

export interface ViewOptions {
  /** MME's NameInTitle: put the open character's name in the page title. */
  name_in_title: boolean;
}

export const DEFAULT_VIEW: ViewOptions = { name_in_title: false };

export function readView(
  storage: Pick<Storage, "getItem"> | undefined,
): ViewOptions {
  try {
    const raw = storage?.getItem(VIEW_KEY);
    if (!raw) return { ...DEFAULT_VIEW };
    const v: unknown = JSON.parse(raw);
    if (typeof v !== "object" || v === null) return { ...DEFAULT_VIEW };
    return {
      name_in_title: (v as Partial<ViewOptions>).name_in_title === true,
    };
  } catch {
    return { ...DEFAULT_VIEW };
  }
}

export function writeView(
  storage: Pick<Storage, "setItem"> | undefined,
  v: ViewOptions,
): void {
  try {
    storage?.setItem(VIEW_KEY, JSON.stringify(v));
  } catch {
    // Private window or blocked storage: the choice lasts for this page only.
  }
}

/** The page title: `touchstone`, or `Name - touchstone` when the option is on and a name is set. */
export function pageTitle(view: ViewOptions, characterName: string): string {
  const name = characterName.trim();
  return view.name_in_title && name ? `${name} - touchstone` : "touchstone";
}

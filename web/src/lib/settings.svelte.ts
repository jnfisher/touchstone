// The server-side settings as the page sees them: loaded once, replaced by what a save returns.
// Saving follows MME's Settings window: with "by character" on and a character open, the exp/hr
// block goes to that character and the global block is left as it was.
import {
  createSettingsClient,
  defaultSettings,
  draftBlock,
  draftSettings,
  type Draft,
  type ExpHrBlock,
  type Settings,
  type SettingsClient,
} from "./settings";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export interface SaveResult {
  ok: boolean;
  /** Set when the exp/hr block belongs to the open character; the caller stores it there. */
  characterBlock?: ExpHrBlock;
}

export function createSettingsStore(
  client: SettingsClient = createSettingsClient(),
) {
  let settings = $state.raw<Settings>();
  let error = $state<string>();
  let saving = $state(false);
  let inflight: Promise<void> | undefined;

  function load(): Promise<void> {
    inflight ??= client
      .get()
      .then((s) => {
        settings = s;
        error = undefined;
      })
      .catch((e: unknown) => {
        error = message(e);
        // Let the next caller try again.
        inflight = undefined;
      });
    return inflight;
  }

  return {
    /** The saved settings, or MME's defaults until they have loaded. */
    get current(): Settings {
      return settings ?? defaultSettings();
    },
    get loaded() {
      return settings !== undefined;
    },
    get error() {
      return error;
    },
    get saving() {
      return saving;
    },
    load,
    /** Reload from the server (the dialog does this each time it opens). */
    async reload() {
      inflight = undefined;
      await load();
    },
    async save(draft: Draft, hasCharacter: boolean): Promise<SaveResult> {
      const perCharacter = draft.byCharacter && hasCharacter;
      const global = this.current.exp_per_hour.global;
      const body = draftSettings(draft, perCharacter ? global : draftBlock(draft));
      saving = true;
      try {
        settings = await client.put(body);
        error = undefined;
        return perCharacter
          ? { ok: true, characterBlock: draftBlock(draft) }
          : { ok: true };
      } catch (e) {
        error = message(e);
        return { ok: false };
      } finally {
        saving = false;
      }
    },
  };
}

export type SettingsStore = ReturnType<typeof createSettingsStore>;

let shared: SettingsStore | undefined;

export function getSettingsStore(): SettingsStore {
  return (shared ??= createSettingsStore());
}

/** `AutoLoadLastChar`; true (MME's default) when the settings cannot be read. */
export async function autoLoadLastCharEnabled(): Promise<boolean> {
  const s = getSettingsStore();
  await s.load();
  return s.loaded ? s.current.auto_load_last_char : true;
}

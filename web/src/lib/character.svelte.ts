// The Char tab's state: the character being edited, its last evaluation, and the saved list.
// Edits re-evaluate after a short pause and autosave under the character's name after a longer
// one. Timers are plain `setTimeout`s (no effects) so the store works, and tests, outside a
// component.
import {
  CharacterError,
  characterClient,
  evalRequest,
  fromServer,
  nameProblem,
  newCharacter,
  type CharacterClient,
  type CharacterDoc,
  type CharacterSummary,
  type EvalOptions,
  type EvalState,
} from "./character";
import { autoLoadLastCharEnabled } from "./settings.svelte";
import { getRealmStore, type RealmStore } from "./realm.svelte";
import { copyName, realmClient, type RealmClient } from "./realm";

export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface StoreOptions {
  evalDelay?: number;
  saveDelay?: number;
  /** Nothing is ever written to the server (the active realm character). */
  readOnly?: boolean;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function createCharacterStore(
  client: CharacterClient = characterClient,
  opts: StoreOptions = {},
) {
  const evalDelay = opts.evalDelay ?? 150;
  const saveDelay = opts.saveDelay ?? 700;

  let doc = $state<CharacterDoc>(newCharacter());
  let options = $state<EvalOptions>({
    hide_class_race: false,
    min_item_level: 0,
  });
  let evaluated = $state.raw<EvalState>();
  let evalError = $state<string>();
  let saved = $state.raw<CharacterSummary[]>([]);
  let saveStatus = $state<SaveStatus>("idle");
  let saveError = $state<string>();
  /** Counts the times a different character was opened (load, new, import, delete), so the Item
   * Manager can rebuild its list as LoadCharacter does; a paste or an edit does not count. */
  let epoch = $state(0);
  /** Counts every time the open character was replaced wholesale (a load, a paste, new, import, delete), as MME's
   * LoadCharacter and paste clear the calculated monster damage; an edit does not count. */
  let replaced = $state(0);
  /** The name the character is stored under on the server, if it is stored. */
  let storedName: string | undefined;

  let evalTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let evalTicket = 0;
  let saveChain: Promise<void> = Promise.resolve();

  function scheduleEval() {
    clearTimeout(evalTimer);
    evalTimer = setTimeout(runEval, evalDelay);
  }

  async function runEval() {
    const req = evalRequest($state.snapshot(doc), $state.snapshot(options));
    const mine = ++evalTicket;
    if (!req) {
      evaluated = undefined;
      evalError = undefined;
      return;
    }
    try {
      const out = await client.evaluate(req);
      if (mine !== evalTicket) return;
      evaluated = out;
      evalError = undefined;
    } catch (e) {
      if (mine !== evalTicket) return;
      evalError = message(e);
    }
  }

  function scheduleSave() {
    clearTimeout(saveTimer);
    if (opts.readOnly) return;
    if (nameProblem(doc.name)) {
      saveStatus = "idle";
      return;
    }
    saveTimer = setTimeout(() => void flush(), saveDelay);
  }

  /** Saves now. Saves are serialised so an older PUT never lands after a newer one. */
  function flush(): Promise<void> {
    clearTimeout(saveTimer);
    if (opts.readOnly) return saveChain;
    saveChain = saveChain.then(async () => {
      const snapshot = $state.snapshot(doc);
      if (nameProblem(snapshot.name)) return;
      saveStatus = "saving";
      try {
        await client.put(snapshot.name, snapshot);
        // A rename leaves the old file behind unless it is removed.
        if (storedName && storedName !== snapshot.name) {
          await client.remove(storedName).catch(() => undefined);
        }
        storedName = snapshot.name;
        saveStatus = "saved";
        saveError = undefined;
        saved = await client.list();
      } catch (e) {
        saveStatus = "error";
        saveError = message(e);
      }
    });
    return saveChain;
  }

  function replace(next: CharacterDoc, stored: string | undefined) {
    clearTimeout(saveTimer);
    replaced++;
    doc = next;
    storedName = stored;
    saveStatus = "idle";
    saveError = undefined;
    scheduleEval();
  }

  return {
    get doc() {
      return doc;
    },
    get options() {
      return options;
    },
    get evaluated() {
      return evaluated;
    },
    get evalError() {
      return evalError;
    },
    get saved() {
      return saved;
    },
    get saveStatus() {
      return saveStatus;
    },
    get saveError() {
      return saveError;
    },
    get nameProblem() {
      return nameProblem(doc.name);
    },
    get storedName() {
      return storedName;
    },
    get loadEpoch() {
      return epoch;
    },
    get replaceEpoch() {
      return replaced;
    },
    /** Apply an edit to the character; re-evaluates and autosaves. */
    edit(change: (d: CharacterDoc) => void) {
      change(doc);
      scheduleEval();
      scheduleSave();
    },
    /** View toggles are not saved with the character. */
    setOptions(change: (o: EvalOptions) => void) {
      change(options);
      scheduleEval();
    },
    async refreshList() {
      try {
        saved = await client.list();
      } catch (e) {
        saveError = message(e);
      }
    },
    startNew() {
      epoch++;
      replace(newCharacter(), undefined);
    },
    async load(name: string) {
      await flush();
      try {
        const next = await client.get(name);
        epoch++;
        replace(next, name);
      } catch (e) {
        saveError = message(e);
      }
    },
    async remove() {
      clearTimeout(saveTimer);
      const name = storedName;
      if (name) {
        try {
          await client.remove(name);
        } catch (e) {
          saveError = message(e);
          return;
        }
      }
      epoch++;
      replace(newCharacter(), undefined);
      saved = await client.list().catch(() => saved);
    },
    /**
     * Imports an `.mmec` file as a saved character and opens it. `exists` means the name is
     * taken; ask, then call again with `overwrite`.
     */
    async importMmec(
      file: Parameters<CharacterClient["importMmec"]>[0],
      overwrite = false,
    ): Promise<"ok" | "exists" | "error"> {
      await flush();
      try {
        const next = await client.importMmec(file, overwrite);
        epoch++;
        replace(next, next.name);
        saveStatus = "saved";
        saved = await client.list().catch(() => saved);
        return "ok";
      } catch (e) {
        if (e instanceof CharacterError && e.status === 409) return "exists";
        saveError = message(e);
        return "error";
      }
    },
    /**
     * Takes a pasted character as the open one. The previous character is saved first (MME saves
     * before a paste). A pasted name that differs from the stored one is a different character:
     * the old file stays, the new one is saved beside it.
     */
    async applyPasted(next: CharacterDoc) {
      await flush();
      const keep = next.name !== "" && next.name === storedName ? storedName : undefined;
      replace(fromServer(next), keep);
      scheduleSave();
    },
    /** Replace the document wholesale (tests, imports). */
    set(next: Partial<CharacterDoc>) {
      replace(fromServer(next), undefined);
    },
    flush,
    evaluateNow: runEval,
  };
}

export type CharacterStore = ReturnType<typeof createCharacterStore>;

// ---------- the shared stores ----------
// The editor (the Sandbox) is one store; a realm character the header picked is held in a second, read-only
// one. The rest of the app reads `getCharacterStore()`, a facade with the same members that shows whichever
// is active, as MME's windows all read the main form. Created on first use; they live for the page.

let sandbox: CharacterStore | undefined;

/** The Sandbox editor's own store (the Characters tab's second view). */
export function getSandboxStore(): CharacterStore {
  return (sandbox ??= createCharacterStore());
}

export type ActiveChoice = { kind: "sandbox" } | { kind: "realm"; id: number };

export const ACTIVE_KEY = "touchstone.active-character";

export function parseChoice(text: string | null | undefined): ActiveChoice {
  const m = /^realm:(\d+)$/.exec(text ?? "");
  return m ? { kind: "realm", id: Number(m[1]) } : { kind: "sandbox" };
}

export interface ActiveDeps {
  sandbox: CharacterStore;
  /** The read-only store that holds the realm character. */
  live: CharacterStore;
  realm: Pick<RealmStore, "load" | "refresh" | "available" | "characters">;
  client: Pick<RealmClient, "input">;
  storage?: Pick<Storage, "getItem" | "setItem">;
}

const store_ = (s?: ActiveDeps["storage"]) => {
  try {
    return s ?? localStorage;
  } catch {
    return undefined;
  }
};

/** A copy name that is free among the saved characters (see `copyName`). */
export async function copyToSandbox(
  sb: CharacterStore,
  input: CharacterDoc,
): Promise<string> {
  await sb.refreshList();
  const name = copyName(
    input.name,
    sb.saved.map((s) => s.name),
  );
  await sb.applyPasted({ ...input, name });
  return name;
}

export function createActiveCharacter(deps: ActiveDeps) {
  const { sandbox: sb, live, realm, client } = deps;
  let choice = $state<ActiveChoice>(
    (() => {
      try {
        return parseChoice(store_(deps.storage)?.getItem(ACTIVE_KEY));
      } catch {
        return { kind: "sandbox" } as ActiveChoice;
      }
    })(),
  );
  let switches = $state(0);
  let inputError = $state<string>();
  let applied: string | undefined;
  let ticket = 0;

  const current = (): CharacterStore => (choice.kind === "realm" ? live : sb);

  function persist() {
    try {
      store_(deps.storage)?.setItem(
        ACTIVE_KEY,
        choice.kind === "realm" ? `realm:${choice.id}` : "sandbox",
      );
    } catch {
      // private window or blocked storage: the choice just is not remembered
    }
  }

  /** Reads the realm character's input; unchanged input leaves the open copy (and its edits) alone. */
  async function loadInput() {
    if (choice.kind !== "realm") return;
    const id = choice.id;
    const mine = ++ticket;
    try {
      const input = await client.input(id);
      if (mine !== ticket) return;
      const key = JSON.stringify(input);
      if (key !== applied) {
        applied = key;
        live.set(input);
      }
      inputError = undefined;
    } catch (e) {
      if (mine !== ticket) return;
      inputError = message(e);
    }
  }

  function select(next: ActiveChoice) {
    choice = next;
    switches++;
    persist();
    ticket++;
    applied = undefined;
    if (next.kind === "realm") {
      live.startNew();
      void loadInput();
    }
  }

  /** The persisted choice is only kept while the realm has that character. */
  function validate() {
    if (choice.kind !== "realm" || !realm.available) {
      if (choice.kind === "realm" && !realm.available) select({ kind: "sandbox" });
      return;
    }
    const id = choice.id;
    if (!realm.characters.some((c) => c.id === id)) select({ kind: "sandbox" });
  }

  const facade: CharacterStore = {
    get doc() {
      return current().doc;
    },
    get options() {
      return current().options;
    },
    get evaluated() {
      return current().evaluated;
    },
    get evalError() {
      return current().evalError;
    },
    get saved() {
      return sb.saved;
    },
    get saveStatus() {
      return current().saveStatus;
    },
    get saveError() {
      return current().saveError;
    },
    get nameProblem() {
      return current().nameProblem;
    },
    get storedName() {
      return current().storedName;
    },
    get loadEpoch() {
      return sb.loadEpoch + live.loadEpoch + switches;
    },
    get replaceEpoch() {
      return sb.replaceEpoch + live.replaceEpoch + switches;
    },
    edit: (change) => current().edit(change),
    setOptions: (change) => current().setOptions(change),
    refreshList: () => sb.refreshList(),
    // These replace the whole character: they are the editor's, and using one makes the sandbox the
    // active character.
    startNew() {
      sb.startNew();
      select({ kind: "sandbox" });
    },
    async load(name) {
      await sb.load(name);
      select({ kind: "sandbox" });
    },
    async remove() {
      await sb.remove();
    },
    async importMmec(file, overwrite) {
      const r = await sb.importMmec(file, overwrite);
      if (r === "ok") select({ kind: "sandbox" });
      return r;
    },
    async applyPasted(next) {
      await sb.applyPasted(next);
      select({ kind: "sandbox" });
    },
    set(next) {
      sb.set(next);
      select({ kind: "sandbox" });
    },
    flush: () => sb.flush(),
    evaluateNow: () => current().evaluateNow(),
  };

  return {
    /** The store the rest of the app reads. */
    store: facade,
    get choice() {
      return choice;
    },
    /** The realm character's id when one is active. */
    get realmId() {
      return choice.kind === "realm" ? choice.id : undefined;
    },
    get inputError() {
      return inputError;
    },
    select,
    /** Reads the roster, then resolves the remembered choice against it. */
    async start() {
      await realm.load();
      validate();
      await loadInput();
    },
    /** The server reloaded: the roster, the open sheet and the active character are read again. */
    async sync() {
      await realm.refresh();
      validate();
      await loadInput();
    },
  };
}

export type ActiveCharacter = ReturnType<typeof createActiveCharacter>;

let active: ActiveCharacter | undefined;

export function getActiveCharacter(): ActiveCharacter {
  return (active ??= createActiveCharacter({
    sandbox: getSandboxStore(),
    live: createCharacterStore(undefined, { readOnly: true }),
    realm: getRealmStore(),
    client: realmClient,
  }));
}

/** What the rest of the app reads as "the character". */
export function getCharacterStore(): CharacterStore {
  return getActiveCharacter().store;
}

const LAST_KEY = "touchstone.character.last";

/** Remembers which saved character was open last, for the next visit. */
export function rememberLastCharacter(name: string | undefined) {
  try {
    if (name) localStorage.setItem(LAST_KEY, name);
  } catch {
    // private window or blocked storage: the app still works, it just starts empty
  }
}

const sandboxOf = (s: CharacterStore): CharacterStore =>
  s === active?.store ? getSandboxStore() : s;

const opened = new WeakMap<CharacterStore, Promise<string | undefined>>();

/**
 * Opens the character that was open last (or the first saved one) unless one is already being
 * edited. Runs once per store; resolves to the name opened.
 */
export function openLastCharacter(
  given: CharacterStore,
): Promise<string | undefined> {
  // the saved characters are the sandbox's, whichever character is active
  const store = sandboxOf(given);
  let p = opened.get(store);
  if (!p) {
    p = (async () => {
      await store.refreshList();
      // Settings > "load the last character at start" (MME's AutoLoadLastChar).
      if (!(await autoLoadLastCharEnabled())) return undefined;
      let last: string | null = null;
      try {
        last = localStorage.getItem(LAST_KEY);
      } catch {
        // see rememberLastCharacter()
      }
      const names = store.saved.map((s) => s.name);
      const name = last && names.includes(last) ? last : names[0];
      if (name && store.doc.name === "" && !store.storedName) {
        await store.load(name);
        return name;
      }
      return undefined;
    })();
    opened.set(store, p);
  }
  return p;
}

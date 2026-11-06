// The Item Manager's page state: the selection, a busy flag and the last message. The rows
// themselves live in the paste store (the paste dialog fills them), so a paste and the Item
// Manager always look at one list. Every change goes to the server, which answers the new rows and
// the character with its saved `carried`/`stash` lists rebuilt; those are written into the
// character (autosave, re-evaluation) when they differ.
import type { CharacterStore } from "./character.svelte";
import {
  invertSelection,
  itemManagerClient,
  runWithQuestions,
  type AskFn,
  type ImRequest,
  type ImResponse,
  type ItemManagerClient,
} from "./itemmanager";
import { getPasteStore, type PasteStore } from "./paste.svelte";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export interface ItemManagerOptions {
  paste?: PasteStore;
  client?: ItemManagerClient;
  /** Writes text to the clipboard; defaults to the browser's. */
  writeClipboard?: (text: string) => Promise<void>;
}

export function createItemManager(char: CharacterStore, opts: ItemManagerOptions = {}) {
  const paste = opts.paste ?? getPasteStore();
  const client = opts.client ?? itemManagerClient;
  const write = opts.writeClipboard ?? ((t: string) => navigator.clipboard.writeText(t));

  let selected = $state.raw<number[]>([]);
  let busy = $state(false);
  let note = $state<string>();
  let problem = $state<string>();

  function base(): ImRequest {
    return {
      character: $state.snapshot(char.doc),
      rows: paste.rows,
      selected: [...selected],
    };
  }

  function takeCharacter(res: ImResponse) {
    const next = res.character;
    const d = char.doc;
    const same =
      JSON.stringify(d.carried ?? []) === JSON.stringify(next.carried ?? []) &&
      JSON.stringify(d.stash ?? []) === JSON.stringify(next.stash ?? []) &&
      d.add_weight === next.add_weight;
    if (same) return;
    char.edit((c) => {
      c.carried = next.carried ?? [];
      c.stash = next.stash ?? [];
      c.add_weight = next.add_weight;
    });
  }

  function apply(res: ImResponse) {
    paste.setRows(res.rows);
    selected = res.selected;
    note = res.messages.length ? res.messages.join(" ") : undefined;
    takeCharacter(res);
  }

  async function guarded(fn: () => Promise<void>) {
    busy = true;
    problem = undefined;
    try {
      await fn();
    } catch (e) {
      problem = message(e);
    } finally {
      busy = false;
    }
  }

  return {
    get rows() {
      return paste.rows;
    },
    get selected() {
      return selected;
    },
    get busy() {
      return busy;
    },
    get note() {
      return note;
    },
    get problem() {
      return problem;
    },
    select(next: number[]) {
      selected = next;
    },
    /** `LoadCharacter`'s list: rebuilt from the saved carried and stash lists. */
    reload() {
      return guarded(async () => {
        const res = await client.rows({ character: $state.snapshot(char.doc) });
        paste.setRows(res.rows);
        selected = [];
        note = undefined;
      });
    },
    /** An Action button (`LV_SetActionCell`). */
    action(action: number, ask: AskFn) {
      if (selected.length === 0) return Promise.resolve();
      return guarded(async () => {
        const res = await runWithQuestions((answers) => client.action({ ...base(), action, answers }), ask);
        if (res) apply(res);
      });
    },
    /** `LV_DeleteSelected`; more than one row is confirmed first. */
    remove(ask: AskFn) {
      if (selected.length === 0) return Promise.resolve();
      return guarded(async () => {
        const res = await runWithQuestions((answers) => client.remove({ ...base(), answers }), ask);
        if (res) apply(res);
      });
    },
    /** `LV_InvertSelection` (nothing for the server to decide). */
    invert() {
      selected = invertSelection(selected, paste.rows.length);
    },
    selectAll() {
      selected = paste.rows.map((_, i) => i);
    },
    /** `LV_CopySelectedActionsToClipboard`. */
    copy() {
      return guarded(async () => {
        const { text } = await client.copy(base());
        if (text === null || text === "") {
          note = "Nothing to copy: flag the selected rows first.";
          return;
        }
        await write(text);
        note = `Copied ${text.split("\r\n").length} command${text.includes("\r\n") ? "s" : ""} to the clipboard.`;
      });
    },
    /** "Add to Item Manager": a getable item as a MANUAL row. */
    add(item: number) {
      return guarded(async () => {
        apply(await client.add({ ...base(), item }));
      });
    },
  };
}

export type ItemManager = ReturnType<typeof createItemManager>;

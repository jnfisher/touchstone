// What the Paste dialog leaves for the rest of the app: the Item Manager rows of the last paste
// (until the Item Manager tab exists) and the party filter values Continue produced, which the
// monster filters read. Plain session state; nothing here is saved.
import type { ItemRow, PartyFilter } from "./paste";

export function createPasteStore() {
  let rows = $state.raw<ItemRow[]>([]);
  let filter = $state.raw<PartyFilter | undefined>();
  return {
    get rows() {
      return rows;
    },
    get partyFilter() {
      return filter;
    },
    setRows(next: ItemRow[]) {
      rows = next;
    },
    setPartyFilter(next: PartyFilter | undefined) {
      filter = next;
    },
  };
}

export type PasteStore = ReturnType<typeof createPasteStore>;

let shared: PasteStore | undefined;
export function getPasteStore(): PasteStore {
  return (shared ??= createPasteStore());
}

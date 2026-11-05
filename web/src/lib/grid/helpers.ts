// Pure helpers for DataGrid, kept free of AG Grid and the DOM so Vitest can cover them.

export type Measure = (text: string) => number;

/** Room for cell padding, and for the sort arrow + filter button beside a header label. */
const CELL_PAD = 40;
const HEADER_PAD = 92;
const MIN_WIDTH = 48;
const MAX_WIDTH = 420;
/** Rows measured per column; rooms has tens of thousands and sizing must stay instant. */
const SAMPLE = 3000;

/** Width that fits the header and the widest value (not only the rows currently rendered). */
export function fitWidth(
  header: string,
  values: readonly string[],
  measure: Measure,
): number {
  let w = measure(header) + HEADER_PAD;
  const step = Math.max(1, Math.ceil(values.length / SAMPLE));
  for (let i = 0; i < values.length; i += step)
    w = Math.max(w, measure(values[i]) + CELL_PAD);
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.ceil(w)));
}

export type Widths = Record<string, number>;

export function widthsKey(section: string): string {
  return `touchstone.grid.widths.${section}`;
}

export function loadWidths(
  storage: Pick<Storage, "getItem"> | undefined,
  section: string,
): Widths {
  try {
    const raw = storage?.getItem(widthsKey(section));
    const v: unknown = raw ? JSON.parse(raw) : {};
    if (!v || typeof v !== "object") return {};
    return Object.fromEntries(
      Object.entries(v).filter(
        ([, w]) => typeof w === "number" && Number.isFinite(w) && w > 0,
      ),
    ) as Widths;
  } catch {
    return {};
  }
}

export function saveWidths(
  storage: Pick<Storage, "setItem"> | undefined,
  section: string,
  w: Widths,
) {
  try {
    storage?.setItem(widthsKey(section), JSON.stringify(w));
  } catch {
    // Storage can be full or blocked; widths are a convenience.
  }
}

/** Tab-separated text with a header line, as MME's copy-to-clipboard produces. */
export function toTsv(
  headers: readonly string[],
  rows: readonly (readonly string[])[],
): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  return [headers, ...rows].map((r) => r.map(clean).join("\t")).join("\r\n");
}

/**
 * Index of the next name starting with `query` (case-insensitive), searching from `from`
 * (inclusive) and wrapping round; -1 when nothing matches.
 */
export function findNext(
  names: readonly string[],
  query: string,
  from: number,
): number {
  const q = query.toLowerCase();
  if (!q || names.length === 0) return -1;
  for (let k = 0; k < names.length; k++) {
    const i = (Math.max(0, from) + k) % names.length;
    if (names[i].toLowerCase().startsWith(q)) return i;
  }
  return -1;
}

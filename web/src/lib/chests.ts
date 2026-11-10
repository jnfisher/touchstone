// The Chests tab: every chest item, what it can give and for how much, and who drops it. The numbers come from
// `/api/chests` (the server prices at the charm in the request); this module holds the wire types, the client,
// and the grids' column definitions. Money is worded the way MME words a price: the short form ("12G") in the
// grid, the full "N Copper (M Coin)" text on hover.
import type { ColDef } from 'ag-grid-community';
import type { RefRow, RefTarget } from './detail';

export const DEFAULT_CHARM = 50;
export const MAX_CHARM = 32767;

export interface ChestSummary {
	item: number;
	name: string;
	/** How many different items it can give. */
	contents: number;
	average_copper: number;
	average: string;
	average_short: string;
	dropped_by: number;
}
export interface ChestList {
	charm: number;
	chests: ChestSummary[];
}
export interface ChestContent {
	item: number;
	name: string;
	percent: number;
	percent_text: string;
	sellable: boolean;
	sell_copper: number;
	sell: string;
	sell_short: string;
	expected_copper: number;
	expected: string;
	expected_short: string;
}
export interface ChestDetail {
	item: number;
	name: string;
	charm: number;
	contents: ChestContent[];
	average_copper: number;
	average: string;
	average_short: string;
	droppers: RefRow[];
}

/** The charm box's text as a charm: a whole number from 0 to 32767, else undefined (the box keeps the old charm). */
export function parseCharm(text: string): number | undefined {
	const t = text.trim();
	if (!/^\d{1,5}$/.test(t)) return undefined;
	const n = Number(t);
	return n <= MAX_CHARM ? n : undefined;
}

type FetchFn = typeof fetch;

export class ChestsError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
		this.name = 'ChestsError';
	}
}

export function createChestsClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function read<T>(res: Response): Promise<T> {
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new ChestsError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	return {
		async list(charm: number, signal?: AbortSignal): Promise<ChestList> {
			return read(await fetchFn(`/api/chests?charm=${charm}`, { signal }));
		},
		async detail(item: number, charm: number, signal?: AbortSignal): Promise<ChestDetail> {
			return read(await fetchFn(`/api/chests/${item}?charm=${charm}`, { signal }));
		}
	};
}
export type ChestsClient = ReturnType<typeof createChestsClient>;
export const chestsClient = createChestsClient();

// ---------- the grids ----------

const money = { type: 'rightAligned', filter: 'agNumberColumnFilter' } as const;
const plain = { type: 'rightAligned', filter: false } as const;

/** The chest list: name, number, contents, average sale per opening, dropped by. */
export function summaryColumns(): ColDef<ChestSummary>[] {
	return [
		{ colId: 'name', headerName: 'Name', field: 'name', maxWidth: 230 },
		{ colId: 'number', headerName: 'No.', field: 'item', ...plain },
		{ colId: 'contents', headerName: 'Contents', field: 'contents', type: 'rightAligned', filter: 'agNumberColumnFilter' },
		{
			colId: 'average',
			headerName: 'Avg sale',
			valueGetter: (p) => p.data?.average_copper,
			valueFormatter: (p) => p.data?.average_short ?? '',
			tooltipValueGetter: (p) => p.data?.average,
			...money
		},
		{ colId: 'dropped', headerName: 'Drops', field: 'dropped_by', type: 'rightAligned', filter: 'agNumberColumnFilter' }
	];
}

/** The contents grid: item, chance, sell price and expected value (chance times price). */
export function contentColumns(): ColDef<ChestContent>[] {
	return [
		{ colId: 'name', headerName: 'Item', field: 'name', filter: false, minWidth: 150, maxWidth: 220 },
		{
			colId: 'chance',
			headerName: 'Chance',
			valueGetter: (p) => p.data?.percent,
			valueFormatter: (p) => p.data?.percent_text ?? '',
			sort: 'desc',
			...plain
		},
		{
			colId: 'sell',
			headerName: 'Sells',
			valueGetter: (p) => p.data?.sell_copper,
			valueFormatter: (p) => (p.data ? (p.data.sellable ? p.data.sell_short : "can't be sold") : ''),
			tooltipValueGetter: (p) => p.data?.sell,
			// cellClass replaces the right-aligned type's own class, so it is repeated here
			cellClass: (p) => (p.data?.sellable ? 'ag-right-aligned-cell' : ['ag-right-aligned-cell', 'ts-cell-nomatch']),
			...plain
		},
		{
			colId: 'expected',
			headerName: 'Expected',
			valueGetter: (p) => p.data?.expected_copper,
			valueFormatter: (p) => p.data?.expected_short ?? '',
			tooltipValueGetter: (p) => p.data?.expected,
			...plain
		}
	];
}

/** One line of "Dropped by": a monster with its chance, or another source (room, textblock, NPC) with none. */
export interface DropperRow {
	id: string;
	/** `monster`, `room`, `textblock`, `npc`, ... (the reference row's kind). */
	kind: string;
	/** What it is, without the "Monster: " style lead. */
	name: string;
	/** The drop chance in percent; undefined for a source that has none. */
	percent?: number;
	percent_text: string;
	target: RefTarget | null;
}

const KIND_LABEL: Record<string, string> = {
	monster: 'Monster',
	room: 'Room',
	textblock: 'Textblock',
	npc: 'NPC',
	item: 'Item',
	spell: 'Spell',
	lair: 'Lair',
	group: 'Group'
};
export const kindLabel = (kind: string) => KIND_LABEL[kind] ?? kind;

/** The server's reference rows (percent in the row, the source in its first cell) as the grid's rows. */
export function dropperRows(refs: readonly RefRow[]): DropperRow[] {
	const out: DropperRow[] = [];
	refs.forEach((r, i) => {
		// the "... plus more" rows are notes, not sources
		if (r.kind === 'more' || r.kind === 'no_limit') return;
		const text = (r.cells[0]?.text ?? r.text).trim();
		const percent = r.text.trim() === '' ? undefined : Number.parseFloat(r.text);
		out.push({
			id: String(i),
			kind: r.kind,
			name: text.replace(/^[A-Za-z]+:? /, ''),
			percent: Number.isFinite(percent) ? percent : undefined,
			percent_text: r.text.trim(),
			target: r.target
		});
	});
	return out;
}

export function dropperColumns(): ColDef<DropperRow>[] {
	return [
		{ colId: 'kind', headerName: 'Source', filter: false, maxWidth: 110, valueGetter: (p) => (p.data ? kindLabel(p.data.kind) : '') },
		{ colId: 'name', headerName: 'Name', field: 'name', filter: false, minWidth: 240, maxWidth: 300 },
		{
			colId: 'chance',
			headerName: 'Drop chance',
			valueGetter: (p) => p.data?.percent,
			valueFormatter: (p) => p.data?.percent_text ?? '',
			...plain
		}
	];
}

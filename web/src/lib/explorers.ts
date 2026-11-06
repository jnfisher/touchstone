// What each MME explorer tab lists: which records belong, the raw columns in MME's order with
// MME's headers, the detail pane, and the computed columns that are still to come (`pending`).
// Column order and headers follow modMain's Add*2LV row builders; record filters follow the
// Filter* procedures in frmMain with MME's default "only in-game" setting.
import type { ClassRec, Item, Monster, Race, Shop, Spell, TableName, TableTypes } from './api';
import type { EvalTab } from './evaluate';
import type { SectionSlug } from './sections';

export type Cell = string | number;

export interface ColumnSpec<T> {
	id: string;
	header: string;
	width?: number;
	/** Right-aligned, numeric filter. */
	numeric?: boolean;
	/** Cell value; numbers sort and filter numerically. */
	get(rec: T): Cell;
	/** Display text when it differs from `String(get())`. */
	format?(v: Cell, rec: T): string;
	/** Numeric sort key for composite text such as "12/3". */
	sortKey?(rec: T): number;
}

export interface DetailRow {
	label: string;
	value: string;
	/** Free text that should wrap instead of staying on one line. */
	long?: boolean;
}
export interface DetailGroup {
	title: string;
	rows: DetailRow[];
}

/** Name lookups for cross references in the detail pane; any table may still be loading. */
export interface Lookups {
	items?: Map<number, string>;
	spells?: Map<number, string>;
	monsters?: Map<number, string>;
	classes?: Map<number, string>;
	races?: Map<number, string>;
	/** MME's NMR version as a number (v1.8.3 is 1.83); gates a few record filters. */
	nmr: number;
}

export interface Explorer<K extends TableName = TableName> {
	slug: SectionSlug;
	table: K;
	/** Other tables the detail pane resolves names from. */
	needs: TableName[];
	include(rec: TableTypes[K], lk: Lookups): boolean;
	columns: ColumnSpec<TableTypes[K]>[];
	rowId(rec: TableTypes[K]): string | number;
	title(rec: TableTypes[K]): string;
	defaultSort?: { col: string; dir: 'asc' | 'desc' };
	/** Set when the server builds the list (`/api/evaluate`); `columns` is then unused. */
	computed?: EvalTab;
	/** Columns MME shows that need the rules engine or a character; not faked here. */
	pending: string[];
	detail(rec: TableTypes[K], lk: Lookups): DetailGroup[];
}

// ---------- helpers ----------

const text = (s: string | null | undefined) => s ?? '';
const row = (label: string, value: string | number, long = false): DetailRow => ({
	label,
	value: String(value),
	long
});
const group = (title: string, rows: DetailRow[]): DetailGroup => ({ title, rows });

const num = <T>(id: string, header: string, get: (r: T) => number, width = 80): ColumnSpec<T> => ({
	id,
	header,
	width,
	numeric: true,
	get
});
const str = <T>(id: string, header: string, get: (r: T) => string, width?: number): ColumnSpec<T> => ({
	id,
	header,
	width,
	get
});

// ---------- items: Weapons, Armour, Sundry ----------


/** The record's name; everything else in the pane comes from the server (`/api/detail`). */
const titleOnly = (name: string): DetailGroup[] => [group(name, [])];

const itemBase = {
	needs: [] as TableName[],
	rowId: (r: Item) => r.number,
	title: (r: Item) => r.name,
	defaultSort: { col: 'name', dir: 'asc' as const },
	detail: (r: Item): DetailGroup[] => titleOnly(r.name)
};
const namedOk = (r: Item) => r.name !== '';

const weapons: Explorer<'items'> = {
	...itemBase,
	slug: 'weapons',
	table: 'items',
	include: (r) => r.item_type === 1 && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/weapons): MME's columns and cells.
	computed: 'weapons',
	columns: [],
	pending: []
};

const armour: Explorer<'items'> = {
	...itemBase,
	slug: 'armour',
	table: 'items',
	include: (r) => r.item_type === 0 && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/armour): MME's columns and cells.
	computed: 'armour',
	columns: [],
	pending: []
};

const sundry: Explorer<'items'> = {
	...itemBase,
	slug: 'items',
	table: 'items',
	include: (r) => ((r.item_type === 0 && r.worn === 0) || r.item_type > 1) && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/items): MME's columns and cells.
	computed: 'items',
	columns: [],
	pending: []
};

// ---------- spells ----------

/** Port of modMMudFunc SpellIsInGame; `disableKai` comes from the Info row (false when unknown). */
export function spellInGame(s: Spell, nmr: number, disableKai = false): boolean {
	const len = (x: string | null) => (x ?? '').length;
	const unusedKai = s.magery !== 5 || s.req_level < 1 || disableKai;
	if (!s.learnable && len(s.learned_from) <= 1 && len(s.casted_by) <= 1 && unusedKai) {
		return nmr >= 1.8 && len(s.classes) > 1;
	}
	return true;
}

const spellName = (s: Spell) => text(s.name);

const spells: Explorer<'spells'> = {
	slug: 'spells',
	table: 'spells',
	needs: [],
	include: (s, lk) => {
		const n = spellName(s);
		return n !== '' && !n.startsWith('1') && !n.toLowerCase().startsWith('sdf') && spellInGame(s, lk.nmr);
	},
	// The list comes from the server (POST /api/evaluate/spells): MME's columns and cells.
	computed: 'spells',
	columns: [],
	rowId: (s) => s.number,
	title: spellName,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: [],
	detail: (s) => titleOnly(spellName(s))
};

// ---------- monsters ----------

const monName = (m: Monster) => text(m.name);

const monsters: Explorer<'monsters'> = {
	slug: 'monsters',
	table: 'monsters',
	needs: [],
	include: (m) => m.in_game && monName(m) !== '' && !monName(m).startsWith('sdf'),
	// The list comes from the server (POST /api/evaluate/monsters): FilterMonsters with the tab's boxes, MME's
	// columns, cells and colours for the current character. The pane is PullMonsterDetail (/api/detail).
	computed: 'monsters',
	columns: [],
	rowId: (m) => m.number,
	title: monName,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: [],
	detail: (m) => titleOnly(monName(m))
};

// ---------- shops ----------

const shopOk = (s: Shop) => {
	const n = s.name;
	return n !== '' && !n.toLowerCase().startsWith('sdf') && n !== 'Leave this blank';
};

const shops: Explorer<'shops'> = {
	slug: 'shops',
	table: 'shops',
	needs: [],
	include: (s) => s.in_game && shopOk(s),
	// The list comes from the server (POST /api/evaluate/shops): MME's columns and cells.
	computed: 'shops',
	columns: [],
	rowId: (s) => s.number,
	title: (s) => s.name,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: [],
	detail: (s) => titleOnly(s.name)
};

// ---------- races and classes ----------

const races: Explorer<'races'> = {
	slug: 'races',
	table: 'races',
	needs: [],
	include: (r) => r.name !== '',
	// The list comes from the server (POST /api/evaluate/races): MME's columns and cells.
	computed: 'races',
	columns: [],
	rowId: (r) => r.number,
	title: (r) => r.name,
	defaultSort: { col: 'num', dir: 'asc' },
	pending: [],
	detail: (r) => titleOnly(r.name)
};

const classes: Explorer<'classes'> = {
	slug: 'classes',
	table: 'classes',
	needs: [],
	include: (c) => c.name !== '',
	// The list comes from the server (POST /api/evaluate/classes): MME's columns and cells.
	computed: 'classes',
	columns: [],
	rowId: (c) => c.number,
	title: (c) => c.name,
	defaultSort: { col: 'num', dir: 'asc' },
	pending: [],
	detail: (c) => titleOnly(c.name)
};

// ---------- registry ----------

export type AnyExplorer = Explorer<TableName>;

export const EXPLORERS: Partial<Record<SectionSlug, AnyExplorer>> = {
	weapons,
	armour,
	spells,
	monsters,
	shops,
	items: sundry,
	races,
	classes
} as unknown as Partial<Record<SectionSlug, AnyExplorer>>;

export function explorerFor(slug: string): AnyExplorer | undefined {
	return (EXPLORERS as Record<string, AnyExplorer | undefined>)[slug];
}

/** MME's NMR version as a number: "v1.8.3" is 1.83. */
export function parseNmr(v: string): number {
	const m = /(\d+)\.(\d+)(?:\.(\d+))?/.exec(v);
	return m ? Number(`${m[1]}.${m[2]}${m[3] ?? ''}`) : 0;
}

/** The rows that belong on the tab, in table order. */
export function listRows<K extends TableName>(ex: Explorer<K>, table: TableTypes[K][], lk: Lookups) {
	return table.filter((r) => ex.include(r, lk));
}

export function buildLookups(
	tables: Partial<{ [K in TableName]: TableTypes[K][] }>,
	nmr: number
): Lookups {
	const map = <T extends { number: number }>(rows: T[] | undefined, name: (r: T) => string | null) =>
		rows ? new Map(rows.map((r) => [r.number, name(r) ?? ''] as [number, string])) : undefined;
	return {
		items: map(tables.items, (r) => r.name),
		spells: map(tables.spells, (r) => r.name),
		monsters: map(tables.monsters, (r) => r.name),
		classes: map(tables.classes, (r) => r.name),
		races: map(tables.races, (r) => r.name),
		nmr
	};
}

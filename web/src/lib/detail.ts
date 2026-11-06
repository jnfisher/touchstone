// The explorers' detail panes as the server builds them (`POST /api/detail/{kind}/{n}`): MME's
// `PullItemDetail`, `PullSpellDetail`, `PullShopDetail`, `PullClassDetail` and `PullRaceDetail`
// with the reference list ordered as MME's ListView leaves it. Nothing is computed here; this module
// shapes the request, groups the rows for display and resolves reference links.
import type { CharacterDoc } from './character';
import type { EvalOptions } from './evaluate';
import { characterReady, wireFilter, type GlobalFilter, type WireFilter } from './globalfilter';
import type { SectionSlug } from './sections';

export type PaneKind = 'items' | 'spells' | 'shops' | 'classes' | 'races' | 'monsters';

/** The explorer tabs that have a server pane, and the pane each uses. */
const PANE_OF: Partial<Record<SectionSlug, PaneKind>> = {
	weapons: 'items',
	armour: 'items',
	items: 'items',
	spells: 'spells',
	shops: 'shops',
	classes: 'classes',
	races: 'races',
	monsters: 'monsters'
};

export function paneKind(slug: string): PaneKind | undefined {
	return PANE_OF[slug as SectionSlug];
}

export interface PaneRow {
	label: string;
	value: string;
	kind: string;
}
export interface RefCell {
	text: string;
	tag: string | null;
}
export interface RefTarget {
	table: 'items' | 'monsters' | 'spells' | 'shops' | 'rooms';
	number: number;
	/** A room's "map/room" key. */
	key?: string;
}
export interface RefRow {
	kind: string;
	text: string;
	tag: string | null;
	cells: RefCell[];
	target: RefTarget | null;
}
/** A sub-item of a monster pane line (MME's `ListSubItems`). */
export interface LineCell {
	text: string;
	tag?: string;
	/** The VB `ForeColor` MME gave the cell. */
	color?: number;
	bold?: boolean;
}
/** A line of the monster pane: MME's `lvMonsterDetail` item. */
export interface PaneLine {
	label: string;
	/** The ListItem's tag, lower case: `item`, `spell`, `monster`, `room`, `greet_text`, `plain`, ... */
	kind: string;
	color?: number;
	bold?: boolean;
	cells: LineCell[];
	/** The record the line stands for, to link to. */
	target: RefTarget | null;
}
export interface Compare {
	label: string;
	text: string;
}
export interface Pane {
	kind: PaneKind;
	number: number;
	text: string;
	rows: PaneRow[];
	references: RefRow[];
	/** Monsters: the pane's list, line by line. */
	lines?: PaneLine[];
	compare: Compare[];
	charm?: number;
	charm_label?: string;
	errors: number[];
}

export interface PaneRequest {
	character?: CharacterDoc;
	global_filter: WireFilter;
	options: EvalOptions;
	charm?: number;
	show_charm?: boolean;
	show_sell?: boolean;
}

/**
 * The request body: the same character and bar as the lists, plus the shop pane's boxes and, for a
 * monster, the lair/party boxes. `keepCharacter` sends a complete character the bar does not mirror
 * (its attack choice still decides the monster pane's damage lines).
 */
export function paneBody(
	doc: CharacterDoc | undefined,
	filter: GlobalFilter,
	extra: {
		charm?: number;
		showCharm?: boolean;
		showSell?: boolean;
		options?: EvalOptions;
		keepCharacter?: boolean;
	} = {}
): PaneRequest {
	const global_filter = wireFilter(filter, doc);
	const body: PaneRequest = { global_filter, options: extra.options ?? {} };
	if (doc && (global_filter.use_character || (extra.keepCharacter && characterReady(doc)))) body.character = doc;
	if (extra.charm !== undefined) body.charm = extra.charm;
	if (extra.showCharm) body.show_charm = true;
	if (extra.showSell) body.show_sell = true;
	return body;
}

export interface PaneGroup {
	title: string;
	rows: PaneRow[];
}

const TITLES: Record<string, string> = {
	summary: 'Summary',
	weapon_damage: 'Damage',
	notice: 'Notice',
	flags: 'Flags',
	uses: 'Uses',
	abilities: 'Abilities',
	casts: 'Casts',
	class_ok: 'Class OK',
	classes: 'Classes',
	races: 'Races',
	negates: 'Negates',
	ability: 'Abilities',
	shop_line: 'Stock',
	fact: 'Details',
	cost: 'Cost',
	reference: 'References',
	message: ''
};

/** Rows grouped by kind, in the order each kind first appears. */
export function groupRows(rows: PaneRow[]): PaneGroup[] {
	const groups: PaneGroup[] = [];
	for (const r of rows) {
		const title = TITLES[r.kind] ?? r.kind;
		let g = groups.find((x) => x.title === title);
		if (!g) groups.push((g = { title, rows: [] }));
		g.rows.push(r);
	}
	return groups;
}

/** The tab an item belongs to: MME lists weapons, worn armour and everything else apart. */
export function itemSlug(it: { item_type: number; worn: number }): 'weapons' | 'armour' | 'items' {
	if (it.item_type === 1) return 'weapons';
	if (it.item_type === 0 && it.worn !== 0) return 'armour';
	return 'items';
}

/** Where a reference row links to: a tab and the row id that tab selects. */
export function linkFor(
	t: RefTarget,
	items?: ReadonlyMap<number, { item_type: number; worn: number }>
): { slug: SectionSlug; id: string | number } | undefined {
	switch (t.table) {
		case 'monsters':
		case 'spells':
		case 'shops':
			return { slug: t.table, id: t.number };
		case 'rooms':
			return t.key ? { slug: 'rooms', id: t.key } : undefined;
		case 'items': {
			const it = items?.get(t.number);
			return it ? { slug: itemSlug(it), id: t.number } : undefined;
		}
	}
}

/** The columns of the reference table: the ListItem text first, then each sub-item. */
export function refColumns(rows: RefRow[]): number {
	return 1 + rows.reduce((n, r) => Math.max(n, r.cells.length), 0);
}

export function refCells(r: RefRow, columns: number): string[] {
	const out = [r.text, ...r.cells.map((c) => c.text)];
	while (out.length < columns) out.push('');
	return out;
}

export class PaneError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'PaneError';
	}
}

type FetchFn = typeof fetch;

export function createPaneClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async pane(kind: PaneKind, n: number, body: PaneRequest, signal?: AbortSignal): Promise<Pane> {
			const res = await fetchFn(`/api/detail/${kind}/${n}`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body),
				signal
			});
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new PaneError(res.status, reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as Pane;
		}
	};
}

export const paneClient = createPaneClient();

// The realm's characters (`/api/realm/*`, card R-001): the wire types, the client, the list's filters and
// columns, and the view model of a character's sheet. Every number is the server's; this module words and
// groups them. A realm character is read-only, so nothing here writes.
import type { ColDef } from 'ag-grid-community';
import type { CharacterDoc, EvalState } from './character';

// ---------- wire types ----------

/** One row of `GET /api/realm/characters`. */
export interface RealmSummary {
	id: number;
	first_name: string;
	last_name: string;
	race: number;
	race_name: string;
	class: number;
	class_name: string;
	level: number;
	exp: number;
	gang: string;
	map: number;
	room: number;
	/** Unix seconds; 0 when the character never logged in. */
	last_on: number;
	last_on_iso: string | null;
	lives: number;
	/** How many stored values touchstone's calculation disagrees with. */
	differs: number;
}

export interface RealmList {
	available: boolean;
	reason?: string;
	characters: RealmSummary[];
}

export interface RealmComparison {
	field: string;
	label: string;
	game: number;
	/** Null when the pipeline could not calculate it. */
	touchstone: number | null;
	/** MME's range, for a value the game rolls within one. */
	range?: [number, number];
	differs: boolean;
	tolerance: string;
	/** The known kind of difference, when it differs. */
	cause?: string;
}

export interface RealmIdentity {
	first_name: string;
	last_name: string;
	title: string;
	race: number;
	race_name: string;
	class: number;
	class_name: string;
	level: number;
	exp: number;
	gang: string;
	evil: number;
	alignment: number;
	alignment_name: string;
	lawful: number;
	lives: number;
	map: number;
	room: number;
	last_on: number;
	last_on_iso: string | null;
	death_date: number;
	unspent_cp: number;
	total_cp: number;
	/** Runic, platinum, gold, silver, copper. */
	coins: number[];
}

export interface RealmWorn {
	item: number;
	name: string;
	/** The game's own slot (a list: the first free slot). */
	slot: number;
	/** MME's equipment slot this item took. */
	mme_slot: number;
	mme_slot_name: string;
	uses: number;
}

export interface RealmInventory {
	item: number;
	name: string;
	slot: number;
	readied: boolean;
	uses: number;
}

export interface RealmSpell {
	spell: number;
	name: string;
}

export interface RealmActiveSpell extends RealmSpell {
	slot: number;
	rounds: number;
	value: number;
	/** The bless slot it fills; absent for a spell MME has no slot for. */
	bless_slot?: number | null;
}

export interface RealmAbility {
	ability: number;
	name: string;
	value: number;
}

export interface RealmUnmapped {
	kind: string;
	id: number;
	name: string;
	reason: string;
}

export interface RealmDetail {
	available: boolean;
	id: number;
	identity: RealmIdentity;
	/** The game's stored values, by field. */
	game: Record<string, number | number[]>;
	/** The evaluated sheet, the shape of `/api/character/evaluate`; null when there is none. */
	sheet: EvalState | null;
	sheet_error: string | null;
	worn: RealmWorn[];
	weapon: { item: number; name: string; mme_slot: number; mapped: boolean } | null;
	inventory: RealmInventory[];
	spellbook: RealmSpell[];
	active_spells: RealmActiveSpell[];
	abilities: RealmAbility[];
	comparison: RealmComparison[];
	differs: number;
	unmapped: RealmUnmapped[];
}

// ---------- the client ----------

type FetchFn = typeof fetch;

export class RealmError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
		this.name = 'RealmError';
	}
}

export function createRealmClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function read<T>(res: Response): Promise<T> {
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new RealmError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	return {
		async list(signal?: AbortSignal): Promise<RealmList> {
			return read(await fetchFn('/api/realm/characters', { signal, cache: 'no-cache' }));
		},
		async detail(id: number, signal?: AbortSignal): Promise<RealmDetail> {
			return read(await fetchFn(`/api/realm/characters/${id}`, { signal, cache: 'no-cache' }));
		},
		/** The mapped character, as the editor and the calculators take it. */
		async input(id: number, signal?: AbortSignal): Promise<CharacterDoc> {
			return read(await fetchFn(`/api/realm/characters/${id}/input`, { signal, cache: 'no-cache' }));
		}
	};
}
export type RealmClient = ReturnType<typeof createRealmClient>;
export const realmClient = createRealmClient();

// ---------- the list ----------

export interface RealmFilters {
	search: string;
	/** Race and class record numbers; null for any. */
	race: number | null;
	class: number | null;
	/** Level bounds as typed; blank or junk is no bound. */
	levelMin: string;
	levelMax: string;
}

export const NO_FILTERS: RealmFilters = { search: '', race: null, class: null, levelMin: '', levelMax: '' };

/** A whole number from a box, or undefined for blank or anything else. */
export function boundOf(text: string): number | undefined {
	const t = text.trim();
	return /^\d{1,4}$/.test(t) ? Number(t) : undefined;
}

/** The label a character carries in lists and the picker: "Name Last". */
export function fullName(c: Pick<RealmSummary, 'first_name' | 'last_name'>): string {
	return [c.first_name, c.last_name].filter((p) => p !== '').join(' ');
}

/** The rows that pass the quick search (every word, anywhere in the row's text) and the three filters. */
export function filterRealm(rows: readonly RealmSummary[], f: RealmFilters): RealmSummary[] {
	const words = f.search.toLowerCase().split(/\s+/).filter(Boolean);
	const lo = boundOf(f.levelMin);
	const hi = boundOf(f.levelMax);
	return rows.filter((r) => {
		if (f.race !== null && r.race !== f.race) return false;
		if (f.class !== null && r.class !== f.class) return false;
		if (lo !== undefined && r.level < lo) return false;
		if (hi !== undefined && r.level > hi) return false;
		if (!words.length) return true;
		const text = `${fullName(r)} ${r.race_name} ${r.class_name} ${r.gang} ${r.level}`.toLowerCase();
		return words.every((w) => text.includes(w));
	});
}

/** The distinct races and classes of a roster, by name, for the filter boxes. */
export function facets(rows: readonly RealmSummary[]): { races: { id: number; name: string }[]; classes: { id: number; name: string }[] } {
	const races = new Map<number, string>();
	const classes = new Map<number, string>();
	for (const r of rows) {
		races.set(r.race, r.race_name);
		classes.set(r.class, r.class_name);
	}
	const sorted = (m: Map<number, string>) => [...m].map(([id, name]) => ({ id, name })).sort((a, b) => a.name.localeCompare(b.name));
	return { races: sorted(races), classes: sorted(classes) };
}

/** A character in the picker's words: "Name Last, Race Class L12". */
export function pickerLabel(c: RealmSummary): string {
	return `${fullName(c)} (${c.race_name} ${c.class_name} L${c.level})`;
}

/** The picker's list for the box's text; the same word match as the list's quick search. */
export function searchPicker(rows: readonly RealmSummary[], text: string): RealmSummary[] {
	return filterRealm(rows, { ...NO_FILTERS, search: text });
}

/** A unix time as "2025-10-09 09:53" (UTC, as the server gives it); "never" for 0. */
export function lastOnText(c: Pick<RealmSummary, 'last_on' | 'last_on_iso'>): string {
	if (!c.last_on || !c.last_on_iso) return 'never';
	return c.last_on_iso.replace('T', ' ').slice(0, 16);
}

const plain = { type: 'rightAligned', filter: false } as const;

export function realmColumns(): ColDef<RealmSummary>[] {
	return [
		{ colId: 'name', headerName: 'Name', field: 'first_name', minWidth: 110 },
		{ colId: 'last', headerName: 'Last name', field: 'last_name', minWidth: 110 },
		{ colId: 'race', headerName: 'Race', field: 'race_name', maxWidth: 110 },
		{ colId: 'class', headerName: 'Class', field: 'class_name', maxWidth: 120 },
		{ colId: 'level', headerName: 'Level', field: 'level', type: 'rightAligned', filter: 'agNumberColumnFilter', maxWidth: 90 },
		{
			colId: 'exp',
			headerName: 'Exp',
			field: 'exp',
			type: 'rightAligned',
			filter: 'agNumberColumnFilter',
			valueFormatter: (p) => (typeof p.value === 'number' ? p.value.toLocaleString('en-US') : '')
		},
		{ colId: 'gang', headerName: 'Gang', field: 'gang', maxWidth: 130 },
		{
			colId: 'laston',
			headerName: 'Last on',
			valueGetter: (p) => p.data?.last_on,
			valueFormatter: (p) => (p.data ? lastOnText(p.data) : ''),
			minWidth: 140
		},
		{ colId: 'lives', headerName: 'Lives', field: 'lives', ...plain, maxWidth: 80 },
		{
			colId: 'differs',
			headerName: 'Differs',
			field: 'differs',
			type: 'rightAligned',
			filter: 'agNumberColumnFilter',
			maxWidth: 90,
			tooltipValueGetter: () => "Stored values touchstone's calculation disagrees with"
		}
	];
}

// ---------- the sheet ----------

/** The R-001 cause codes in words (docs/differences.md, "Realm characters"). */
const CAUSES: Record<string, string> = {
	'no-magic-class': 'The game stores no mana and a spellcasting base of -150 for a class with no magic; MME works both out from its formulas for every class.',
	'kai-mana': 'The game gives a Kai class a mana of its level less one; MME treats a Kai like any caster.',
	'no-stealth-skill': 'With no stealth skill from class or race the game stores 0; MME shows the ability total alone, armour penalties included.',
	'no-picklocks-skill': 'The game gives picklocks only to a character with the ability and stores 0 otherwise; MME works a figure out for every class.',
	'active-spell': 'A spell running on the character changed this stored value at the strength it was cast; touchstone takes the average cast.',
	'unmapped-item': 'An item MME has no place for still counts in the game but is left out of touchstone\'s calculation.',
	'coin-weight-capped': 'The game counts all the coins in the load; MME takes the weight of at most 999 of them.'
};

export const NO_CAUSE = 'No known cause for this difference.';

/** Why a field differs: the cause in words, or a plain statement that none is known. */
export function causeText(cause: string | undefined): string {
	return cause === undefined ? NO_CAUSE : (CAUSES[cause] ?? `Known kind of difference: ${cause}.`);
}

export interface SheetRow {
	field: string;
	label: string;
	/** What touchstone calculates, as shown ("n/a" when it could not). */
	touchstone: string;
	/** What the game stores, as shown. */
	game: string;
	differs: boolean;
	/** The hover text: why a flagged row differs, how a clean one is compared. */
	tip: string;
}

export interface SheetGroup {
	title: string;
	rows: SheetRow[];
}

const STATS = ['str', 'int', 'wil', 'agi', 'hea', 'cha'];
const VITALS = ['max_hp', 'max_mana', 'max_mr', 'spellcast', 'perception_abilities', 'stealth', 'picklocks', 'weight', 'encumbrance_pct'];

function shown(field: string, v: number): string {
	return field === 'encumbrance_pct' ? `${v}%` : String(v);
}

/** One comparison as two displayed values and a flag; a rolled value shows MME's range. */
export function sheetRow(c: RealmComparison): SheetRow {
	const mine = c.range ? `${c.range[0]}-${c.range[1]}` : c.touchstone === null ? 'n/a' : shown(c.field, c.touchstone);
	const tip = c.differs
		? `${causeText(c.cause)}${c.range ? ` The game's value is outside MME's range of ${c.range[0]} to ${c.range[1]}.` : ''}`
		: `Agrees (${c.tolerance}).`;
	return { field: c.field, label: c.label, touchstone: mine, game: shown(c.field, c.game), differs: c.differs, tip };
}

/** The comparison as the sheet's three tables: stats, vitals and skills, and the Alter bonuses. */
export function sheetGroups(comparison: readonly RealmComparison[]): SheetGroup[] {
	const rows = comparison.map(sheetRow);
	const pick = (fields: string[]) => rows.filter((r) => fields.includes(r.field));
	const rest = rows.filter((r) => !STATS.includes(r.field) && !VITALS.includes(r.field));
	return [
		{ title: 'Stats', rows: pick(STATS) },
		{ title: 'Vitals and skills', rows: pick(VITALS) },
		{ title: 'Bonuses', rows: rest }
	].filter((g) => g.rows.length > 0);
}

/** The stored values touchstone does not compare, with their labels, for the "also stored" list. */
const STORED_ONLY: [string, string][] = [
	['hp', 'HP now'],
	['mana', 'Mana now'],
	['martial', 'Martial arts'],
	['thievery', 'Thievery'],
	['disarm', 'Disarm traps'],
	['traps', 'Traps'],
	['tracking', 'Tracking'],
	['energy', 'Energy'],
	['max_energy', 'Max energy']
];

export function storedOnly(game: RealmDetail['game']): { label: string; value: string }[] {
	return STORED_ONLY.flatMap(([k, label]) => {
		const v = game[k];
		return typeof v === 'number' ? [{ label, value: String(v) }] : [];
	});
}

/** The five purses, copper last, as "11 gold, 17 copper"; "none" for an empty purse. */
export function coinText(coins: readonly number[]): string {
	const names = ['runic', 'platinum', 'gold', 'silver', 'copper'];
	const parts = coins.flatMap((n, i) => (n > 0 ? [`${n.toLocaleString('en-US')} ${names[i]}`] : []));
	return parts.length ? parts.join(', ') : 'none';
}

/** MME's equipment slots in the order the sheet lists worn gear: head to feet, then the weapon. */
export interface WornLine {
	slot: string;
	item: number;
	name: string;
	/** False for an item MME has no place for. */
	mapped: boolean;
}

/**
 * Worn gear by MME slot, the wielded weapon last. An unmapped item is listed apart (`unmapped`), so a worn
 * line is always a mapped one.
 */
export function wornLines(d: Pick<RealmDetail, 'worn' | 'weapon'>): WornLine[] {
	const lines: WornLine[] = [...d.worn]
		.sort((a, b) => a.mme_slot - b.mme_slot)
		.map((w) => ({ slot: w.mme_slot_name, item: w.item, name: w.name, mapped: true }));
	if (d.weapon?.mapped) lines.push({ slot: 'Weapon', item: d.weapon.item, name: d.weapon.name, mapped: true });
	return lines;
}

/** How many of the compared values differ with no known cause: the ones worth a second look. */
export function unexplained(comparison: readonly RealmComparison[]): number {
	return comparison.filter((c) => c.differs && c.cause === undefined).length;
}

/** The id the sandbox's copy of a realm character gets as a name: free among the saved ones. */
export function copyName(first: string, taken: readonly string[]): string {
	const used = new Set(taken);
	if (!used.has(first)) return first;
	let n = 1;
	for (;;) {
		const name = n === 1 ? `${first} (copy)` : `${first} (copy ${n})`;
		if (!used.has(name)) return name;
		n++;
	}
}

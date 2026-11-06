// The Compare tab's lists (MME's lvWeaponCompare, lvArmourCompare, lvSpellCompare, lvMonsterCompare).
// A list is just the record numbers in the order they were added; its rows are the explorer's rows for
// those records (`POST /api/evaluate/{tab}` with `options.numbers`). The lists travel with the
// character as the `.mmec` [Compare] keys, kept in the character's `extra` so autosave, export and
// import carry them. Everything here is list bookkeeping; no game rule is computed.

export const COMPARE_KINDS = ['weapons', 'armour', 'spells', 'monsters'] as const;
export type CompareKind = (typeof COMPARE_KINDS)[number];
export type CompareLists = Record<CompareKind, number[]>;

export interface ExtraEntry {
	section: string;
	key: string;
	value: string;
}

export const COMPARE_SECTION = 'Compare';
/** SaveCharacter's key for each list. */
export const COMPARE_KEY: Record<CompareKind, string> = {
	weapons: 'WeaponCompare',
	armour: 'ArmourCompare',
	spells: 'SpellCompare',
	monsters: 'MonsterCompare'
};
/** SaveCharacter writes at most 101 entries per list (its loop stops once the counter passes 101). */
export const MAX_SAVED = 101;

export const COMPARE_LABEL: Record<CompareKind, string> = {
	weapons: 'Weapons',
	armour: 'Armour',
	spells: 'Spells',
	monsters: 'Monsters'
};

export function emptyLists(): CompareLists {
	return { weapons: [], armour: [], spells: [], monsters: [] };
}

/**
 * The record numbers of a saved list. The loaders read up to each comma, so text after the last comma
 * is ignored, and a piece that is not a positive record (`Val` gives 0) finds nothing and adds nothing.
 */
export function parseList(text: string): number[] {
	const pieces = text.split(',');
	pieces.pop();
	const out: number[] = [];
	for (const p of pieces) {
		const n = Math.trunc(parseFloat(p));
		if (Number.isFinite(n) && n > 0) out.push(n);
	}
	return out;
}

/** SaveCharacter's text for a list: each number and a comma. */
export function formatList(list: readonly number[]): string {
	return list
		.slice(0, MAX_SAVED)
		.map((n) => `${n},`)
		.join('');
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function entryFor(extra: readonly ExtraEntry[], key: string): ExtraEntry | undefined {
	return extra.find((e) => same(e.section, COMPARE_SECTION) && same(e.key, key));
}

/** The lists a character carries. */
export function readLists(extra: readonly ExtraEntry[] | undefined): CompareLists {
	const out = emptyLists();
	for (const k of COMPARE_KINDS) {
		const e = entryFor(extra ?? [], COMPARE_KEY[k]);
		if (e) out[k] = parseList(e.value);
	}
	return out;
}

/**
 * `extra` with the lists written into the [Compare] entries (a new array; the other entries are kept
 * as they were). Nothing is added for four empty lists on a character that has no [Compare] entries.
 */
export function writeLists(extra: readonly ExtraEntry[] | undefined, lists: CompareLists): ExtraEntry[] {
	const out = (extra ?? []).map((e) => ({ ...e }));
	const any = COMPARE_KINDS.some((k) => lists[k].length > 0);
	const has = COMPARE_KINDS.some((k) => entryFor(out, COMPARE_KEY[k]));
	if (!any && !has) return out;
	for (const k of COMPARE_KINDS) {
		const value = formatList(lists[k]);
		const e = entryFor(out, COMPARE_KEY[k]);
		if (e) e.value = value;
		else out.push({ section: COMPARE_SECTION, key: COMPARE_KEY[k], value });
	}
	return out;
}

/**
 * CompareAddItem / CompareAddMonster / mnuSpellsPopUpItem add: a record already on the list is taken
 * off it (the "add it again?" question is commented out in MME and `RemovePopup*Compare` runs instead),
 * otherwise it goes on the end. Not a record (0 or less) changes nothing.
 */
export function toggled(list: readonly number[], n: number): number[] {
	if (!(n > 0)) return [...list];
	const at = list.indexOf(n);
	return at >= 0 ? list.filter((_, i) => i !== at) : [...list, n];
}

/** RemovePopup*Compare on the selected entry: that one entry leaves the list. */
export function withoutAt(list: readonly number[], index: number): number[] {
	return list.filter((_, i) => i !== index);
}

// ---------- "add all equipped" (cmdEquipButtons case 3) ----------

export type ItemKind = 'weapon' | 'armour' | 'other' | 'missing';

/** `POST /api/compare/classify`: the list `CompareAddItem` puts each record on. */
export interface Classified {
	number: number;
	kind: ItemKind;
	worn: boolean;
}

/**
 * `CompareAddItem` for each equipped record in slot order: armour (ItemType 0) goes to the armour list,
 * weapons to the weapon list, anything else is ignored. Each one toggles, so running it again with the
 * same gear takes the same items off.
 */
export function addEquipped(lists: CompareLists, equipped: readonly number[], kinds: ReadonlyMap<number, ItemKind>): CompareLists {
	const out: CompareLists = { ...lists };
	for (const n of equipped) {
		if (!(n > 0)) continue;
		const kind = kinds.get(n);
		if (kind === 'weapon') out.weapons = toggled(out.weapons, n);
		else if (kind === 'armour') out.armour = toggled(out.armour, n);
	}
	return out;
}

type FetchFn = typeof fetch;

export function createCompareClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async classify(items: number[]): Promise<Classified[]> {
			if (items.length === 0) return [];
			const res = await fetchFn('/api/compare/classify', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ items })
			});
			if (!res.ok) throw new Error((await res.text().catch(() => '')).trim() || `HTTP ${res.status}`);
			return ((await res.json()) as { items: Classified[] }).items;
		}
	};
}

export const compareClient = createCompareClient();

// The global filter bar (MME's frmGlobalFilter): class, race, level, alignment, "use character" and
// the minimum item level, applied to every explorer list. The values live in this browser's local
// storage like the other view options; every access is wrapped because storage can be blocked.
import type { CharacterDoc } from './character';

export const GLOBAL_FILTER_KEY = 'touchstone.globalfilter';

/** MME's three-digit boxes. */
export const MAX_BOX = 999;

export interface GlobalFilter {
	/** Mirror the character on the Character tab instead of the values below. */
	use_character: boolean;
	/** A class number; 0 is the combo's first entry, any class. */
	class: number;
	race: number;
	level: number;
	/** 0 any, 1 good, 2 neutral, 3 evil. */
	alignment: number;
	min_item_level: number;
}

/**
 * Level 999, any class, any alignment: the case MME's `ItemIsUsableByChar` answers true for at
 * once, so a bar left alone filters nothing. "Use character" starts on, as P4-05's checkbox did.
 */
export const DEFAULT_FILTER: GlobalFilter = {
	use_character: true,
	class: 0,
	race: 0,
	level: MAX_BOX,
	alignment: 0,
	min_item_level: 0
};

/** What goes to the server: the `global_filter` object of `POST /api/evaluate/{tab}`. */
export type WireFilter = GlobalFilter;

function whole(v: unknown, max: number, fallback: number): number {
	if (typeof v !== 'number' || !Number.isFinite(v)) return fallback;
	return Math.min(max, Math.max(0, Math.trunc(v)));
}

/** A damaged or foreign value falls back to the default for that field. */
export function sanitize(v: unknown): GlobalFilter {
	const o = typeof v === 'object' && v !== null ? (v as Partial<Record<keyof GlobalFilter, unknown>>) : {};
	const d = DEFAULT_FILTER;
	return {
		use_character: typeof o.use_character === 'boolean' ? o.use_character : d.use_character,
		class: whole(o.class, 32767, d.class),
		race: whole(o.race, 32767, d.race),
		level: whole(o.level, MAX_BOX, d.level),
		alignment: whole(o.alignment, 3, d.alignment),
		min_item_level: whole(o.min_item_level, MAX_BOX, d.min_item_level)
	};
}

export function readFilter(storage: Pick<Storage, 'getItem'> | undefined): GlobalFilter {
	try {
		const raw = storage?.getItem(GLOBAL_FILTER_KEY);
		return raw ? sanitize(JSON.parse(raw)) : { ...DEFAULT_FILTER };
	} catch {
		return { ...DEFAULT_FILTER };
	}
}

export function writeFilter(storage: Pick<Storage, 'setItem'> | undefined, f: GlobalFilter): void {
	try {
		storage?.setItem(GLOBAL_FILTER_KEY, JSON.stringify(f));
	} catch {
		// Private window or blocked storage: the choice lasts for this page only.
	}
}

/** Class and race are picked: the character can be evaluated, so it can be mirrored. */
export function characterReady(doc: CharacterDoc | undefined): doc is CharacterDoc {
	return !!doc && doc.class !== null && doc.race !== null;
}

/**
 * The filter as the server gets it. Mirroring needs a complete character; without one the bar's
 * own values apply, so the lists never go empty because a character is half made.
 */
export function wireFilter(f: GlobalFilter, doc: CharacterDoc | undefined): WireFilter {
	return { ...f, use_character: f.use_character && characterReady(doc) };
}

/** Whether the bar changes the lists at all (it is at the defaults of "no filter"). */
export function isFiltering(f: GlobalFilter, doc: CharacterDoc | undefined): boolean {
	if (f.use_character && characterReady(doc)) return true;
	return f.class !== 0 || f.level < MAX_BOX || f.alignment !== 0;
}

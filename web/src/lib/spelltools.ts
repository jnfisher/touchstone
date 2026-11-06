// The small rules around the spell tools: what the Spells tab's option boxes enable
// (`chkSpellOptions_Click`), how the lookups' answers become dialog rows, the Spell Book window's
// request, and the chest text MME copies. The rules themselves run on the server.
import type { BuilderSpellbook, Chest, Counters, SpellBookRequest, SpellCasts } from './api';
import type { EvalOptions } from './evaluate';
import type { GlobalFilter } from './globalfilter';
import type { RefTarget } from './detail';

/** A line of the results dialog: the cells and the record the line stands for. */
export interface ResultRow {
	cells: string[];
	link?: RefTarget;
}

/** The dialog MME's Results window becomes. */
export interface ResultsDialog {
	title: string;
	/** Column headings, when the list has more than one column. */
	columns?: string[];
	rows: ResultRow[];
}

/** The Spells tab's "Calc. Combat" box: the "vs MR" box and the "vs Anti-Magic" box follow it. */
export function spellOptionsEnabled(calcCombat: boolean): { vsMr: boolean; antiMagic: boolean } {
	return { vsMr: calcCombat, antiMagic: calcCombat };
}

export interface SpellBar {
	calcCombat: boolean;
	vsMr: string;
	antiMagic: boolean;
}

/** The evaluate options the Spells tab's boxes make (a box that is disabled counts for nothing). */
export function spellBarOptions(bar: SpellBar): EvalOptions {
	if (!bar.calcCombat) return {};
	const on = spellOptionsEnabled(bar.calcCombat);
	return {
		calc_combat: true,
		...(on.vsMr ? { vs_mr: bar.vsMr } : {}),
		...(on.antiMagic && bar.antiMagic ? { anti_magic: true } : {})
	};
}

/** `Find counters`: `Item: name (n)` and `Spell: name (n)` rows that open their records. */
export function counterRows(c: Counters): ResultRow[] {
	return c.rows.map((r) => ({
		cells: [r.text],
		link: { table: r.table, number: r.number }
	}));
}

/** `What casts this spell?`: the reference rows MME's `GetLocations` makes. */
export function castRows(c: SpellCasts): ResultRow[] {
	return c.rows.map((r) => ({ cells: [r.text, ...r.cells.map((x) => x.text)], link: r.target ?? undefined }));
}

/** The chest list: the chance, then the item. */
export function chestRows(c: Chest): ResultRow[] {
	return c.rows.map((r) => ({
		cells: [r.text, r.item_text],
		link: { table: 'items', number: r.item }
	}));
}

/**
 * What "Copy Chest to Clipboard" puts on the clipboard: MME's text, whose lines hold only the
 * chances (the items are in the sub-items it does not read).
 */
export function chestClipText(name: string, number: number, c: Chest): string | undefined {
	if (c.rows.length === 0) return undefined;
	return [`Chest info for ${name} (${number}):`, ...c.rows.map((r) => r.text)].join('\r\n');
}

/** The Spell Book window's controls. */
export interface BookForm {
	class: number;
	level: string;
	alignment: number;
	magery: number;
	magery_level: number;
}

export const MAGERY_NAMES = ['Any', 'Mage', 'Priest', 'Druid', 'Bard', 'Kai'] as const;
export const MAGERY_LEVELS = ['Any', '1', '2', '3'] as const;
export const ALIGNMENTS = ['Any', 'Good', 'Neutral', 'Evil'] as const;

/** The window as the Spellbook button of the builder opens it: level 999 and the class's magery. */
export function bookSeed(sb: BuilderSpellbook): Partial<BookForm> | undefined {
	if (sb.action !== 'open' || sb.class === null) return undefined;
	return {
		class: sb.class,
		level: String(sb.level ?? 999),
		magery: sb.magery ?? 0,
		magery_level: sb.magery_level ?? 0
	};
}

/** Is the global filter bar filtering anything (MME's `chkGlobalFilter`)? */
export function barIsOn(f: GlobalFilter): boolean {
	return f.class > 0 || f.level < 999 || f.alignment > 0;
}

/** The request for the list: the form plus what the bar and the character say. */
export function bookRequest(
	form: BookForm,
	bar: GlobalFilter,
	magery: { type: number; level: number }
): SpellBookRequest {
	return {
		class: form.class,
		level: form.level,
		alignment: form.alignment,
		magery: form.magery,
		magery_level: form.magery_level,
		filter_on: barIsOn(bar),
		global_level: String(bar.level),
		global_class: bar.class,
		char_magery: magery.type,
		char_magery_level: Math.min(magery.level, 3)
	};
}

/** Sort a Spell Book column the way MME's click handler does: text for Name, Short, Magery and Detail. */
export function isTextColumn(index: number): boolean {
	return index === 1 || index === 2 || index === 3 || index === 11;
}

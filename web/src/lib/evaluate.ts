// The explorer lists as the server builds them (`POST /api/evaluate/{tab}`): MME's own columns and
// cells for the current character. Nothing is computed here; this module only shapes the request,
// maps the payload to grid columns and turns MME's cell colours into theme classes.
import type { ColDef } from 'ag-grid-community';
import type { CharacterDoc } from './character';
import { characterReady, wireFilter, type GlobalFilter, type WireFilter } from './globalfilter';
import type { MonsterControls } from './monsters';

export type EvalTab = 'weapons' | 'armour' | 'spells' | 'monsters' | 'items' | 'races' | 'classes' | 'shops';

export interface EvalColumn {
	id: string;
	header: string;
	numeric: boolean;
}
export interface EvalCell {
	text: string;
	/** A number for numeric columns, the text otherwise. */
	sort: number | string;
	/** The VB ForeColor MME assigned to the cell. */
	color?: number;
}
export interface EvalRow {
	/** The record number (0 for the weapon list's surprise punch). */
	number: number;
	/** One cell per column, the `#` column first. */
	cells: EvalCell[];
}
export interface EvalTable {
	columns: EvalColumn[];
	rows: EvalRow[];
	/** Monsters: some record was filtered out, so MME's caption reads "*Monsters*". */
	filtered?: boolean;
}

/** The view toggles of the tabs; MME's defaults when absent. */
export interface EvalOptions {
	calc_combat?: boolean;
	slow_85?: boolean;
	attack?: number;
	vs_ac?: string;
	vs_dr?: string;
	vs_dodge?: string;
	ability?: number;
	ability_op?: 'le' | 'ge';
	ability_value?: string;
	level?: number;
	anti_magic?: boolean;
	vs_mr?: string;
	/** The Monsters tab's filter boxes, lair/party mode and extra filters. */
	monsters?: MonsterControls;
}

export const EVAL_TABS: readonly EvalTab[] = ['weapons', 'armour', 'spells', 'monsters', 'items', 'races', 'classes', 'shops'];

export function isEvalTab(s: string): s is EvalTab {
	return (EVAL_TABS as readonly string[]).includes(s);
}

export interface EvalRequest {
	/** Sent only while the filter mirrors it. */
	character?: CharacterDoc;
	global_filter: WireFilter;
	options: EvalOptions;
}

/**
 * The request body. The global filter bar goes along as it stands; the character goes along only
 * when the bar mirrors it, which needs class and race picked (otherwise the bar's own values
 * apply). The server's `global_filter` then also stands for MME's `chkGlobalFilter`.
 *
 * `keepCharacter` sends a complete character even when the bar does not mirror it: MME keeps its
 * attack choice (and the exp/hr block) whatever the global filter says, and the Monsters list and
 * pane show damage by it.
 */
export function evaluateBody(
	doc: CharacterDoc | undefined,
	filter: GlobalFilter,
	options: EvalOptions = {},
	keepCharacter = false
): EvalRequest {
	const global_filter = wireFilter(filter, doc);
	if (doc && (global_filter.use_character || (keepCharacter && characterReady(doc)))) {
		return { character: doc, global_filter, options };
	}
	return { global_filter, options };
}

// MME's ForeColor values (BGR integers): a race stat range well below / above the average, a monster
// cell calculated against this character or party, and a monster row that does not match the filter.
const BELOW_AVERAGE = 0x80;
const ABOVE_AVERAGE = 0x8000;
const CALCULATED = 193 + 232 * 65536;
const NOT_MATCHING = 192 + 192 * 256 + 192 * 65536;

/** Theme class for a cell colour; colours MME does not use for lists are left to the grid. */
export function colorClass(color: number | undefined): string | undefined {
	if (color === BELOW_AVERAGE) return 'ts-cell-below';
	if (color === ABOVE_AVERAGE) return 'ts-cell-above';
	if (color === CALCULATED) return 'ts-cell-calc';
	if (color === NOT_MATCHING) return 'ts-cell-nomatch';
	return undefined;
}

const upper = (v: unknown) => String(v ?? '').toUpperCase();

/** Grid columns for a payload: sort and filter by `sort`, show `text`. */
export function toColDefs(columns: EvalColumn[], sort?: { col: string; dir: 'asc' | 'desc' }): ColDef<EvalRow>[] {
	return columns.map((c, i) => {
		const def: ColDef<EvalRow> = {
			colId: c.id,
			headerName: c.header,
			valueGetter: (p) => p.data?.cells[i]?.sort,
			valueFormatter: (p) => p.data?.cells[i]?.text ?? '',
			getQuickFilterText: (p) => p.data?.cells[i]?.text ?? '',
			cellClass: (p) => colorClass(p.data?.cells[i]?.color)
		};
		if (c.numeric) {
			def.filter = 'agNumberColumnFilter';
			def.type = 'rightAligned';
		} else {
			// MME sorts text with UCase
			def.comparator = (a, b) => (upper(a) < upper(b) ? -1 : upper(a) > upper(b) ? 1 : 0);
		}
		if (sort?.col === c.id) def.sort = sort.dir;
		return def;
	});
}

type FetchFn = typeof fetch;

export class EvaluateError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'EvaluateError';
	}
}

export function createEvaluateClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async evaluate(tab: EvalTab, body: EvalRequest, signal?: AbortSignal): Promise<EvalTable> {
			const res = await fetchFn(`/api/evaluate/${tab}`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body),
				signal
			});
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new EvaluateError(res.status, reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as EvalTable;
		}
	};
}

export const evaluateClient = createEvaluateClient();

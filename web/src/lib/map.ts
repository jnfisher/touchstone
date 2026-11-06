// The graphical map's client side (M-002): the shapes `/api/map*` return (bin/touchstone/src/map.rs, from
// crates/touchstone-rules/src/map.rs), the options as MME's check boxes name them, the cell numbering the
// drawing needs, and the small rules of frmMain/frmMap's map controls. The layout itself is the server's.
import type { Settings } from './settings';

export type CellColor = 'Plain' | 'Up' | 'Down' | 'UpDown' | 'Black';
export type Marker = 'Command' | 'Npc' | 'Lair' | 'Marked' | 'Missing' | 'Overwritten' | 'Start';
export type SegmentKind =
	| 'General'
	| 'MapChange'
	| 'Trap'
	| 'Text'
	| 'DoorGate'
	| 'KeyItemToll'
	| 'Restricted'
	| 'Timed'
	| 'Hidden'
	| 'Edge';

export interface RoomRef {
	map: number;
	room: number;
}
export interface MapCell {
	cell: number;
	x: number;
	y: number;
	map: number;
	room: number;
	key: string;
	color: CellColor;
	markers: Marker[];
	replaced: RoomRef | null;
}
export interface Segment {
	cell: number;
	/** 0 to 7: N, S, E, W, NE, NW, SE, SW. */
	dir: number;
	kind: SegmentKind;
}
export interface Unplaced extends RoomRef {
	from_cell: number;
	dir: number;
	reason: 'OffGrid' | 'Duplicate';
}
export interface MapResponse {
	title: string;
	surface: string;
	start: RoomRef;
	center_cell: number;
	cols: number;
	rows: number;
	cells: MapCell[];
	segments: Segment[];
	unplaced: Unplaced[];
	missing: RoomRef[];
	overwrite_passes: number;
}

export type Walk = { Go: RoomRef } | 'Stay' | { Message: string };

export interface LeadRow {
	kind: 'Room' | 'Spell' | 'Monster';
	text: string;
	/** `map/room` for a room, the record number for the others. */
	tag: string;
}
export interface LeadsHere {
	caption: string | null;
	nothing: boolean;
	rows: LeadRow[];
}
export interface Preset {
	index: number;
	name: string;
	map: number;
	room: number;
}
export interface PresetsResponse {
	page_size: number;
	count: number;
	presets: Preset[];
}
export interface LegendEntry {
	group: 'line' | 'cell' | 'marker';
	id: string;
	label: string;
}

// ---------- surfaces ----------

export type MapSize = 'main' | '20x20' | '30x30' | '30x50' | '50x50' | 'zoom21' | 'zoom28';

/** `cmbMapSize` in frmMap's order, preceded by the main window's fixed grid. */
export const SIZES: { value: MapSize; label: string }[] = [
	{ value: 'main', label: 'Main window grid (30x23)' },
	{ value: '20x20', label: 'Map Size 20x20' },
	{ value: '30x30', label: 'Map Size 30x30' },
	{ value: '30x50', label: 'Map Size 30x50' },
	{ value: '50x50', label: 'Map Size 50x50' },
	{ value: 'zoom21', label: 'Map Size 21x21 ZOOM' },
	{ value: 'zoom28', label: 'Map Size 28x28 ZOOM' }
];
export const WINDOW_SIZES = SIZES.filter((s) => s.value !== 'main');

export function isSize(s: string | null | undefined): s is MapSize {
	return SIZES.some((x) => x.value === s);
}

/** MME's cell numbering of a surface: the north-west cell and the step between vertical neighbours. */
export function cellNumbering(size: MapSize): { first: number; stride: number } {
	if (size === 'main') return { first: 1, stride: 30 };
	if (size === 'zoom21' || size === 'zoom28') return { first: 9001, stride: 28 };
	return { first: 1, stride: 50 };
}

/** The cell number at a column and row (from 0). */
export function cellNumber(size: MapSize, x: number, y: number): number {
	const g = cellNumbering(size);
	return g.first + y * g.stride + x;
}

// ---------- options ----------

/** `chkMapOptions` and `optAlsoMark` (the MegaMUD choice is not ported), plus the tooltip switch (index 5). */
export interface MapOptionsState {
	follow: boolean;
	noHidden: boolean;
	noLairs: boolean;
	noNpcs: boolean;
	noCmds: boolean;
	dupes: boolean;
	overwrite: boolean;
	noRestricted: boolean;
	noTips: boolean;
	alsoMark: 'none' | 'shops' | 'spells';
}

/** Every box clear and "None" chosen, as both forms' designers leave them. */
export const DEFAULT_MAP_OPTIONS: MapOptionsState = {
	follow: false,
	noHidden: false,
	noLairs: false,
	noNpcs: false,
	noCmds: false,
	dupes: false,
	overwrite: false,
	noRestricted: false,
	noTips: false,
	alsoMark: 'none'
};

type BoxKey = Exclude<keyof MapOptionsState, 'alsoMark'>;

/** The check boxes in `chkMapOptions` order, with the endpoint's name and each form's caption. */
export const OPTION_BOXES: { key: BoxKey; api: string | null; main: string; window: string }[] = [
	{ key: 'follow', api: 'follow', main: 'Follow Map Changes', window: 'Follow Map Changes' },
	{ key: 'noHidden', api: 'no_hidden', main: 'Not Hidden', window: "Don't Follow Hidden" },
	{ key: 'noLairs', api: 'no_lairs', main: 'Not Lairs', window: "Don't Mark Lairs" },
	{ key: 'noNpcs', api: 'no_npcs', main: 'Not NPCs', window: "Don't Mark NPCs" },
	{ key: 'noCmds', api: 'no_cmds', main: 'Not Commands', window: "Don't Mark Commands" },
	{ key: 'noTips', api: null, main: 'No Tips', window: "Don't Show Tooltips" },
	{ key: 'dupes', api: 'dupes', main: 'Allow Dupes', window: 'Allow Dupes' },
	{ key: 'overwrite', api: 'overwrite', main: 'Allow Overwrite', window: 'Allow Overwrite' },
	{ key: 'noRestricted', api: 'no_restricted', main: 'Not Restricted', window: "Don't Follow Restricted" }
];

export const ALSO_MARK: { value: MapOptionsState['alsoMark']; label: string }[] = [
	{ value: 'none', label: 'None' },
	{ value: 'shops', label: 'Shops' },
	{ value: 'spells', label: 'Spells' }
];

/** The `options=` list of `/api/map`. */
export function optionsList(o: MapOptionsState): string {
	const out: string[] = [];
	for (const b of OPTION_BOXES) if (b.api && o[b.key]) out.push(b.api);
	if (o.alsoMark === 'shops') out.push('mark_shops');
	if (o.alsoMark === 'spells') out.push('mark_spells');
	return out.join(',');
}

/** The reverse of [`optionsList`], for the pop-out's address; unknown names are ignored. */
export function optionsFromList(list: string | null | undefined): MapOptionsState {
	const o = { ...DEFAULT_MAP_OPTIONS };
	for (const name of (list ?? '').split(',').map((s) => s.trim())) {
		const b = OPTION_BOXES.find((x) => x.api === name);
		if (b) o[b.key] = true;
		else if (name === 'mark_shops') o.alsoMark = 'shops';
		else if (name === 'mark_spells') o.alsoMark = 'spells';
	}
	// the tooltip switch is not a server option, so it travels as its own word
	if ((list ?? '').split(',').includes('no_tips')) o.noTips = true;
	return o;
}

/** The address form: the server's list plus the tooltip switch. */
export function optionsParam(o: MapOptionsState): string {
	return [optionsList(o), o.noTips ? 'no_tips' : ''].filter(Boolean).join(',');
}

export function mapQuery(
	at: RoomRef,
	size: MapSize,
	o: MapOptionsState,
	center?: number
): string {
	const p = new URLSearchParams({ map: String(at.map), room: String(at.room), size });
	const list = optionsList(o);
	if (list) p.set('options', list);
	if (center !== undefined) p.set('center', String(center));
	return p.toString();
}

/** The Rooms-tab text options the cell tooltip is built with (`chkMapOptions` 2, 3 and 4 hide what the map does not mark). */
export function tooltipRoomOptions(o: MapOptionsState) {
	return { hideNpc: o.noNpcs, hideLairs: o.noLairs, hideCmds: o.noCmds, allExits: false, illu: 0 };
}

// ---------- remembered per browser ----------

type Store = Pick<Storage, 'getItem' | 'setItem'>;

export interface MapPrefs {
	size: MapSize;
	options: MapOptionsState;
}

export const MAP_PREFS_KEYS = { tab: 'touchstone.map.tab', window: 'touchstone.map.window' } as const;

export function defaultPrefs(variant: 'tab' | 'window'): MapPrefs {
	return { size: variant === 'tab' ? 'main' : '20x20', options: { ...DEFAULT_MAP_OPTIONS } };
}

function browserStore(): Store | undefined {
	try {
		return typeof localStorage === 'undefined' ? undefined : localStorage;
	} catch {
		return undefined;
	}
}

/** What the browser remembers; anything unreadable or out of range falls back to MME's defaults. */
export function loadPrefs(variant: 'tab' | 'window', store: Store | undefined = browserStore()): MapPrefs {
	const d = defaultPrefs(variant);
	try {
		const raw = store?.getItem(MAP_PREFS_KEYS[variant]);
		if (!raw) return d;
		const v = JSON.parse(raw) as Partial<MapPrefs>;
		const size = isSize(v.size) && (variant === 'tab' || v.size !== 'main') ? v.size : d.size;
		const options = { ...d.options };
		const given = (v.options ?? {}) as Record<string, unknown>;
		for (const b of OPTION_BOXES) if (typeof given[b.key] === 'boolean') options[b.key] = given[b.key] as boolean;
		if (given.alsoMark === 'shops' || given.alsoMark === 'spells' || given.alsoMark === 'none') options.alsoMark = given.alsoMark;
		return { size, options };
	} catch {
		return d;
	}
}

export function savePrefs(variant: 'tab' | 'window', prefs: MapPrefs, store: Store | undefined = browserStore()): void {
	try {
		store?.setItem(MAP_PREFS_KEYS[variant], JSON.stringify(prefs));
	} catch {
		// private window or blocked storage: the map still works, it just forgets
	}
}

// ---------- walking ----------

/** The compass buttons in the order the buttons sit around the middle one. */
export const COMPASS = ['NW', 'N', 'NE', 'W', null, 'E', 'SW', 'S', 'SE'] as const;
export const VERTICAL = ['U', 'D'] as const;
export type Direction = 'N' | 'NE' | 'E' | 'SE' | 'S' | 'SW' | 'W' | 'NW' | 'U' | 'D';

/**
 * frmMap `Form_KeyPress` (digits as on the number pad: 8 north, 2 south, 4 west, 6 east, 7 9 1 3 the diagonals, 0 up,
 * "." down), plus the arrow keys and Page Up / Page Down. Undefined for any other key or a modified one.
 */
export function directionForKey(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'altKey' | 'metaKey'>): Direction | undefined {
	if (e.ctrlKey || e.altKey || e.metaKey) return undefined;
	switch (e.key) {
		case 'ArrowUp':
		case '8':
			return 'N';
		case 'ArrowDown':
		case '2':
			return 'S';
		case 'ArrowLeft':
		case '4':
			return 'W';
		case 'ArrowRight':
		case '6':
			return 'E';
		case '9':
			return 'NE';
		case '7':
			return 'NW';
		case '3':
			return 'SE';
		case '1':
			return 'SW';
		case '0':
		case 'PageUp':
			return 'U';
		case '.':
		case 'PageDown':
			return 'D';
		default:
			return undefined;
	}
}

/** `Segment.dir` and `Unplaced.dir` as direction names. */
export const SEGMENT_DIRS = ['N', 'S', 'E', 'W', 'NE', 'NW', 'SE', 'SW'] as const;

/** The step of each `Segment.dir` on the grid (columns right, rows down). */
export const DIR_STEP: readonly (readonly [number, number])[] = [
	[0, -1],
	[0, 1],
	[1, 0],
	[-1, 0],
	[1, -1],
	[-1, -1],
	[1, 1],
	[-1, 1]
];

// ---------- the pop-out window ----------

/** frmMap: `?map=&room=&size=&options=` (the options in [`optionsParam`] form). */
export function popoutUrl(at: RoomRef, size: MapSize, o: MapOptionsState): string {
	const p = new URLSearchParams({ map: String(at.map), room: String(at.room) });
	if (size !== 'main') p.set('size', size);
	const list = optionsParam(o);
	if (list) p.set('options', list);
	return `/map?${p}`;
}

export interface PopoutParams {
	at: RoomRef | undefined;
	size: MapSize | undefined;
	options: MapOptionsState | undefined;
}

export function parsePopout(search: string): PopoutParams {
	const p = new URLSearchParams(search);
	const n = (k: string) => {
		const v = p.get(k);
		return v !== null && /^\d{1,9}$/.test(v) ? Number(v) : 0;
	};
	const size = p.get('size');
	return {
		at: n('map') > 0 && n('room') > 0 ? { map: n('map'), room: n('room') } : undefined,
		size: isSize(size) && size !== 'main' ? size : undefined,
		options: p.has('options') ? optionsFromList(p.get('options')) : undefined
	};
}

// ---------- presets ----------

/** `cmdMapPresetSelect_Click`: ten buttons to a page, five pages. */
export const PRESET_PAGE = 10;

export function presetPage(presets: readonly Preset[], page: number): Preset[] {
	return presets.slice(page * PRESET_PAGE, page * PRESET_PAGE + PRESET_PAGE);
}

/** VB `Val` on the text boxes of frmEditPreset: digits at the start, else 0. */
function val(text: string): number {
	const m = /^\s*(\d+)/.exec(text);
	return m ? Math.min(Number(m[1]), 32767) : 0;
}

export interface PresetDraft {
	name: string;
	map: string;
	room: string;
}

/** `EditPreset` after OK: replace (or add) the entry of the preset in the saved list. */
export function withPreset(saved: Settings['map_presets'], index: number, d: PresetDraft): Settings['map_presets'] {
	const next = saved.filter((p) => p.index !== index);
	next.push({ index, name: d.name, map: val(d.map), room: val(d.room) });
	return next.sort((a, b) => a.index - b.index);
}

// ---------- the client ----------

type FetchFn = typeof fetch;

export class MapApiError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
		this.name = 'MapApiError';
	}
}

export function createMapClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function read<T>(res: Response): Promise<T> {
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new MapApiError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	return {
		map(at: RoomRef, size: MapSize, o: MapOptionsState, center?: number, signal?: AbortSignal): Promise<MapResponse> {
			return fetchFn(`/api/map?${mapQuery(at, size, o, center)}`, { signal }).then((r) => read<MapResponse>(r));
		},
		/** `window` selects frmMap's `cmdMove_Click` rules instead of frmMain's `MapGoDirection`. */
		go(at: RoomRef, dir: Direction, window: boolean, signal?: AbortSignal): Promise<Walk> {
			const p = new URLSearchParams({ map: String(at.map), room: String(at.room), dir });
			if (window) p.set('window', 'true');
			return fetchFn(`/api/map/go?${p}`, { signal }).then((r) => read<Walk>(r));
		},
		leadsHere(at: RoomRef, signal?: AbortSignal): Promise<LeadsHere | null> {
			const p = new URLSearchParams({ map: String(at.map), room: String(at.room) });
			return fetchFn(`/api/map/leads-here?${p}`, { signal }).then((r) => read<LeadsHere | null>(r));
		},
		presets(signal?: AbortSignal): Promise<PresetsResponse> {
			return fetchFn('/api/map/presets', { signal }).then((r) => read<PresetsResponse>(r));
		},
		legend(signal?: AbortSignal): Promise<LegendEntry[]> {
			return fetchFn('/api/map/legend', { signal }).then((r) => read<LegendEntry[]>(r));
		}
	};
}

export const mapClient = createMapClient();
export type MapClient = ReturnType<typeof createMapClient>;

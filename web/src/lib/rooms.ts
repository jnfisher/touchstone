// The Rooms tab (MME's text half): a room by map and room number, the room name finder, the
// rooms-with-these-exits finder and the Go Back history. The text and rows come from the server
// (`/api/rooms/...`); this module shapes the requests and keeps the history as MME does.
import type { RefTarget } from './detail';

export interface RoomRefRow {
	text: string;
	tag: string;
	/** Where `GotoLocation` would take the row; null for rows that open a command tree or nothing. */
	target: RefTarget | null;
}
export interface RoomExit {
	dir: string;
	text: string;
	kind: string;
	map: number;
	room: number;
	exit_type: string;
	/** The destination's name; null for an action text with no destination. */
	name: string | null;
}
export interface RoomInfo {
	found: boolean;
	map: number;
	room: number;
	name: string;
	/** The Rooms frame caption: "Rooms -- Name (map/room)". */
	title: string;
	light: number;
	refs_header: string;
	refs: RoomRefRow[];
	exits: RoomExit[];
	/** What the map cell's tooltip says (name, who is also here, light, items, exits, commands, lair totals). */
	tooltip: string | null;
	message: string | null;
	errors: number[];
}
export interface FindTextResult {
	matches: { map: number; room: number; name: string }[];
	truncated: boolean;
	message: string | null;
}
export interface RowsResult {
	caption: string;
	rows: RoomRefRow[];
	messages: string[];
}

/** The display boxes of the tab (`chkMapOptions`) and the character's illumination. Record numbers follow the Settings window. */
export interface RoomOptions {
	hideNpc: boolean;
	hideLairs: boolean;
	hideCmds: boolean;
	allExits: boolean;
	illu: number;
}
export const DEFAULT_ROOM_OPTIONS: RoomOptions = {
	hideNpc: false,
	hideLairs: false,
	hideCmds: false,
	allExits: false,
	illu: 0
};

/** The direction buttons of the "Find Room with Exits" window, in MME's order. */
export const EXIT_DIRS = ['N', 'S', 'E', 'W', 'NE', 'NW', 'SE', 'SW', 'U', 'D'] as const;
export type ExitDir = (typeof EXIT_DIRS)[number];

export function roomQuery(o: RoomOptions): string {
	const p = new URLSearchParams();
	if (o.hideNpc) p.set('hide_npc', 'true');
	if (o.hideLairs) p.set('hide_lairs', 'true');
	if (o.hideCmds) p.set('hide_cmds', 'true');
	if (o.allExits) p.set('all_exits', 'true');
	if (o.illu) p.set('illu', String(Math.trunc(o.illu)));
	const s = p.toString();
	return s ? `?${s}` : '';
}

/** "17/278" (spaces allowed) as map and room; undefined for anything else. */
export function parseRoomKey(s: string): { map: number; room: number } | undefined {
	const m = /^\s*(\d{1,9})\s*\/\s*(\d{1,9})\s*$/.exec(s);
	return m ? { map: Number(m[1]), room: Number(m[2]) } : undefined;
}

/** A room-command key that stands for several rooms (`17/278|17/2073-2096`) rather than one. */
export function isRoomList(key: string): boolean {
	return /[|-]/.test(key);
}

/** The `map/room` of a results row, if it names a room (command textblock rows do not). */
export function rowRoom(r: RoomRefRow): { map: number; room: number } | undefined {
	return parseRoomKey(r.tag);
}

export class RoomsError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
		this.name = 'RoomsError';
	}
}

type FetchFn = typeof fetch;

export function createRoomsClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function read<T>(res: Response): Promise<T> {
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new RoomsError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	const post = (path: string, body: unknown, signal?: AbortSignal) =>
		fetchFn(path, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(body),
			signal
		});
	return {
		async room(map: number, room: number, o: RoomOptions, signal?: AbortSignal): Promise<RoomInfo> {
			return read(await fetchFn(`/api/rooms/${map}/${room}${roomQuery(o)}`, { signal }));
		},
		async findText(q: string, limit = 100, signal?: AbortSignal): Promise<FindTextResult> {
			const p = new URLSearchParams({ q, limit: String(limit) });
			return read(await fetchFn(`/api/rooms/find-text?${p}`, { signal }));
		},
		async findExits(
			name: string,
			exact: boolean,
			exits: readonly ExitDir[],
			signal?: AbortSignal
		): Promise<RowsResult> {
			return read(await post('/api/rooms/find-exits', { name, exact, exits }, signal));
		},
		async cmdRooms(
			list: string,
			rowText: string,
			percent: string,
			signal?: AbortSignal
		): Promise<RowsResult> {
			return read(
				await post(
					'/api/rooms/cmd-rooms',
					{ list, row_text: rowText, percent },
					signal
				)
			);
		}
	};
}

export const roomsClient = createRoomsClient();

/** The "Find Room with Exits" window's rules before it searches: a name of three characters and a direction. */
export function exitSearchReady(name: string, exits: readonly ExitDir[]): boolean {
	return name.trim().length >= 3 && exits.length > 0;
}

/**
 * The Go Back list of `MapStartMapping` and `cmdRoomsButtons_Click`: the last rooms shown (21 slots,
 * the newest first). A jump records its room unless it is the newest; Go Back walks to older ones
 * without recording, wraps to the second slot after the last, and falls back to it when a slot is
 * empty.
 */
export class RoomHistory {
	private maps = new Array<number>(21).fill(0);
	private rooms = new Array<number>(21).fill(0);
	private goBack = 0;

	/** A room was shown by a normal jump (`nMap_iGoBack = 0`, then `MapStartMapping`). */
	visit(map: number, room: number): void {
		this.goBack = 0;
		if (this.maps[0] !== map || this.rooms[0] !== room) {
			for (let x = 19; x >= 0; x--) {
				this.maps[x + 1] = this.maps[x];
				this.rooms[x + 1] = this.rooms[x];
			}
		}
		this.maps[0] = map;
		this.rooms[0] = room;
	}

	/** The room Go Back goes to, or undefined when there is none (the list is empty). */
	back(): { map: number; room: number } | undefined {
		this.goBack = this.goBack >= 20 ? 1 : this.goBack + 1;
		for (;;) {
			const g = this.goBack;
			if (this.maps[g] > 0 && this.rooms[g] > 0) return { map: this.maps[g], room: this.rooms[g] };
			if (g > 1) this.goBack = 1;
			else return undefined;
		}
	}
}

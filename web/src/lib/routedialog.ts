// The route dialog (`POST /api/reachability/route`): MME's `ShowRoomRoute` and `ShowMonsterRoute`. The server builds the
// caption and the Step / Location rows; this module shapes the request and picks the level MME reads from
// `txtGlobalLevel(0)`.
import type { CharacterDoc } from './character';
import { characterReady, type GlobalFilter } from './globalfilter';

export interface RouteRow {
	step: number;
	text: string;
	/** The room the row points at. */
	map: number;
	room: number;
}

/** The dialog, or the message MME shows in a box when there is no route to list. */
export interface RouteDialogData {
	caption: string;
	rows: RouteRow[];
	message: string | null;
}

export type RouteTarget = { monster: number; alt?: boolean } | { map: number; room: number };

export interface RouteRequest {
	monster?: number;
	map?: number;
	room?: number;
	alt?: boolean;
	level: number;
}

/**
 * `val(txtGlobalLevel(0).Text)`: the open character's level while the bar mirrors it, else the bar's own level.
 */
export function routeLevel(filter: GlobalFilter, doc: CharacterDoc | undefined): number {
	return filter.use_character && characterReady(doc) ? doc.level : filter.level;
}

export function routeRequest(target: RouteTarget, level: number): RouteRequest {
	if ('monster' in target) {
		return target.alt ? { monster: target.monster, alt: true, level } : { monster: target.monster, level };
	}
	return { map: target.map, room: target.room, level };
}

export class RouteError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'RouteError';
	}
}

type FetchFn = typeof fetch;

export function createRouteClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async route(body: RouteRequest, signal?: AbortSignal): Promise<RouteDialogData> {
			const res = await fetchFn('/api/reachability/route', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body),
				signal
			});
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new RouteError(res.status, reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as RouteDialogData;
		}
	};
}

export type RouteClient = ReturnType<typeof createRouteClient>;
export const routeClient = createRouteClient();

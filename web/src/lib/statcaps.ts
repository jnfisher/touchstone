// The character editor's stat range. Stock MME takes any value up to three digits in a stat box and prices the points
// on one ladder; under the Quicksilver rules a stat can be trained past the race maximum to a soft cap and 30 points
// beyond. `GET /api/races/{n}/stat-caps` says where those limits are for a race; `/api/info`'s profile says whether
// they apply.
import { boxNumber } from './character';

export interface StatCap {
	race_max: number;
	soft_cap: number;
	train_max: number;
}

export interface StatCaps {
	/** False for stock: the boxes keep MME's plain range. */
	soft_caps: boolean;
	/** Strength, Intellect, Willpower, Agility, Health, Charm. Empty when `soft_caps` is false. */
	stats: StatCap[];
}

export class StatCapsError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'StatCapsError';
	}
}

type FetchFn = typeof fetch;

export function createStatCapsClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async caps(race: number, signal?: AbortSignal): Promise<StatCaps> {
			const res = await fetchFn(`/api/races/${race}/stat-caps`, { signal });
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new StatCapsError(res.status, reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as StatCaps;
		}
	};
}

export type StatCapsClient = ReturnType<typeof createStatCapsClient>;
export const statCapsClient = createStatCapsClient();

/** The soft cap and the training maximum of stat `i` when the load prices soft caps and the race is known. */
export function capOf(caps: StatCaps | undefined, i: number): StatCap | undefined {
	return caps?.soft_caps ? caps.stats[i] : undefined;
}

/** What a stat box takes: `boxNumber`'s three digits, and under Quicksilver no more than the training maximum. */
export function parseStat(text: string, cap: StatCap | undefined): number {
	const n = boxNumber(text);
	return cap ? Math.min(n, cap.train_max) : n;
}

/** The note beside a stat box. */
export function capNote(cap: StatCap | undefined): string {
	return cap ? `soft cap ${cap.soft_cap}, max ${cap.train_max}` : '';
}

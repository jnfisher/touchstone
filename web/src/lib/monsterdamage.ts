// The Options menu's monster damage actions (`/api/monster-damage/*`): MME's Calculate Monster Damage vs Character and
// vs Party, and Clear Calculated Monster Damage. The server runs the simulations and keeps the arrays; this module
// shapes the requests and the progress text.
import type { CharacterDoc } from './character';
import type { PartyBoxes } from './monsters';

export type DamageMode = 'char' | 'party';

/** `GET /api/monster-damage/status`. */
export interface DamageStatus {
	char: { calculated: boolean };
	party: { calculated: boolean };
	/** A calculation in progress. */
	running: { mode: DamageMode; done: number; total: number; elapsed_ms: number } | null;
	/** Counts every change of the stored arrays. */
	generation: number;
}

/** What `POST /api/monster-damage/calculate` answers. */
export interface DamageDone {
	mode: DamageMode;
	monsters: number;
	skipped: number;
	elapsed_ms: number;
}

/** One of `FilterMonsters`' "calculate the monster damage first?" questions, as the list reports it. */
export interface DamagePrompt {
	dialog: 'calc_char' | 'recalc_char' | 'calc_party' | 'recalc_party';
	title: string;
	message: string;
	mode: DamageMode;
	/** What a No sends to `/dismiss`. */
	key: string;
}

export const emptyStatus = (): DamageStatus => ({
	char: { calculated: false },
	party: { calculated: false },
	running: null,
	generation: 0
});

export interface CalcRequest {
	character: CharacterDoc;
	mode: DamageMode;
	party: PartyBoxes;
}

export function calcBody(character: CharacterDoc, mode: DamageMode, party: PartyBoxes): CalcRequest {
	return { character, mode, party };
}

/** The calculation needs the character's defenses (or the party boxes) and so a class and a race. */
export function canCalculate(doc: CharacterDoc | undefined): boolean {
	return !!doc && doc.class !== null && doc.race !== null;
}

export const MODE_LABEL: Record<DamageMode, string> = { char: 'vs character', party: 'vs party' };

/** MME's progress form caption, with the monsters done and the time so far. */
export function progressText(r: NonNullable<DamageStatus['running']>): string {
	const secs = Math.floor(r.elapsed_ms / 1000);
	return `Calculating monster damage ${MODE_LABEL[r.mode]}... ${r.done.toLocaleString('en-US')} of ${r.total.toLocaleString('en-US')} (${secs}s)`;
}

type FetchFn = typeof fetch;

export class DamageError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'DamageError';
	}
}

export function createDamageClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function send<T>(url: string, init?: RequestInit): Promise<T> {
		const res = await fetchFn(url, init);
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new DamageError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	const post = (body: unknown): RequestInit => ({
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	return {
		status: () => send<DamageStatus>('/api/monster-damage/status', { cache: 'no-store' }),
		calculate: (body: CalcRequest) => send<DamageDone>('/api/monster-damage/calculate', post(body)),
		/** Both arrays, as the menu does; `mode` clears just one. */
		clear: (mode?: DamageMode) => send<DamageStatus>('/api/monster-damage/clear', post(mode ? { mode } : {})),
		dismiss: (p: Pick<DamagePrompt, 'mode' | 'key'>) =>
			send<DamageStatus>('/api/monster-damage/dismiss', post({ mode: p.mode, key: p.key }))
	};
}

export type DamageClient = ReturnType<typeof createDamageClient>;
export const damageClient = createDamageClient();

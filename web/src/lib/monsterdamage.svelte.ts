import type { CharacterDoc } from './character';
import type { PartyBoxes } from './monsters';
import {
	calcBody,
	canCalculate,
	damageClient,
	emptyStatus,
	type DamageClient,
	type DamageMode,
	type DamagePrompt,
	type DamageStatus
} from './monsterdamage';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export interface DamageStoreOptions {
	/** How often the progress of a running calculation is read. */
	pollMs?: number;
}

export function createDamageStore(client: DamageClient = damageClient, opts: DamageStoreOptions = {}) {
	const pollMs = opts.pollMs ?? 300;
	let status = $state.raw<DamageStatus>(emptyStatus());
	/** What is being done now: a calculation (with its mode) or the clear. */
	let busy = $state<DamageMode | 'clear' | undefined>();
	let error = $state<string>();
	/** What the last run did when it was not an error: a Cancel keeps the previous values. */
	let note = $state<string>();
	/** Counts every change to the stored damage, so the Monsters list and pane read it again. */
	let version = $state(0);
	let timer: ReturnType<typeof setInterval> | undefined;
	let seenToken: number | undefined;

	async function refresh() {
		try {
			status = await client.status();
		} catch {
			// the progress read is a courtesy; the calculation's own answer carries the errors
		}
	}

	return {
		get status() {
			return status;
		},
		get busy() {
			return busy;
		},
		get error() {
			return error;
		},
		get note() {
			return note;
		},
		get version() {
			return version;
		},
		refresh,
		/**
		 * `CalculateMonsterDamageVsCharALL` for the open character or the party boxes. False when nothing ran: a
		 * calculation is already going, or a character calculation has no class and race.
		 */
		async calculate(doc: CharacterDoc | undefined, mode: DamageMode, party: PartyBoxes): Promise<boolean> {
			if (busy || !doc) return false;
			// the party calculation reads only the party boxes (MME runs it with no character loaded); the server fills in a class and race
			if (mode === 'char' && !canCalculate(doc)) return false;
			busy = mode;
			error = undefined;
			note = undefined;
			timer = setInterval(() => void refresh(), pollMs);
			let ok = false;
			try {
				const done = await client.calculate(calcBody(doc, mode, party));
				ok = !done.cancelled;
				if (done.cancelled) note = 'Monster damage calculation cancelled; the previous values are kept.';
			} catch (e) {
				error = message(e);
			} finally {
				clearInterval(timer);
				timer = undefined;
				await refresh();
				busy = undefined;
				version++;
			}
			return ok;
		},
		/** The Cancel button of the progress line: the server stops after the monster it is on. */
		async cancel(): Promise<void> {
			if (!busy || busy === 'clear') return;
			try {
				status = await client.cancel();
			} catch (e) {
				error = message(e);
			}
		},
		/** `ClearMonsterDamageVsCharALL` for both arrays: the menu's "Clear Calculated Monster Damage". */
		async clear(): Promise<void> {
			if (busy) return;
			busy = 'clear';
			error = undefined;
			try {
				status = await client.clear();
			} catch (e) {
				error = message(e);
			} finally {
				busy = undefined;
				version++;
			}
		},
		/** The question was answered No: it is not asked again until the defenses (or the party boxes) change. */
		async dismiss(prompt: DamagePrompt): Promise<void> {
			try {
				status = await client.dismiss(prompt);
			} catch (e) {
				error = message(e);
			}
			version++;
		},
		/**
		 * MME clears the arrays when a character is loaded or pasted and when the fields are reset, but not on an
		 * edit. `token` changes with each of those; the first call only learns it.
		 */
		async characterChanged(token: number): Promise<void> {
			const was = seenToken;
			seenToken = token;
			if (was === undefined || was === token) return;
			await this.clear();
		}
	};
}

export type DamageStore = ReturnType<typeof createDamageStore>;

let shared: DamageStore | undefined;

/** One store for the page: the Monsters list, its pane and the Options menu read it. */
export function getDamageStore(): DamageStore {
	return (shared ??= createDamageStore());
}

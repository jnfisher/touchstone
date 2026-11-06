import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { newCharacter } from './character';
import { DEFAULT_PARTY } from './monsters';
import { emptyStatus, type DamageClient, type DamagePrompt, type DamageStatus } from './monsterdamage';
import { createDamageStore } from './monsterdamage.svelte';

const ready = { ...newCharacter('x'), class: 1, race: 2 };

function fake() {
	let release: (() => void) | undefined;
	let status: DamageStatus = emptyStatus();
	let cancelled = false;
	const client = {
		status: vi.fn(async () => structuredClone(status)),
		calculate: vi.fn(
			(body: { mode: 'char' | 'party' }) =>
				new Promise((resolve) => {
					status = { ...status, running: { mode: body.mode, done: 40, total: 100, elapsed_ms: 2100 } };
					release = () => {
						// a cancelled run stores nothing: the status is as it was before
						if (cancelled) {
							status = emptyStatus();
							resolve({ mode: body.mode, monsters: 100, skipped: 0, elapsed_ms: 900, cancelled: true });
							return;
						}
						status = { ...emptyStatus(), char: { calculated: true }, generation: 1 };
						resolve({ mode: body.mode, monsters: 100, skipped: 0, elapsed_ms: 3000 });
					};
				})
		),
		cancel: vi.fn(async () => {
			if (status.running) status = { ...status, running: { ...status.running, cancelling: true } };
			cancelled = true;
			return structuredClone(status);
		}),
		clear: vi.fn(async () => {
			status = emptyStatus();
			return structuredClone(status);
		}),
		dismiss: vi.fn(async () => structuredClone(status))
	};
	return { client: client as unknown as DamageClient, raw: client, finish: () => release?.() };
}

describe('monster damage store', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('shows the progress while the server calculates, then the new state, and counts the change', async () => {
		const { client, raw, finish } = fake();
		const store = createDamageStore(client, { pollMs: 100 });
		expect(store.version).toBe(0);
		const run = store.calculate(ready, 'char', DEFAULT_PARTY);
		expect(store.busy).toBe('char');
		expect(store.status.running).toBeNull();
		await vi.advanceTimersByTimeAsync(250);
		expect(store.status.running).toMatchObject({ mode: 'char', done: 40, total: 100 });
		expect(raw.calculate).toHaveBeenCalledTimes(1);
		// a second click while it runs does nothing
		expect(await store.calculate(ready, 'party', DEFAULT_PARTY)).toBe(false);
		finish();
		expect(await run).toBe(true);
		expect(store.busy).toBeUndefined();
		expect(store.status.char.calculated).toBe(true);
		expect(store.status.running).toBeNull();
		expect(store.version).toBe(1);
		// polling stopped
		const reads = raw.status.mock.calls.length;
		await vi.advanceTimersByTimeAsync(1000);
		expect(raw.status.mock.calls.length).toBe(reads);
	});

	it('cancels a running calculation, keeps the old values and says so', async () => {
		const { client, raw, finish } = fake();
		const store = createDamageStore(client, { pollMs: 100 });
		const run = store.calculate(ready, 'char', DEFAULT_PARTY);
		await vi.advanceTimersByTimeAsync(250);
		await store.cancel();
		expect(raw.cancel).toHaveBeenCalledTimes(1);
		// the progress line reads "cancelling" until the server stops
		expect(store.status.running?.cancelling).toBe(true);
		finish();
		expect(await run).toBe(false);
		expect(store.status.char.calculated).toBe(false);
		expect(store.error).toBeUndefined();
		expect(store.note).toMatch(/cancelled/);
		expect(store.busy).toBeUndefined();
		// the next run clears the note
		void store.calculate(ready, 'party', DEFAULT_PARTY);
		expect(store.note).toBeUndefined();
	});

	it('has nothing to cancel when nothing runs, or only a clear', async () => {
		const { client, raw } = fake();
		const store = createDamageStore(client);
		await store.cancel();
		expect(raw.cancel).not.toHaveBeenCalled();
	});

	it('does not calculate without a complete character', async () => {
		const { client, raw } = fake();
		const store = createDamageStore(client);
		expect(await store.calculate(undefined, 'char', DEFAULT_PARTY)).toBe(false);
		expect(await store.calculate(newCharacter('x'), 'char', DEFAULT_PARTY)).toBe(false);
		expect(raw.calculate).not.toHaveBeenCalled();
		expect(store.version).toBe(0);
	});

	it('calculates for the party with no class and race, as MME does with no character loaded', async () => {
		const { client, raw, finish } = fake();
		const store = createDamageStore(client);
		const run = store.calculate(newCharacter('x'), 'party', DEFAULT_PARTY);
		finish();
		expect(await run).toBe(true);
		expect(raw.calculate).toHaveBeenCalledOnce();
		expect(store.version).toBe(1);
	});

	it('keeps the server text when a calculation fails and still refreshes the list', async () => {
		const { client, raw } = fake();
		raw.calculate.mockRejectedValueOnce(new Error('the calculation failed: overflow'));
		const store = createDamageStore(client);
		expect(await store.calculate(ready, 'char', DEFAULT_PARTY)).toBe(false);
		expect(store.error).toBe('the calculation failed: overflow');
		expect(store.busy).toBeUndefined();
		expect(store.version).toBe(1);
	});

	it('clears both arrays and dismisses a question', async () => {
		const { client, raw } = fake();
		const store = createDamageStore(client);
		await store.clear();
		expect(raw.clear).toHaveBeenCalledTimes(1);
		expect(store.version).toBe(1);
		const prompt: DamagePrompt = { dialog: 'calc_char', title: 't', message: 'm', mode: 'char', key: 'k' };
		await store.dismiss(prompt);
		expect(raw.dismiss).toHaveBeenCalledWith(prompt);
		expect(store.version).toBe(2);
	});

	it('clears when a different character was loaded or pasted, not on the first look', async () => {
		const { client, raw } = fake();
		const store = createDamageStore(client);
		await store.characterChanged(3);
		await store.characterChanged(3);
		expect(raw.clear).not.toHaveBeenCalled();
		await store.characterChanged(4);
		expect(raw.clear).toHaveBeenCalledTimes(1);
	});
});

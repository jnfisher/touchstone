import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ACTIVE_KEY, copyToSandbox, createActiveCharacter, createCharacterStore, parseChoice } from './character.svelte';
import { newCharacter, type CharacterClient, type CharacterDoc } from './character';
import { createRealmStore } from './realm.svelte';
import type { RealmClient, RealmList } from './realm';
import { cormac, roster } from './realm.fixture';

const input = (name: string, level: number): CharacterDoc => ({ ...newCharacter(name), class: 5, race: 2, level });

function setup(saved: string[] = [], remembered?: string) {
	const files = new Map<string, CharacterDoc>(saved.map((n) => [n, newCharacter(n)]));
	const chars = {
		evaluate: vi.fn().mockResolvedValue({ hp: { caption: 'ok' } }),
		blessSpells: vi.fn().mockResolvedValue([]),
		list: vi.fn(async () => [...files.keys()].map((name) => ({ name, race: null, class: null, level: 1 }))),
		get: vi.fn(),
		put: vi.fn(async (n: string, d: CharacterDoc) => void files.set(n, d)),
		remove: vi.fn(),
		importMmec: vi.fn()
	};
	let list: RealmList = { available: true, characters: roster };
	let inputs: Record<number, CharacterDoc> = { 3: input('Cormac', 16), 1: input('Aldric', 1) };
	const client = {
		list: vi.fn(async () => list),
		detail: vi.fn(async () => cormac),
		input: vi.fn(async (id: number) => structuredClone(inputs[id]))
	};
	const mem = new Map<string, string>(remembered ? [[ACTIVE_KEY, remembered]] : []);
	const storage = { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v) };
	const sandbox = createCharacterStore(chars as unknown as CharacterClient);
	const live = createCharacterStore(chars as unknown as CharacterClient, { readOnly: true });
	const realm = createRealmStore(client as unknown as RealmClient);
	const active = createActiveCharacter({ sandbox, live, realm, client, storage });
	return {
		chars,
		files,
		client,
		mem,
		sandbox,
		live,
		realm,
		active,
		setList: (l: RealmList) => (list = l),
		setInputs: (i: Record<number, CharacterDoc>) => (inputs = i)
	};
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('active character', () => {
	it('is the sandbox until a realm character is picked, and parses what it stored', () => {
		const s = setup();
		expect(s.active.choice).toEqual({ kind: 'sandbox' });
		s.sandbox.edit((d) => (d.name = 'Mine'));
		expect(s.active.store.doc.name).toBe('Mine');
		expect(parseChoice('realm:12')).toEqual({ kind: 'realm', id: 12 });
		expect(parseChoice('garbage')).toEqual({ kind: 'sandbox' });
		expect(parseChoice(null)).toEqual({ kind: 'sandbox' });
	});

	it('resolves a realm character from the API input, and persists the choice', async () => {
		const s = setup();
		s.active.select({ kind: 'realm', id: 3 });
		expect(s.active.store.doc.name).toBe(''); // blank until the input lands
		await vi.advanceTimersByTimeAsync(0);
		expect(s.client.input).toHaveBeenCalledWith(3);
		expect(s.active.store.doc).toMatchObject({ name: 'Cormac', level: 16, class: 5, race: 2 });
		expect(s.mem.get(ACTIVE_KEY)).toBe('realm:3');
		await vi.advanceTimersByTimeAsync(300);
		expect(s.active.store.evaluated).toEqual({ hp: { caption: 'ok' } });
		// the realm character is never saved
		s.active.store.edit((d) => (d.level = 17));
		await vi.advanceTimersByTimeAsync(2000);
		expect(s.chars.put).not.toHaveBeenCalled();
		expect(s.sandbox.doc.level).not.toBe(17);
		// going back leaves the sandbox as it was
		s.active.select({ kind: 'sandbox' });
		expect(s.mem.get(ACTIVE_KEY)).toBe('sandbox');
		expect(s.active.store.doc).toBe(s.sandbox.doc);
	});

	it('restores the remembered character at start and falls back when it is gone', async () => {
		const s = setup([], 'realm:3');
		expect(s.active.realmId).toBe(3);
		await s.active.start();
		expect(s.active.store.doc.name).toBe('Cormac');

		const gone = setup([], 'realm:99');
		await gone.active.start();
		expect(gone.active.choice).toEqual({ kind: 'sandbox' });
		expect(gone.mem.get(ACTIVE_KEY)).toBe('sandbox');
	});

	it('falls back to the sandbox when the source has no realm', async () => {
		const s = setup([], 'realm:3');
		s.setList({ available: false, characters: [] });
		await s.active.start();
		expect(s.active.choice.kind).toBe('sandbox');
	});

	it('refreshes the realm character when the realm reloads, keeping it when unchanged', async () => {
		const s = setup();
		s.active.select({ kind: 'realm', id: 3 });
		await vi.advanceTimersByTimeAsync(0);
		const epoch = s.active.store.replaceEpoch;
		await s.active.sync();
		expect(s.active.store.replaceEpoch).toBe(epoch); // same input: nothing replaced
		s.setInputs({ 3: input('Cormac', 17) });
		await s.active.sync();
		expect(s.active.store.doc.level).toBe(17);
		expect(s.active.store.replaceEpoch).toBeGreaterThan(epoch);
	});

	it('replacing the whole character through the shared store makes the sandbox active', async () => {
		const s = setup();
		s.active.select({ kind: 'realm', id: 3 });
		await vi.advanceTimersByTimeAsync(0);
		await s.active.store.applyPasted({ ...newCharacter('Pasted'), class: 1, race: 1 });
		expect(s.active.choice.kind).toBe('sandbox');
		expect(s.active.store.doc.name).toBe('Pasted');
	});

	it('a failed input read is reported and keeps the app alive', async () => {
		const s = setup();
		s.client.input.mockRejectedValueOnce(new Error('boom'));
		s.active.select({ kind: 'realm', id: 3 });
		await vi.advanceTimersByTimeAsync(0);
		expect(s.active.inputError).toBe('boom');
	});
});

describe('copy to sandbox', () => {
	it('loads the realm input into the editor under a free name and leaves the active choice', async () => {
		const s = setup(['Cormac']);
		const name = await copyToSandbox(s.sandbox, input('Cormac', 16));
		expect(name).toBe('Cormac (copy)');
		expect(s.sandbox.doc).toMatchObject({ name: 'Cormac (copy)', level: 16, class: 5 });
		expect(s.active.choice.kind).toBe('sandbox'); // untouched
		await vi.advanceTimersByTimeAsync(1000);
		expect([...s.files.keys()].sort()).toEqual(['Cormac', 'Cormac (copy)']);
	});
});

describe('realm store', () => {
	it('loads once, selects, and refreshes the list and the open sheet', async () => {
		const s = setup();
		await s.realm.load();
		await s.realm.load();
		expect(s.client.list).toHaveBeenCalledTimes(1);
		expect(s.realm.available).toBe(true);
		await s.realm.select(3);
		expect(s.realm.detail?.id).toBe(3);
		s.setList({ available: true, characters: roster.filter((c) => c.id !== 3) });
		await s.realm.refresh();
		expect(s.realm.selected).toBeUndefined();
		expect(s.realm.detail).toBeUndefined();
	});
});

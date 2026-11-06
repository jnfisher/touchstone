import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CharacterClient, CharacterDoc } from './character';
import { createCharacterStore } from './character.svelte';

function fakeClient() {
	const files = new Map<string, CharacterDoc>();
	const client = {
		evaluate: vi.fn().mockResolvedValue({ hp: { caption: 'ok' } }),
		blessSpells: vi.fn().mockResolvedValue([]),
		list: vi.fn(async () => [...files.keys()].map((name) => ({ name, race: null, class: null, level: 1 }))),
		get: vi.fn(async (n: string) => structuredClone(files.get(n)!)),
		put: vi.fn(async (n: string, d: CharacterDoc) => {
			files.set(n, structuredClone({ ...d, name: n }));
			return d;
		}),
		remove: vi.fn(async (n: string) => void files.delete(n))
	};
	return { client: client as unknown as CharacterClient, files, raw: client };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('character store', () => {
	it('debounces evaluation and skips it without class and race', async () => {
		const { client, raw } = fakeClient();
		const s = createCharacterStore(client);
		s.edit((d) => (d.level = 5));
		await vi.advanceTimersByTimeAsync(500);
		expect(raw.evaluate).not.toHaveBeenCalled();
		s.edit((d) => ((d.class = 1), (d.race = 1)));
		s.edit((d) => (d.level = 6));
		s.edit((d) => (d.level = 7));
		await vi.advanceTimersByTimeAsync(200);
		expect(raw.evaluate).toHaveBeenCalledTimes(1);
		expect(raw.evaluate.mock.calls[0][0]).toMatchObject({ level: 7, class: 1, race: 1 });
		expect(s.evaluated).toEqual({ hp: { caption: 'ok' } });
	});

	it('shows the server error inline and clears it on success', async () => {
		const { client, raw } = fakeClient();
		raw.evaluate.mockRejectedValueOnce(new Error('level must be 0 to 999'));
		const s = createCharacterStore(client);
		s.edit((d) => ((d.class = 1), (d.race = 1)));
		await vi.advanceTimersByTimeAsync(200);
		expect(s.evalError).toBe('level must be 0 to 999');
		s.edit((d) => (d.level = 3));
		await vi.advanceTimersByTimeAsync(200);
		expect(s.evalError).toBeUndefined();
	});

	it('does not save without a valid name, then autosaves under it', async () => {
		const { client, raw, files } = fakeClient();
		const s = createCharacterStore(client);
		s.edit((d) => (d.level = 9));
		await vi.advanceTimersByTimeAsync(2000);
		expect(raw.put).not.toHaveBeenCalled();
		s.edit((d) => (d.name = 'Bob'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(raw.put).toHaveBeenCalledTimes(1);
		expect(files.get('Bob')?.level).toBe(9);
		expect(s.saveStatus).toBe('saved');
		expect(s.saved.map((x) => x.name)).toEqual(['Bob']);
	});

	it('removes the old file on rename', async () => {
		const { client, files } = fakeClient();
		const s = createCharacterStore(client);
		s.edit((d) => (d.name = 'Bob'));
		await s.flush();
		s.edit((d) => (d.name = 'Rob'));
		await s.flush();
		expect([...files.keys()]).toEqual(['Rob']);
	});

	it('reports a failed save', async () => {
		const { client, raw } = fakeClient();
		raw.put.mockRejectedValueOnce(new Error('disk full'));
		const s = createCharacterStore(client);
		s.edit((d) => (d.name = 'Bob'));
		await s.flush();
		expect(s.saveStatus).toBe('error');
		expect(s.saveError).toBe('disk full');
	});

	it('loads, starts new and deletes', async () => {
		const { client, files } = fakeClient();
		const s = createCharacterStore(client);
		s.edit((d) => ((d.name = 'Bob'), (d.level = 4)));
		await s.flush();
		s.startNew();
		expect(s.doc.name).toBe('');
		await s.load('Bob');
		expect(s.doc.level).toBe(4);
		expect(s.storedName).toBe('Bob');
		await s.remove();
		expect(files.size).toBe(0);
		expect(s.doc.name).toBe('');
		expect(s.saved).toEqual([]);
	});
});

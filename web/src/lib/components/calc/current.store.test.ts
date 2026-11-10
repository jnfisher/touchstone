import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createCharacterStore, createActiveCharacter } from '$lib/character.svelte';
import { newCharacter, type CharacterClient, type CharacterDoc } from '$lib/character';
import { createRealmStore } from '$lib/realm.svelte';
import type { RealmClient } from '$lib/realm';
import { roster } from '$lib/realm.fixture';
import { createCurrentCharacter } from './current.svelte';

const doc = (name: string, level: number): CharacterDoc => ({ ...newCharacter(name), class: 5, race: 2, level });

function setup() {
	const chars = {
		evaluate: vi.fn().mockResolvedValue({ hp: { caption: 'ok' }, stats: [], stat_values: [0, 0, 0, 0, 0, 0] }),
		list: vi.fn(async () => []),
		put: vi.fn(),
		get: vi.fn(),
		remove: vi.fn(),
		importMmec: vi.fn(),
		blessSpells: vi.fn().mockResolvedValue([])
	};
	const inputs: Record<number, CharacterDoc> = { 3: doc('Cormac', 16), 1: doc('Aldric', 1) };
	const client = {
		list: vi.fn(async () => ({ available: true, characters: roster })),
		detail: vi.fn(),
		input: vi.fn(async (id: number) => structuredClone(inputs[id]))
	};
	const sandbox = createCharacterStore(chars as unknown as CharacterClient);
	const live = createCharacterStore(chars as unknown as CharacterClient, { readOnly: true });
	const realm = createRealmStore(client as unknown as RealmClient);
	const active = createActiveCharacter({ sandbox, live, realm, client, storage: { getItem: () => null, setItem: () => undefined } });
	const make = () =>
		createCurrentCharacter({
			store: active.store,
			activeRealmId: () => active.realmId,
			realm,
			client,
			makeOverride: () => createCharacterStore(chars as unknown as CharacterClient, { readOnly: true })
		});
	return { chars, client, sandbox, realm, active, make };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('current character of a calculator', () => {
	it('follows the active character', async () => {
		const s = setup();
		await s.realm.load();
		const cur = s.make();
		s.sandbox.edit((d) => (d.name = 'Mine'));
		expect(cur.name).toBe('Mine');
		expect(cur.source).toMatchObject({ via: 'active', name: 'Mine' });
		s.active.select({ kind: 'realm', id: 3 });
		await vi.advanceTimersByTimeAsync(0);
		expect(cur.name).toBe('Cormac');
		expect(cur.source).toMatchObject({ via: 'active', realmId: 3, name: 'Cormac Fixture', level: 16 });
		expect(cur.source.kind).toContain(roster.find((r) => r.id === 3)!.class_name);
	});

	it('can borrow another realm character for itself only, and go back', async () => {
		const s = setup();
		await s.realm.load();
		const a = s.make();
		const b = s.make();
		s.sandbox.edit((d) => (d.name = 'Mine'));
		await a.useAnother(1);
		await vi.advanceTimersByTimeAsync(300);
		expect(a.name).toBe('Aldric');
		expect(a.level).toBe(1);
		expect(a.source.via).toBe('override');
		// the other calculator and the active character are untouched
		expect(b.name).toBe('Mine');
		expect(s.active.store.doc.name).toBe('Mine');
		a.clearOverride();
		expect(a.name).toBe('Mine');
		expect(a.override).toBeUndefined();
	});

	it('never writes a borrowed character back', async () => {
		const s = setup();
		await s.realm.load();
		const a = s.make();
		await a.useAnother(3);
		await vi.advanceTimersByTimeAsync(5000);
		expect(s.chars.put).not.toHaveBeenCalled();
	});

	it('drops a borrowed character that left the realm, and reports a failed read', async () => {
		const s = setup();
		await s.realm.load();
		const a = s.make();
		await a.useAnother(3);
		s.client.list.mockResolvedValue({ available: true, characters: roster.filter((r) => r.id !== 3) });
		await s.realm.refresh();
		await a.reload();
		expect(a.override).toBeUndefined();
		s.client.input.mockRejectedValueOnce(new Error('gone'));
		await a.useAnother(1);
		expect(a.overrideError).toBe('gone');
	});

	it('opens the sheet of the character in use', async () => {
		const s = setup();
		await s.realm.load();
		const a = s.make();
		const go = vi.fn();
		a.openSheet(go);
		expect(s.realm.view).toBe('sandbox');
		await a.useAnother(3);
		a.openSheet(go);
		expect(s.realm.view).toBe('realm');
		expect(s.realm.selected).toBe(3);
		expect(go).toHaveBeenLastCalledWith('/character');
	});
});

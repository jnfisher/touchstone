import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { newCharacter, type CharacterClient, type CharacterDoc } from './character';
import { createCharacterStore } from './character.svelte';
import { createCompareStore } from './compare.svelte';
import { emptyLists, type Classified, type ExtraEntry } from './compare';

function fakeClient() {
	const files = new Map<string, CharacterDoc>();
	const importMmec = vi.fn();
	const client = {
		evaluate: vi.fn().mockResolvedValue({}),
		blessSpells: vi.fn().mockResolvedValue([]),
		list: vi.fn(async () => [...files.keys()].map((name) => ({ name, race: null, class: null, level: 1 }))),
		get: vi.fn(async (n: string) => structuredClone(files.get(n)!)),
		put: vi.fn(async (n: string, d: CharacterDoc) => {
			files.set(n, structuredClone({ ...d, name: n }));
			return d;
		}),
		remove: vi.fn(async (n: string) => void files.delete(n)),
		importMmec
	};
	return { client: client as unknown as CharacterClient, files, importMmec };
}

/** The [Compare] section the server writes on export, from the document's `extra` (SaveCharacter's order). */
function compareSection(doc: CharacterDoc): string {
	const rows = (doc.extra ?? []).filter((e) => e.section === 'Compare');
	return rows.length ? '[Compare]\r\n' + rows.map((e) => `${e.key}=${e.value}\r\n`).join('') : '';
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const classify = (kinds: Record<number, Classified['kind']>) => ({
	classify: vi.fn(async (items: number[]) =>
		items.map((n) => ({ number: n, kind: kinds[n] ?? 'missing', worn: false }) as Classified)
	)
});

describe('compare store', () => {
	it('keeps the lists in the character and saves them with it', async () => {
		const { client, files } = fakeClient();
		const chars = createCharacterStore(client);
		const cmp = createCompareStore(chars, classify({}));
		chars.edit((d) => (d.name = 'Tester'));
		cmp.toggle('weapons', 310);
		cmp.toggle('weapons', 311);
		cmp.toggle('monsters', 7);
		cmp.toggle('weapons', 310); // already there: off
		expect(cmp.lists).toEqual({ ...emptyLists(), weapons: [311], monsters: [7] });
		expect(cmp.count('weapons')).toBe(1);
		expect(cmp.has('monsters', 7)).toBe(true);
		await chars.flush();
		const saved = files.get('Tester')!;
		const entry = (key: string) => saved.extra!.find((e: ExtraEntry) => e.section === 'Compare' && e.key === key)?.value;
		expect(entry('WeaponCompare')).toBe('311,');
		expect(entry('MonsterCompare')).toBe('7,');
		expect(compareSection(saved)).toBe(
			'[Compare]\r\nWeaponCompare=311,\r\nArmourCompare=\r\nSpellCompare=\r\nMonsterCompare=7,\r\n'
		);
	});

	it('removes the selected entry and clears one list or all four', () => {
		const chars = createCharacterStore(fakeClient().client);
		const cmp = createCompareStore(chars, classify({}));
		for (const n of [1, 2, 3]) cmp.toggle('spells', n);
		cmp.toggle('armour', 9);
		cmp.removeAt('spells', 1);
		expect(cmp.lists.spells).toEqual([1, 3]);
		cmp.clear('spells');
		expect(cmp.lists).toEqual({ ...emptyLists(), armour: [9] });
		cmp.toggle('monsters', 4);
		cmp.clearAll();
		expect(cmp.lists).toEqual(emptyLists());
	});

	it('adds everything equipped to the weapon and armour lists', async () => {
		const chars = createCharacterStore(fakeClient().client);
		const stub = classify({ 10: 'armour', 11: 'armour', 20: 'weapon', 30: 'other' });
		const cmp = createCompareStore(chars, stub);
		chars.edit((d) => {
			d.equipped = d.equipped.map((_, i) => ({ 0: 10, 3: 20, 5: 30, 6: 11, 7: 11 })[i] ?? 0);
		});
		await cmp.addEquipped();
		// 11 is in two slots (two rings): CompareAddItem toggles, so the second one takes the first off again
		expect(cmp.lists).toEqual({ ...emptyLists(), weapons: [20], armour: [10] });
		// the same record in two slots is asked about once
		expect(stub.classify).toHaveBeenCalledWith([10, 20, 30, 11]);
	});

	it('loads an imported character with its lists and carries them through the next save', async () => {
		// fixtures/data/mmec/warrior.mmec: the [Compare] section as MME wrote it, in the DTO the import answers
		const { client, files, importMmec } = fakeClient();
		const chars = createCharacterStore(client);
		const cmp = createCompareStore(chars, classify({}));
		importMmec.mockResolvedValue({
			...newCharacter(),
			name: 'Imported',
			extra: [
				{ section: 'MonsterFilter', key: 'HP', value: '123' },
				{ section: 'Compare', key: 'WeaponCompare', value: '310,311,' },
				{ section: 'Compare', key: 'ArmourCompare', value: '101,205,' },
				{ section: 'Compare', key: 'SpellCompare', value: '' },
				{ section: 'Compare', key: 'MonsterCompare', value: '' }
			]
		});
		expect(await chars.importMmec({} as never)).toBe('ok');
		expect(cmp.lists).toEqual({ weapons: [310, 311], armour: [101, 205], spells: [], monsters: [] });

		cmp.toggle('weapons', 48);
		cmp.removeAt('armour', 0);
		await chars.flush();
		const saved = files.get('Imported')!;
		// the entries the page does not own are kept, in place
		expect(saved.extra![0]).toEqual({ section: 'MonsterFilter', key: 'HP', value: '123' });
		expect(compareSection(saved)).toBe(
			'[Compare]\r\nWeaponCompare=310,311,48,\r\nArmourCompare=205,\r\nSpellCompare=\r\nMonsterCompare=\r\n'
		);
		// opening it again reads the same lists back
		await chars.load('Imported');
		expect(cmp.lists).toEqual({ weapons: [310, 311, 48], armour: [205], spells: [], monsters: [] });
	});

	it('tells which sub-tab is showing', () => {
		const cmp = createCompareStore(createCharacterStore(fakeClient().client), classify({}));
		expect(cmp.active).toBe('weapons');
		cmp.setActive('monsters');
		expect(cmp.active).toBe('monsters');
	});
});

import { describe, expect, it, vi } from 'vitest';
import type { Item } from './api';
import {
	boxNumber,
	createCharacterClient,
	CharacterError,
	derivedStatName,
	EQUIP_SLOTS,
	evalRequest,
	filterChoices,
	fromServer,
	nameProblem,
	newCharacter,
	QUESTS,
	slotChoices
} from './character';

const item = (o: Partial<Item>): Item => ({ number: 1, name: 'x', item_type: 0, worn: 0, in_game: true, ...o }) as Item;

describe('character mapping', () => {
	it('has 20 slots with the weapon at 16 and 12 quests', () => {
		expect(EQUIP_SLOTS).toHaveLength(20);
		expect(EQUIP_SLOTS[16].label).toBe('Weapon');
		expect(QUESTS).toHaveLength(12);
		expect(QUESTS.filter((q) => q.option !== undefined).map((q) => q.index)).toEqual([5, 9, 10, 11]);
	});

	it('does not evaluate until class and race are picked', () => {
		const c = newCharacter('A');
		const o = { hide_class_race: false, min_item_level: 0 };
		expect(evalRequest(c, o)).toBeUndefined();
		c.class = 1;
		expect(evalRequest(c, o)).toBeUndefined();
		c.race = 2;
		expect(evalRequest(c, o)).toMatchObject({ class: 1, race: 2, options: o });
	});

	it('pads short stored lists and keeps the rest', () => {
		const c = fromServer({ name: 'B', equipped: [5], bless: [7, 8], quests: [true] });
		expect(c.equipped).toHaveLength(20);
		expect(c.equipped[0]).toBe(5);
		expect(c.bless.slice(0, 3)).toEqual([7, 8, 0]);
		expect(c.quests[0]).toBe(true);
		expect(c.quests).toHaveLength(12);
		expect(c.attack.heal_rounds).toBe(30);
	});

	it('mirrors the server name rule', () => {
		for (const ok of ['Bob', 'Mr. Bob', 'Zoë']) expect(nameProblem(ok)).toBeUndefined();
		for (const bad of ['', '.x', 'a/b', 'a\\b', 'a:b', 'a.', 'a ', ' a', 'a\nb', 'x'.repeat(65)])
			expect(nameProblem(bad), JSON.stringify(bad)).toBeDefined();
	});

	it('reads boxes like Val and clamps to three digits', () => {
		expect(boxNumber('12abc')).toBe(12);
		expect(boxNumber('abc')).toBe(0);
		expect(boxNumber('5000')).toBe(999);
		expect(boxNumber('-4')).toBe(0);
	});

	it('names the derived stats', () => {
		expect(derivedStatName(2)).toBe('Armour Class');
		expect(derivedStatName(45)).toBe('Stat 45');
	});

	it('offers slot items by worn value, weapons for the weapon slot', () => {
		const items = [
			item({ number: 1, name: 'helm', worn: 2 }),
			item({ number: 2, name: 'boots', worn: 5 }),
			item({ number: 3, name: 'sword', item_type: 1 }),
			item({ number: 4, name: 'hidden helm', worn: 2, in_game: false })
		];
		expect(slotChoices(items, EQUIP_SLOTS[0]).map((c) => c.name)).toEqual(['helm']);
		expect(slotChoices(items, EQUIP_SLOTS[16]).map((c) => c.name)).toEqual(['sword', ]);
		expect(slotChoices(items, EQUIP_SLOTS[14]).map((c) => c.name)).toEqual(['boots', 'helm']);
		expect(filterChoices(slotChoices(items, EQUIP_SLOTS[14]), 'HEL')).toHaveLength(1);
	});
});

describe('character client', () => {
	const ok = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

	it('posts evaluate and surfaces the server reason', async () => {
		const f = vi.fn().mockResolvedValueOnce(ok({ ok: 1 })).mockResolvedValueOnce(new Response('pick a class and race', { status: 422 }));
		const c = createCharacterClient(f);
		await c.evaluate({ a: 1 });
		expect(f.mock.calls[0][0]).toBe('/api/character/evaluate');
		expect(f.mock.calls[0][1].method).toBe('POST');
		await expect(c.evaluate({})).rejects.toMatchObject({ status: 422, message: 'pick a class and race' });
		await expect(c.evaluate({})).rejects.toBeInstanceOf(Error);
		void CharacterError;
	});

	it('builds the bless query and encodes names', async () => {
		const f = vi.fn().mockImplementation(() => Promise.resolve(ok([])));
		const c = createCharacterClient(f);
		await c.blessSpells({ class: 3, level: 10, alignment: 1, all: true, include: [0, 5, 6] });
		expect(f.mock.calls[0][0]).toBe('/api/character/bless-spells?class=3&level=10&alignment=1&all=true&include=5%2C6');
		await c.get('Mr. Bob/x');
		expect(f.mock.calls[1][0]).toBe('/api/characters/Mr.%20Bob%2Fx');
	});

	it('treats 204 as empty', async () => {
		const f = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
		await expect(createCharacterClient(f).remove('a')).resolves.toBeUndefined();
	});
});

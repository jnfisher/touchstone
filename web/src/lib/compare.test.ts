import { describe, expect, it, vi } from 'vitest';
import {
	addEquipped,
	COMPARE_KINDS,
	createCompareClient,
	emptyLists,
	formatList,
	MAX_SAVED,
	parseList,
	readLists,
	toggled,
	withoutAt,
	writeLists,
	type ExtraEntry
} from './compare';

describe('saved list text', () => {
	it('reads comma-terminated numbers as the loaders do', () => {
		// LoadCharacter / ReloadMonsterCompare walk the string comma by comma
		expect(parseList('310,311,')).toEqual([310, 311]);
		expect(parseList('')).toEqual([]);
		// text after the last comma is never read
		expect(parseList('5,6,7')).toEqual([5, 6]);
		// Val() of a non-number is 0, which is no record
		expect(parseList('5,x,0,-3,7,')).toEqual([5, 7]);
		expect(parseList('12abc,3.9,')).toEqual([12, 3]);
	});

	it('writes each number and a comma, 101 at most', () => {
		// SaveCharacter: str = str & oLI.Text & ","; the loop leaves after the 101st
		expect(formatList([1, 2, 3])).toBe('1,2,3,');
		expect(formatList([])).toBe('');
		const long = Array.from({ length: 150 }, (_, i) => i + 1);
		expect(parseList(formatList(long))).toHaveLength(MAX_SAVED);
	});
});

describe('list changes', () => {
	it('takes a record off when it is already there and puts it on the end otherwise', () => {
		// CompareAddItem: an item already on the list goes through RemovePopupItemCompare
		expect(toggled([1, 2], 3)).toEqual([1, 2, 3]);
		expect(toggled([1, 2, 3], 2)).toEqual([1, 3]);
		expect(toggled([1, 2], 0)).toEqual([1, 2]);
		expect(toggled([1, 2], -1)).toEqual([1, 2]);
	});

	it('removes one entry by position', () => {
		expect(withoutAt([4, 5, 4], 2)).toEqual([4, 5]);
		expect(withoutAt([4, 5], 9)).toEqual([4, 5]);
	});

	it('adds equipped weapons and armour to their lists and ignores the rest', () => {
		// cmdEquipButtons case 3 calls CompareAddItem for each equipped slot with an item
		const kinds = new Map([
			[10, 'armour' as const],
			[20, 'weapon' as const],
			[30, 'other' as const],
			[40, 'armour' as const]
		]);
		const start = { ...emptyLists(), armour: [40] };
		const gear = [10, 0, 20, 30, 999, 40];
		const once = addEquipped(start, gear, kinds);
		// 40 was on the list, so it comes off
		expect(once).toEqual({ ...emptyLists(), weapons: [20], armour: [10] });
		const twice = addEquipped(once, gear, kinds);
		expect(twice).toEqual({ ...emptyLists(), armour: [40] });
		expect(start.armour).toEqual([40]);
	});
});

describe('the lists in the character', () => {
	const other: ExtraEntry[] = [{ section: 'MonsterFilter', key: 'HP', value: '123' }];

	it('reads the [Compare] keys whatever their case', () => {
		const extra: ExtraEntry[] = [
			...other,
			{ section: 'compare', key: 'weaponcompare', value: '310,311,' },
			{ section: 'Compare', key: 'MonsterCompare', value: '7,' }
		];
		expect(readLists(extra)).toEqual({ ...emptyLists(), weapons: [310, 311], monsters: [7] });
		expect(readLists(undefined)).toEqual(emptyLists());
	});

	it('writes all four keys beside the entries it does not own', () => {
		const out = writeLists(other, { ...emptyLists(), spells: [5, 6] });
		expect(out).toEqual([
			...other,
			{ section: 'Compare', key: 'WeaponCompare', value: '' },
			{ section: 'Compare', key: 'ArmourCompare', value: '' },
			{ section: 'Compare', key: 'SpellCompare', value: '5,6,' },
			{ section: 'Compare', key: 'MonsterCompare', value: '' }
		]);
		expect(other).toHaveLength(1);
	});

	it('updates entries in place and leaves a character with no lists alone', () => {
		const extra: ExtraEntry[] = [
			{ section: 'Compare', key: 'WeaponCompare', value: '1,' },
			{ section: 'Compare', key: 'ArmourCompare', value: '2,' },
			{ section: 'Compare', key: 'SpellCompare', value: '' },
			{ section: 'Compare', key: 'MonsterCompare', value: '' },
			{ section: 'Other', key: 'K', value: 'v' }
		];
		const out = writeLists(extra, { ...emptyLists(), weapons: [9, 1] });
		expect(out.map((e) => e.value)).toEqual(['9,1,', '', '', '', 'v']);
		expect(writeLists(other, emptyLists())).toEqual(other);
		expect(writeLists(undefined, emptyLists())).toEqual([]);
	});

	it('round-trips every list', () => {
		const lists = { weapons: [3, 1, 2], armour: [8], spells: [], monsters: [44, 45] };
		expect(readLists(writeLists([], lists))).toEqual(lists);
		expect(COMPARE_KINDS).toHaveLength(4);
	});
});

describe('classify client', () => {
	it('posts the item numbers and skips the call for none', async () => {
		const f = vi.fn(
			async (_u: string, init?: RequestInit) =>
				new Response(JSON.stringify({ items: [{ number: 7, kind: 'weapon', worn: false }] }), { status: 200 })
		);
		const c = createCompareClient(f as unknown as typeof fetch);
		expect(await c.classify([])).toEqual([]);
		expect(f).not.toHaveBeenCalled();
		expect(await c.classify([7])).toEqual([{ number: 7, kind: 'weapon', worn: false }]);
		expect(f.mock.calls[0][0]).toBe('/api/compare/classify');
		expect(JSON.parse(String(f.mock.calls[0][1]?.body))).toEqual({ items: [7] });
	});

	it('reports a refusal', async () => {
		const f = vi.fn(async () => new Response('items has more than 500 entries', { status: 400 }));
		const c = createCompareClient(f as unknown as typeof fetch);
		await expect(c.classify([1])).rejects.toThrow('more than 500');
	});
});

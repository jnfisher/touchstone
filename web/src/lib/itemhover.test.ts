import { describe, expect, it } from 'vitest';
import type { Item } from './api';
import type { Pane } from './detail';
import { hoveredItem, itemFacts, paneLines, placeCard } from './itemhover';

const item = (o: Partial<Item>): Item =>
	({
		number: 1,
		name: 'thing',
		limit: 0,
		encum: 100,
		item_type: 0,
		use_count: 0,
		min: 0,
		max: 0,
		armour_class: 0,
		damage_resist: 0,
		weapon_type: 0,
		armour_type: 0,
		worn: 0,
		accy: 0,
		str_req: 0,
		speed: 0,
		abilities: [],
		...o
	}) as Item;

describe('facts', () => {
	it('shows a weapon by its damage, speed and what it asks', () => {
		expect(itemFacts(item({ item_type: 1, weapon_type: 3, min: 12, max: 30, speed: 2400, accy: 5, str_req: 60, encum: 250, limit: 1 }))).toEqual([
			{ label: 'Weapon', value: '2H Sharp' },
			{ label: 'Base damage', value: '12 to 30' },
			{ label: 'Speed', value: '2400' },
			{ label: 'Accuracy', value: '5' },
			{ label: 'Strength', value: '60' },
			{ label: 'Weight', value: '250' },
			{ label: 'Limit', value: '1' }
		]);
	});

	it('shows armour by where it is worn and its AC and DR in whole points', () => {
		expect(itemFacts(item({ worn: 11, armour_type: 7, armour_class: 190, damage_resist: 25, encum: 400 }))).toEqual([
			{ label: 'Worn', value: 'Torso' },
			{ label: 'Armour', value: 'Chainmail' },
			{ label: 'AC / DR', value: '19 / 2.5' },
			{ label: 'Weight', value: '400' }
		]);
	});

	it('shows anything else by its type', () => {
		expect(itemFacts(item({ item_type: 7, encum: 1 }))).toEqual([
			{ label: 'Type', value: 'Key' },
			{ label: 'Weight', value: '1' }
		]);
		expect(itemFacts(item({ item_type: 9, use_count: 3 })).map((l) => l.label)).toEqual(['Type', 'Uses', 'Weight']);
	});
});

describe('pane', () => {
	it('lists the labelled rows, then the comparisons', () => {
		const pane = {
			rows: [
				{ label: 'Damage', value: '184/round', kind: 'weapon_damage' },
				{ label: '', value: 'a loose line', kind: 'message' },
				{ label: 'Abilities', value: 'Crits +3', kind: 'abilities' }
			],
			compare: [{ label: 'Compared to katana', text: 'Damage: +4' }]
		} as Pane;
		expect(paneLines(pane)).toEqual([
			{ label: 'Damage', value: '184/round' },
			{ label: 'Abilities', value: 'Crits +3' },
			{ label: 'Compared to katana', value: 'Damage: +4' }
		]);
	});
});

describe('placing the card', () => {
	const card = { width: 340, height: 200 };
	const view = { width: 1000, height: 700 };

	it('goes under the name, left edges aligned', () => {
		expect(placeCard({ left: 100, top: 50, width: 80, height: 20 }, card, view)).toEqual({ left: 100, top: 76 });
	});

	it('goes above when there is no room below, and stays inside the window', () => {
		expect(placeCard({ left: 900, top: 600, width: 80, height: 20 }, card, view)).toEqual({ left: 652, top: 394 });
		expect(placeCard({ left: -20, top: 100, width: 80, height: 20 }, card, { width: 1000, height: 250 })).toEqual({ left: 8, top: 8 });
	});
});

describe('placing the card beside an open list', () => {
	const card = { width: 340, height: 200 };
	const view = { width: 1000, height: 700 };
	const row = { left: 100, top: 300, width: 300, height: 24 };

	it('goes to the right of the list, level with the row', () => {
		expect(placeCard(row, card, view, { left: 100, top: 200, width: 300, height: 260 })).toEqual({ left: 406, top: 300 });
	});

	it('goes to the left when the right has no room, and stays inside the window', () => {
		const list = { left: 600, top: 200, width: 300, height: 260 };
		expect(placeCard({ ...row, left: 600 }, card, view, list)).toEqual({ left: 254, top: 300 });
		expect(placeCard({ ...row, left: 600, top: 650 }, card, view, list)).toEqual({ left: 254, top: 492 });
	});
});

describe('what the pointer is over', () => {
	// the two things the lookup asks of an element, without a DOM
	type Fake = { attrs: Record<string, string>; up: Record<string, Fake | undefined> };
	const el = (attrs: Record<string, string> = {}, up: Record<string, Fake | undefined> = {}): Fake => ({ attrs, up });
	const dom = (f: Fake): Element =>
		({
			closest: (sel: string) => (f.up[sel] ? dom(f.up[sel]) : null),
			getAttribute: (name: string) => f.attrs[name] ?? null,
			fake: f
		}) as unknown as Element;
	const NAME_CELL = '[data-item-rows] .ag-cell[col-id="name"]';

	it('finds a marked name, from inside it too', () => {
		const button = el({ 'data-item': '212' });
		const hit = hoveredItem(dom(el({}, { '[data-item]': button })));
		expect(hit?.item).toBe(212);
		expect((hit?.anchor as unknown as { fake: Fake }).fake).toBe(button);
		expect(hoveredItem(dom(el()))).toBeUndefined();
		expect(hoveredItem(null)).toBeUndefined();
	});

	it('finds the row of a grid of items, on the name cell only', () => {
		const row = el({ 'row-id': '822' });
		const cell = el({}, { '.ag-row': row });
		expect(hoveredItem(dom(el({}, { [NAME_CELL]: cell })))?.item).toBe(822);
		// another column of the same row, or a name cell outside a grid of items
		expect(hoveredItem(dom(el({}, { '.ag-row': row })))).toBeUndefined();
	});

	it('ignores a mark that is not an item number', () => {
		for (const mark of ['', '0', 'abc', '1.5']) {
			expect(hoveredItem(dom(el({}, { '[data-item]': el({ 'data-item': mark }) })))).toBeUndefined();
		}
	});
});

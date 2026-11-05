import { describe, expect, it } from 'vitest';
import type { Item, Monster, Spell } from './api';
import { abilityValue, EXPLORERS, explorerFor, parseNmr, putCommas, spellInGame } from './explorers';
import { SECTIONS } from './sections';
import * as E from './enums';

const slots = (...p: [number, number][]) => [
	...p.map(([ability, value]) => ({ ability, value })),
	...Array.from({ length: 20 - p.length }, () => ({ ability: 0, value: 0 }))
];
const item = (o: Partial<Item>): Item =>
	({ number: 1, name: 'x', in_game: true, item_type: 1, worn: 0, abilities: slots(), accy: 0, class_rest: [], race_rest: [], negate_spell: [], ...o }) as Item;
const lk = { nmr: 1.83 };

describe('explorer definitions', () => {
	it('covers every data tab, each with unique column ids', () => {
		for (const s of SECTIONS.filter((s) => s.slug !== 'character' && s.slug !== 'compare')) {
			const ex = explorerFor(s.slug);
			expect(ex, s.slug).toBeDefined();
			const ids = ex!.columns.map((c) => c.id);
			expect(new Set(ids).size).toBe(ids.length);
			if (ex!.defaultSort) expect(ids).toContain(ex!.defaultSort.col);
		}
		expect(Object.keys(EXPLORERS)).toHaveLength(9);
	});

	// MME headers and order: modMain AddWeapon2LV, AddArmour2LV, AddMonster2LV.
	it('uses MME headers in MME order', () => {
		const h = (slug: string) => explorerFor(slug)!.columns.map((c) => c.header);
		expect(h('weapons').slice(0, 5)).toEqual(['#', 'Name', 'Wepn Type', 'Min Dmg', 'Max Dmg']);
		expect(h('armour')).toEqual(['#', 'Name', 'Worn', 'Armr Type', 'Level', 'Enc', 'AC', 'Acc', 'Crits', 'Limit']);
		expect(h('monsters').slice(0, 4)).toEqual(['#', 'Name', 'RGN', 'Exp']);
		expect(explorerFor('weapons')!.pending).toContain('Dmg/Rnd');
	});

	// FilterWeapons, FilterArmour, FilterSundry: item-type split with "only in game" on.
	it('splits items into weapons, armour and sundry', () => {
		const w = item({ item_type: 1 });
		const a = item({ item_type: 0, worn: 2 });
		const ring = item({ item_type: 0, worn: 0 });
		const scroll = item({ item_type: 9 });
		const out = item({ item_type: 1, in_game: false });
		const inc = (slug: string, r: Item) => explorerFor(slug)!.include(r, lk);
		expect([w, a, ring, scroll].map((r) => inc('weapons', r))).toEqual([true, false, false, false]);
		expect([w, a, ring, scroll].map((r) => inc('armour', r))).toEqual([false, true, true, false]);
		expect([w, a, ring, scroll].map((r) => inc('items', r))).toEqual([false, false, true, true]);
		expect(inc('weapons', out)).toBe(false);
	});

	it('derives weapon columns from abilities like AddWeapon2LV', () => {
		const w = item({
			armour_class: 15,
			damage_resist: 5,
			accy: 3,
			abilities: slots([135, 12], [22, 4], [58, 7], [116, -10])
		});
		const col = (id: string) => explorerFor('weapons')!.columns.find((c) => c.id === id)!;
		expect(col('level').get(w)).toBe(12);
		expect(col('acc').get(w)).toBe(7);
		expect(col('crits').get(w)).toBe(7);
		expect(col('bsacc').get(w)).toBe(-10);
		expect(col('ac').get(w)).toBe('2/0.5');
		expect(col('bsacc').get(item({}))).toBe('No');
	});

	it('formats monster exp with the multiplier and thousands separators', () => {
		const m = { exp: 1500, exp_multi: 2, abilities: slots([34, 20]) } as Monster;
		const exp = explorerFor('monsters')!.columns.find((c) => c.id === 'exp')!;
		expect(exp.get(m)).toBe(3000);
		expect(exp.format!(3000, m)).toBe('3,000');
		expect(exp.format!(0, m)).toBe('0');
		expect(explorerFor('monsters')!.columns.find((c) => c.id === 'dodge')!.get(m)).toBe(20);
	});

	// SpellIsInGame
	it('keeps only spells that exist in game', () => {
		const base = { learnable: false, learned_from: null, casted_by: null, classes: null, magery: 1, req_level: 1 } as Spell;
		expect(spellInGame(base, 1.83)).toBe(false);
		expect(spellInGame({ ...base, learnable: true }, 1.83)).toBe(true);
		expect(spellInGame({ ...base, classes: '(1)(2)' }, 1.83)).toBe(true);
		expect(spellInGame({ ...base, classes: '(1)(2)' }, 1.7)).toBe(false);
		expect(spellInGame({ ...base, magery: 5 }, 1.83)).toBe(true);
	});

	it('builds a detail pane with a titled first group', () => {
		const g = explorerFor('weapons')!.detail(item({ name: 'Sword', abilities: slots([43, 9]) }), lk);
		expect(g[0].title).toBe('Sword');
		expect(g.find((x) => x.title === 'Abilities')!.rows[0]).toEqual({ label: 'Ability 43', value: '9', long: false });
	});
});

describe('helpers', () => {
	it('parses NMR versions', () => {
		expect(parseNmr('v1.8.3')).toBe(1.83);
		expect(parseNmr('1.8')).toBe(1.8);
		expect(parseNmr('?')).toBe(0);
	});
	it('groups digits', () => {
		expect(putCommas(1234567)).toBe('1,234,567');
		expect(putCommas(1234.5)).toBe('1,234.5');
		expect(putCommas(12)).toBe('12');
	});
	it('takes the last matching ability slot', () => {
		expect(abilityValue(slots([22, 1], [105, 2]), 22, 105)).toBe(2);
		expect(abilityValue(slots(), 22)).toBeUndefined();
	});
	// modMMudFunc Get*Enum fallbacks
	it('names codes like MME', () => {
		expect(E.armourType(5)).toBe('Leather');
		expect(E.armourType(42)).toBe('Unknown (42)');
		expect(E.itemType(77)).toBe('77');
		expect(E.mageryName(0, 3)).toBe('None');
		expect(E.mageryName(2, 3)).toBe('Priest-3');
	});
});

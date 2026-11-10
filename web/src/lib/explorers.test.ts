import { describe, expect, it } from 'vitest';
import type { Item, Monster, Spell } from './api';
import { EXPLORERS, explorerFor, parseNmr, spellInGame } from './explorers';
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
		for (const s of SECTIONS.filter((s) => s.slug !== 'character' && s.slug !== 'compare' && s.slug !== 'rooms' && s.slug !== 'chests')) {
			const ex = explorerFor(s.slug);
			expect(ex, s.slug).toBeDefined();
			const ids = ex!.columns.map((c) => c.id);
			expect(new Set(ids).size).toBe(ids.length);
			// the server's columns carry the default sort of a computed tab
			if (ex!.defaultSort && !ex!.computed) expect(ids).toContain(ex!.defaultSort.col);
		}
		expect(Object.keys(EXPLORERS)).toHaveLength(8);
	});

	// Every data explorer is built by the server (POST /api/evaluate/{tab}); Rooms has its own tab.
	it('lists the eight computed tabs from the server', () => {
		const computed = Object.values(EXPLORERS)
			.filter((e) => e!.computed)
			.map((e) => e!.computed);
		expect(computed.sort()).toEqual(['armour', 'classes', 'items', 'monsters', 'races', 'shops', 'spells', 'weapons']);
		for (const slug of ['weapons', 'armour', 'items', 'monsters', 'spells', 'shops', 'races', 'classes'])
			expect(explorerFor(slug)!.pending).toEqual([]);
	});

	// FilterMonsters lists the in-game, named records (the server then filters by the boxes).
	it('lists in-game named monsters on the Monsters tab', () => {
		const ex = explorerFor('monsters')!;
		const m = (o: Partial<Monster>) => ({ number: 1, name: 'rat', in_game: true, ...o }) as Monster;
		expect(ex.include(m({}), lk)).toBe(true);
		expect(ex.include(m({ in_game: false }), lk)).toBe(false);
		expect(ex.include(m({ name: null }), lk)).toBe(false);
		expect(ex.include(m({ name: 'sdf rat' }), lk)).toBe(false);
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

	// SpellIsInGame
	it('keeps only spells that exist in game', () => {
		const base = { learnable: false, learned_from: null, casted_by: null, classes: null, magery: 1, req_level: 1 } as Spell;
		expect(spellInGame(base, 1.83)).toBe(false);
		expect(spellInGame({ ...base, learnable: true }, 1.83)).toBe(true);
		expect(spellInGame({ ...base, classes: '(1)(2)' }, 1.83)).toBe(true);
		expect(spellInGame({ ...base, classes: '(1)(2)' }, 1.7)).toBe(false);
		expect(spellInGame({ ...base, magery: 5 }, 1.83)).toBe(true);
	});

	it('leaves the item, spell, shop, class, race and monster panes to the server', () => {
		// Expected: the rows of these panes are MME's own text from /api/detail; only the name is local.
		const w = item({ name: 'Sword', abilities: slots([43, 9]) });
		for (const slug of ['weapons', 'armour', 'items'] as const) {
			const g = explorerFor(slug)!.detail(w, lk);
			expect(g).toEqual([{ title: 'Sword', rows: [] }]);
		}
		for (const slug of ['spells', 'shops', 'classes', 'races', 'monsters'] as const) {
			const g = explorerFor(slug)!.detail({ name: 'x', abilities: slots() } as never, lk);
			expect(g.map((x) => x.rows.length)).toEqual([0]);
		}
	});
});

describe('helpers', () => {
	it('parses NMR versions', () => {
		expect(parseNmr('v1.8.3')).toBe(1.83);
		expect(parseNmr('1.8')).toBe(1.8);
		expect(parseNmr('?')).toBe(0);
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

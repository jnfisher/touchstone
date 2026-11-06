import { describe, expect, it, vi } from 'vitest';
import { createMenuActions } from './menuactions.svelte';
import { calcPrefill, combatPrefill, vsMob } from './handoff.svelte';

function deps() {
	const toggles: [string, number][] = [];
	const removed: [string, number][] = [];
	const lists = { weapons: [5], armour: [], spells: [], monsters: [7] };
	const cmp = {
		lists,
		toggle: (k: string, n: number) => toggles.push([k, n]),
		removeAt: (k: string, i: number) => removed.push([k, i])
	};
	const doc = { bless: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], learned_spells: [3], equipped: [] as number[] };
	const chars = { doc, options: {}, edit: (f: (d: typeof doc) => void) => f(doc) };
	const go = vi.fn();
	return { toggles, removed, doc, go, d: { chars, cmp, go } as never };
}

describe('menu actions', () => {
	it('adds a record to its compare list, and removes the selected entry from a compare list', async () => {
		const x = deps();
		const a = createMenuActions(x.d);
		await a.run('saveList', { list: 'weapons', number: 9 });
		expect(x.toggles).toEqual([['weapons', 9]]);
		await a.run('saveList', { list: 'weaponCompare', number: 5 });
		expect(x.removed).toEqual([['weapons', 0]]);
		await a.run('monsterList', { list: 'monsters', number: 4 });
		expect(x.toggles.at(-1)).toEqual(['monsters', 4]);
	});

	it('opens the calculators and the combat setup with what MME passes them', async () => {
		const x = deps();
		const a = createMenuActions(x.d);
		await a.run('calcSwings', { list: 'weapons', number: 12 });
		expect(x.go).toHaveBeenLastCalledWith('/calc/swing');
		expect(calcPrefill.take('swing')).toEqual({ weapon: 12 });
		await a.run('calcBackstab', { list: 'weapons', number: 13 });
		expect(calcPrefill.take('hit')).toBeUndefined();
		expect(calcPrefill.take('backstab')).toEqual({ weapon: 13 });
		await a.run('setBackstabWeapon', { list: 'weapons', number: 14 });
		expect(combatPrefill.take()).toEqual({ backstab: 14 });
		await a.run('setCombatSpell', { list: 'spells', number: 15 });
		expect(x.go).toHaveBeenLastCalledWith('/character');
		expect(combatPrefill.take()).toEqual({ spell: 15 });
	});

	it('marks a spell learned or not, and takes the first open bless slot once', async () => {
		const x = deps();
		const a = createMenuActions(x.d);
		await a.run('learn', { list: 'spells', number: 3 });
		expect(x.doc.learned_spells).toEqual([]);
		await a.run('learn', { list: 'spells', number: 3 });
		expect(x.doc.learned_spells).toEqual([3]);
		await a.run('setBless', { list: 'spells', number: 40 });
		expect(x.doc.bless[0]).toBe(40);
		expect(await a.run('setBless', { list: 'spells', number: 40 })).toBe('Already a bless spell');
	});

	it('sets the weapon calc from the monster row (AC/DR, dodge) and the spell calc from its MR', async () => {
		const x = deps();
		const a = createMenuActions(x.d);
		const cells = ['8', 'cave worm', '', '', '', '14/3', '9', '43'];
		await a.run('setWeaponCalc', { list: 'monsters', number: 8, cells });
		expect(vsMob.weapons).toMatchObject({ ac: '14', dr: '3', dodge: '9', name: 'cave worm' });
		expect(x.go).toHaveBeenLastCalledWith('/weapons');
		await a.run('setSpellCalc', { list: 'monsters', number: 8, cells });
		expect(vsMob.spells?.mr).toBe('43');
	});

	it('uses the lair averages of the pane for a lair-only group', async () => {
		const x = deps();
		const a = createMenuActions(x.d);
		await a.run('setWeaponCalc', {
			list: 'monsters',
			number: 8,
			cells: ['8', 'orc lair', '', '', '', '1/1', '1', '1'],
			lairStats: true,
			lines: [
				{ text: 'AVG AC/DR', subs: ['20/5'] },
				{ text: 'AVG Dodge', subs: ['12'] }
			]
		});
		expect(vsMob.weapons).toMatchObject({ ac: '20', dr: '5', dodge: '12' });
	});

	it('goes to the Monsters tab for the summoning lookup', async () => {
		const x = deps();
		await createMenuActions(x.d).run('lookUpMonster', { list: 'monsters', number: 21 });
		expect(x.go).toHaveBeenLastCalledWith('/monsters');
	});
});

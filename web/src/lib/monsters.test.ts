import { describe, expect, it } from 'vitest';
import {
	DEFAULT_CONTROLS,
	freshControls,
	isFiltering,
	lineColorClass,
	MONSTER_FILTER_KEY,
	readControls,
	sanitize,
	writeControls
} from './monsters';
import { createMonsterFilterStore } from './monsters.svelte';

function memory(initial?: string) {
	const data = new Map<string, string>(initial === undefined ? [] : [[MONSTER_FILTER_KEY, initial]]);
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v),
		data
	};
}

describe('monster filter controls', () => {
	// RemoveFilters writes these into the boxes (frmMain); the extras start off.
	it('start where RemoveFilters leaves MME', () => {
		const c = freshControls();
		expect([c.regen, c.hp, c.exp, c.damage, c.magic_value]).toEqual(['999', '99999', '0', '99999', '999']);
		expect([c.lair, c.magic, c.regen_op, c.extras.enabled]).toEqual([false, false, 0, false]);
		expect(c.extras.armour_class).toBe(9999);
		expect(c.party.size).toBe('1');
		expect(isFiltering(c)).toBe(false);
	});

	it('count as filtering once a box, the mode or the extras change', () => {
		const c = freshControls();
		expect(isFiltering({ ...c, hp: '200' })).toBe(true);
		expect(isFiltering({ ...c, exp: '5K' })).toBe(true);
		expect(isFiltering({ ...c, lair: true })).toBe(true);
		expect(isFiltering({ ...c, magic: true })).toBe(true);
		expect(isFiltering({ ...c, extras: { ...c.extras, enabled: true } })).toBe(true);
		// an empty box means its default
		expect(isFiltering({ ...c, hp: ' ' })).toBe(false);
	});

	it('repair damaged storage field by field', () => {
		const bad = sanitize({
			lair: 'yes',
			regen_op: 7,
			hp: 250,
			exp: { no: 1 },
			party: { size: '3', ac: null },
			extras: { enabled: true, armour_class: 'many', num_mobs_gte: 1.5, abilities: [[28, 1, 2], 'x'] }
		});
		expect(bad.lair).toBe(false);
		expect(bad.regen_op).toBe(0);
		expect(bad.hp).toBe('250');
		expect(bad.exp).toBe(DEFAULT_CONTROLS.exp);
		expect([bad.party.size, bad.party.ac]).toEqual(['3', '60']);
		expect(bad.extras.enabled).toBe(true);
		expect(bad.extras.armour_class).toBe(9999);
		expect(bad.extras.num_mobs_gte).toBe(1.5);
		expect(bad.extras.abilities[0]).toEqual([28, 1, 2]);
		expect(bad.extras.abilities[1]).toEqual([0, 1, 1]);
	});

	it('survive a reload and a blocked or garbled storage', () => {
		const s = memory();
		const c = { ...freshControls(), hp: '300', lair: true };
		writeControls(s, c);
		expect(readControls(s)).toEqual(c);
		expect(readControls(memory('{not json'))).toEqual(freshControls());
		expect(readControls(undefined)).toEqual(freshControls());
		const blocked = {
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			}
		};
		expect(readControls(blocked)).toEqual(freshControls());
		expect(() => writeControls(blocked, c)).not.toThrow();
	});
});

describe('monster filter store', () => {
	it('keeps every change and Reset puts the boxes back but keeps the mode, party and extra values', () => {
		const s = memory();
		const store = createMonsterFilterStore(s);
		store.set({ lair: true, hp: '120' });
		store.setParty({ size: '4' });
		store.setExtras({ enabled: true, armour_class: 30 });
		expect(readControls(s).party.size).toBe('4');
		store.reset();
		expect(store.controls.hp).toBe('99999');
		expect(store.controls.lair).toBe(true);
		expect(store.controls.party.size).toBe('4');
		expect(store.controls.extras.enabled).toBe(false);
		expect(store.controls.extras.armour_class).toBe(30);
	});
});

describe('pane colours', () => {
	// The VB ForeColor values of AddMonsterDamageOutText and PullMonsterDetail, as theme classes.
	it('map the colours MME uses to theme tokens', () => {
		expect(lineColorClass(204)).toBe('text-destructive');
		expect(lineColorClass(255)).toBe('text-destructive');
		expect(lineColorClass(32768)).toBe('text-positive');
		expect(lineColorClass(13395456)).toBe('text-info');
		expect(lineColorClass(8388736)).toBe('text-violet');
		expect(lineColorClass(14025872)).toBe('text-violet');
		expect(lineColorClass(-2147483627)).toBe('text-muted-foreground');
		expect(lineColorClass(undefined)).toBe('');
		expect(lineColorClass(123)).toBe('');
	});
});

import { describe, expect, it } from 'vitest';
import { createNavStore } from './navhistory.svelte';

function setup() {
	const sel: Record<string, string | number | undefined> = {};
	const went: string[] = [];
	const nav = createNavStore({
		go: (p) => went.push(p),
		selected: (s) => sel[s],
		select: (s, id) => (sel[s] = id)
	});
	return { sel, went, nav };
}

describe('navigation store', () => {
	it('back restores the tab and the record that was selected on it', () => {
		const { sel, went, nav } = setup();
		nav.shown('/monsters');
		sel.monsters = 31;
		nav.shown('/spells');
		sel.spells = 5;
		expect(nav.canBack).toBe(true);
		expect(nav.back()).toBe(true);
		expect(went).toEqual(['/monsters']);
		expect(sel.monsters).toBe(31);
		// arriving by our own move is not a new visit
		nav.shown('/monsters');
		expect(nav.canForward).toBe(true);
		sel.monsters = 40;
		expect(nav.forward()).toBe(true);
		expect(went).toEqual(['/monsters', '/spells']);
		expect(sel.spells).toBe(5);
		nav.shown('/spells');
		nav.back();
		// the record changed after leaving is what comes back
		expect(sel.monsters).toBe(40);
	});

	it('does nothing with nowhere to go', () => {
		const { went, nav } = setup();
		nav.shown('/items');
		expect(nav.back()).toBe(false);
		expect(nav.forward()).toBe(false);
		expect(went).toEqual([]);
	});
});

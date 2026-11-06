import { describe, expect, it } from 'vitest';
import { createNavHistory, NAV_HISTORY_MAX, slugOfPath } from './navhistory';

describe('navigation history', () => {
	it('goes back and forward over the tabs visited, with the record selected on each', () => {
		const h = createNavHistory();
		h.visit('/weapons');
		h.visit('/monsters', 8); // weapons was left with record 8
		h.visit('/spells', 12); // monsters was left with record 12
		expect(h.back(3)?.path).toBe('/monsters'); // spells left with 3
		expect(h.current?.id).toBe(12);
		expect(h.back(13)?.path).toBe('/weapons');
		expect(h.current?.id).toBe(8);
		expect(h.back()).toBeUndefined();
		expect(h.forward(9)?.path).toBe('/monsters');
		expect(h.current?.id).toBe(13);
		expect(h.forward()?.path).toBe('/spells');
		expect(h.current?.id).toBe(3);
		expect(h.forward()).toBeUndefined();
	});

	it('does not count a visit to the page already shown (NavHistory_RecordNavigation)', () => {
		const h = createNavHistory();
		h.visit('/items');
		h.visit('/items', 4);
		expect(h.entries).toHaveLength(1);
		expect(h.canBack).toBe(false);
	});

	it('drops the forward part when the user goes somewhere new', () => {
		const h = createNavHistory();
		h.visit('/a');
		h.visit('/b');
		h.back();
		h.visit('/c');
		expect(h.entries.map((e) => e.path)).toEqual(['/a', '/c']);
		expect(h.canForward).toBe(false);
	});

	it('keeps the newest NAV_HISTORY_MAX places (NAV_HISTORY_MAX = 50 in MME)', () => {
		const h = createNavHistory();
		for (let i = 0; i < NAV_HISTORY_MAX + 5; i++) h.visit(`/p${i}`);
		expect(h.entries).toHaveLength(NAV_HISTORY_MAX);
		expect(h.entries[0].path).toBe('/p5');
		expect(h.current?.path).toBe(`/p${NAV_HISTORY_MAX + 4}`);
	});

	it('reads a section out of a path', () => {
		expect(slugOfPath('/monsters')).toBe('monsters');
		expect(slugOfPath('/calc/swing')).toBeUndefined();
	});
});

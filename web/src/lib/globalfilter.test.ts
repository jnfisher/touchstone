import { describe, expect, it } from 'vitest';
import { newCharacter } from './character';
import {
	DEFAULT_FILTER,
	GLOBAL_FILTER_KEY,
	isFiltering,
	readFilter,
	sanitize,
	wireFilter,
	writeFilter
} from './globalfilter';
import { createGlobalFilterStore } from './globalfilter.svelte';

const memory = (init: Record<string, string> = {}) => {
	const data = { ...init };
	return {
		data,
		getItem: (k: string) => data[k] ?? null,
		setItem: (k: string, v: string) => void (data[k] = v)
	};
};

describe('stored filter', () => {
	it('starts at "filter nothing, mirror the character" and survives damaged storage', () => {
		expect(readFilter(undefined)).toEqual(DEFAULT_FILTER);
		expect(readFilter(memory())).toEqual(DEFAULT_FILTER);
		expect(readFilter(memory({ [GLOBAL_FILTER_KEY]: '{oops' }))).toEqual(DEFAULT_FILTER);
		expect(readFilter(memory({ [GLOBAL_FILTER_KEY]: '7' }))).toEqual(DEFAULT_FILTER);
		expect(DEFAULT_FILTER.level).toBe(999);
	});
	it('keeps good fields, repairs bad ones, clamps to the boxes', () => {
		expect(sanitize({ class: 3, level: 20, alignment: 9, race: 'x', min_item_level: -4, use_character: 'y' })).toEqual({
			...DEFAULT_FILTER,
			class: 3,
			level: 20,
			alignment: 3,
			race: 0,
			min_item_level: 0
		});
		expect(sanitize({ level: 5000 }).level).toBe(999);
		expect(sanitize({ level: 12.9 }).level).toBe(12);
	});
	it('round-trips, and a store persists each change', () => {
		const m = memory();
		const f = { ...DEFAULT_FILTER, use_character: false, class: 2 };
		writeFilter(m, f);
		expect(readFilter(m)).toEqual(f);
		const store = createGlobalFilterStore(m);
		expect(store.filter.class).toBe(2);
		store.set({ level: 30 });
		expect(JSON.parse(m.data[GLOBAL_FILTER_KEY]).level).toBe(30);
		store.reset();
		expect(store.filter).toEqual(DEFAULT_FILTER);
	});
	it('survives storage that throws', () => {
		const boom = {
			getItem() {
				throw new Error('blocked');
			},
			setItem() {
				throw new Error('blocked');
			}
		};
		expect(readFilter(boom)).toEqual(DEFAULT_FILTER);
		const store = createGlobalFilterStore(boom);
		store.set({ alignment: 1 });
		expect(store.filter.alignment).toBe(1);
	});
});

describe('request mapping', () => {
	const picked = { ...newCharacter('Bo'), class: 1, race: 2 };
	it('mirrors only a complete character', () => {
		expect(wireFilter(DEFAULT_FILTER, picked).use_character).toBe(true);
		expect(wireFilter(DEFAULT_FILTER, newCharacter('x')).use_character).toBe(false);
		expect(wireFilter(DEFAULT_FILTER, undefined).use_character).toBe(false);
		expect(wireFilter({ ...DEFAULT_FILTER, use_character: false }, picked).use_character).toBe(false);
	});
	it('says whether the bar changes the lists', () => {
		const off = { ...DEFAULT_FILTER, use_character: false };
		expect(isFiltering(off, picked)).toBe(false);
		expect(isFiltering({ ...off, level: 40 }, picked)).toBe(true);
		expect(isFiltering({ ...off, class: 2 }, picked)).toBe(true);
		expect(isFiltering({ ...off, alignment: 3 }, picked)).toBe(true);
		expect(isFiltering(DEFAULT_FILTER, picked)).toBe(true);
		expect(isFiltering(DEFAULT_FILTER, undefined)).toBe(false);
	});
});

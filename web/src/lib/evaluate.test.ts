import { describe, expect, it, vi } from 'vitest';
import { newCharacter } from './character';
import { DEFAULT_FILTER } from './globalfilter';
import { colorClass, createEvaluateClient, evaluateBody, EvaluateError, isEvalTab, toColDefs, type EvalRow } from './evaluate';

const columns = [
	{ id: 'num', header: '#', numeric: true },
	{ id: 'name', header: 'Name', numeric: false },
	{ id: 'dmg', header: 'Dmg/1st', numeric: true }
];
const row: EvalRow = {
	number: 7,
	cells: [
		{ text: '7', sort: 7 },
		{ text: 'Sword', sort: 'Sword', color: 0x8000 },
		{ text: '12', sort: 12.5 }
	]
};

describe('request', () => {
	const picked = { ...newCharacter('Bo'), class: 1, race: 2 };
	const mirror = { ...DEFAULT_FILTER, use_character: true, min_item_level: 4 };
	const own = { ...DEFAULT_FILTER, use_character: false, class: 3, level: 20, alignment: 2 };
	it('sends the character only while the bar mirrors it and it is complete', () => {
		expect(evaluateBody(picked, mirror, { attack: 4 })).toEqual({
			character: picked,
			global_filter: mirror,
			options: { attack: 4 }
		});
		// the bar's own values: no character
		expect(evaluateBody(picked, own)).toEqual({ global_filter: own, options: {} });
		// mirroring an unfinished or missing character falls back to the bar's own values
		expect(evaluateBody(newCharacter('x'), mirror)).toEqual({
			global_filter: { ...mirror, use_character: false },
			options: {}
		});
		expect(evaluateBody(undefined, mirror).character).toBeUndefined();
		expect(evaluateBody(undefined, mirror).global_filter.use_character).toBe(false);
	});
	it('knows the tabs it serves', () => {
		expect(isEvalTab('weapons')).toBe(true);
		expect(isEvalTab('monsters')).toBe(false);
	});
});

describe('column mapping', () => {
	const defs = toColDefs(columns, { col: 'name', dir: 'asc' });
	const get = (i: number, field: 'valueGetter' | 'valueFormatter') =>
		(defs[i][field] as (p: { data: EvalRow }) => unknown)({ data: row });

	it('sorts by the sort key and shows the text', () => {
		expect(get(2, 'valueGetter')).toBe(12.5);
		expect(get(2, 'valueFormatter')).toBe('12');
		expect(get(1, 'valueGetter')).toBe('Sword');
		expect(defs[2].filter).toBe('agNumberColumnFilter');
		expect(defs[1].sort).toBe('asc');
	});
	it('sorts text without regard to case, as MME does', () => {
		const cmp = defs[1].comparator as (a: string, b: string) => number;
		expect(cmp('abe', 'Bob')).toBe(-1);
		expect(cmp('Bob', 'bob')).toBe(0);
	});
	it('maps MME cell colours to theme classes', () => {
		const cls = defs[1].cellClass as (p: { data: EvalRow }) => string | undefined;
		expect(cls({ data: row })).toBe('ts-cell-above');
		expect(colorClass(0x80)).toBe('ts-cell-below');
		expect(colorClass(undefined)).toBeUndefined();
		expect(colorClass(0x123456)).toBeUndefined();
	});
});

describe('client', () => {
	it('posts the body and returns the table', async () => {
		const fetchFn = vi.fn(async () => new Response(JSON.stringify({ columns, rows: [row] })));
		const t = await createEvaluateClient(fetchFn as unknown as typeof fetch).evaluate('weapons', { global_filter: DEFAULT_FILTER, options: {} });
		expect(t.rows).toHaveLength(1);
		const [url, init] = fetchFn.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/evaluate/weapons');
		expect(init.method).toBe('POST');
	});
	it('reports the server reason', async () => {
		const fetchFn = vi.fn(async () => new Response('pick a class and race', { status: 422 }));
		await expect(createEvaluateClient(fetchFn as unknown as typeof fetch).evaluate('spells', { global_filter: DEFAULT_FILTER, options: {} })).rejects.toMatchObject({
			status: 422,
			message: 'pick a class and race'
		});
		expect(new EvaluateError(400, 'x').name).toBe('EvaluateError');
	});
});

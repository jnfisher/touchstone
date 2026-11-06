import { describe, expect, it, vi } from 'vitest';
import { newCharacter } from './character';
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
	it('sends the character with the global filter on, only when it is complete and in use', () => {
		expect(evaluateBody(picked, true, { attack: 4 })).toEqual({
			character: { ...picked, global_filter: true },
			options: { attack: 4 }
		});
		expect(evaluateBody(picked, false)).toEqual({ options: {} });
		expect(evaluateBody(newCharacter('x'), true)).toEqual({ options: {} });
		expect(evaluateBody(undefined, true)).toEqual({ options: {} });
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
		const t = await createEvaluateClient(fetchFn as unknown as typeof fetch).evaluate('weapons', { options: {} });
		expect(t.rows).toHaveLength(1);
		const [url, init] = fetchFn.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/evaluate/weapons');
		expect(init.method).toBe('POST');
	});
	it('reports the server reason', async () => {
		const fetchFn = vi.fn(async () => new Response('pick a class and race', { status: 422 }));
		await expect(createEvaluateClient(fetchFn as unknown as typeof fetch).evaluate('spells', { options: {} })).rejects.toMatchObject({
			status: 422,
			message: 'pick a class and race'
		});
		expect(new EvaluateError(400, 'x').name).toBe('EvaluateError');
	});
});

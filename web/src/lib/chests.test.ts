import { describe, expect, it, vi } from 'vitest';
import type { ColDef } from 'ag-grid-community';
import type { RefRow } from './detail';
import {
	DEFAULT_CHARM,
	contentColumns,
	createChestsClient,
	dropperColumns,
	dropperRows,
	parseCharm,
	summaryColumns,
	type ChestContent,
	type ChestSummary
} from './chests';

const summary: ChestSummary = {
	item: 909,
	name: 'iron-banded chest',
	contents: 68,
	average_copper: 514721.7,
	average: '514,722 Copper (51.47 Platinum)',
	average_short: '51P',
	dropped_by: 4
};
const content = (o: Partial<ChestContent>): ChestContent => ({
	item: 409,
	name: 'holy vestements',
	percent: 62.5,
	percent_text: '62.5%',
	sellable: true,
	sell_copper: 40000,
	sell: '40,000 Copper (400 Gold)',
	sell_short: '400G',
	expected_copper: 24988,
	expected: '24,988 Copper (249.88 Gold)',
	expected_short: '250G',
	...o
});

// the grids call these with only the row, as AG Grid does for the parts a test does not need
const cell = <T>(col: ColDef<T>, key: 'valueGetter' | 'valueFormatter' | 'tooltipValueGetter', data: T, value?: unknown) =>
	(col[key] as (p: unknown) => unknown)({ data, value });
const byId = <T>(cols: ColDef<T>[], id: string) => cols.find((c) => c.colId === id)!;

describe('charm box', () => {
	it('starts at 50 and takes whole numbers up to the game limit', () => {
		expect(DEFAULT_CHARM).toBe(50);
		expect(parseCharm('0')).toBe(0);
		expect(parseCharm(' 75 ')).toBe(75);
		expect(parseCharm('32767')).toBe(32767);
	});
	it('refuses anything else, so the old charm stays in use', () => {
		for (const t of ['', 'abc', '-1', '1.5', '32768', '123456'])
			expect(parseCharm(t), t).toBeUndefined();
	});
});

describe('client', () => {
	it('asks for the list and a chest at a charm', async () => {
		const f = vi.fn(async (_url: string) => new Response(JSON.stringify({ charm: 50, chests: [] })));
		const c = createChestsClient(f as unknown as typeof fetch);
		await c.list(50);
		await c.detail(909, 12);
		expect(f.mock.calls[0][0]).toBe('/api/chests?charm=50');
		expect(f.mock.calls[1][0]).toBe('/api/chests/909?charm=12');
	});
	it('reports a failure with the server text', async () => {
		const f = vi.fn(async (_url: string) => new Response('item 1 is not a chest', { status: 404 }));
		const c = createChestsClient(f as unknown as typeof fetch);
		await expect(c.detail(1, 50)).rejects.toMatchObject({ status: 404, message: 'item 1 is not a chest' });
	});
});

describe('chest list grid', () => {
	const cols = summaryColumns();
	it('has name, number, contents, average sale per opening and dropped by', () => {
		expect(cols.map((c) => c.headerName)).toEqual(['Name', 'Number', 'Contents', 'Avg sale / opening', 'Dropped by']);
		expect(new Set(cols.map((c) => c.colId)).size).toBe(cols.length);
	});
	it('sorts the average by its copper value but shows the short coin form', () => {
		const avg = byId(cols, 'average');
		expect(cell(avg, 'valueGetter', summary)).toBe(514721.7);
		expect(cell(avg, 'valueFormatter', summary)).toBe('51P');
		expect(cell(avg, 'tooltipValueGetter', summary)).toBe('514,722 Copper (51.47 Platinum)');
		expect(avg.filter).toBe('agNumberColumnFilter');
	});
});

describe('contents grid', () => {
	const cols = contentColumns();
	it('lists item, chance, sell price and expected value, best chance first', () => {
		expect(cols.map((c) => c.headerName)).toEqual(['Item', 'Number', 'Chance', 'Sells for', 'Expected']);
		expect(byId(cols, 'chance').sort).toBe('desc');
	});
	it('shows what an item sells for, or that it cannot be sold', () => {
		const sell = byId(cols, 'sell');
		expect(cell(sell, 'valueFormatter', content({}))).toBe('400G');
		const none = content({ sellable: false, sell_copper: 0, sell: "can't be sold", sell_short: '' });
		expect(cell(sell, 'valueFormatter', none)).toBe("can't be sold");
		expect((sell.cellClass as (p: unknown) => unknown)({ data: none })).toBe('ts-cell-nomatch');
		expect((sell.cellClass as (p: unknown) => unknown)({ data: content({}) })).toBeUndefined();
	});
	it('shows the expected value as chance times price', () => {
		const ex = byId(cols, 'expected');
		expect(cell(ex, 'valueGetter', content({}))).toBe(24988);
		expect(cell(ex, 'valueFormatter', content({}))).toBe('250G');
		expect(cell(byId(cols, 'chance'), 'valueFormatter', content({}))).toBe('62.5%');
	});
});

describe('dropped by grid', () => {
	const mon = (text: string, name: string, n: number): RefRow => ({
		kind: 'monster',
		text,
		tag: text.replace('%', ''),
		cells: [{ text: `Monster: ${name}(${n})`, tag: String(n) }],
		target: { table: 'monsters', number: n }
	});
	const refs: RefRow[] = [
		mon('100%', 'huge black dragon', 394),
		mon('15%', 'giant two-headed troll', 393),
		{
			kind: 'room',
			text: '',
			tag: '0',
			cells: [{ text: 'Room: Royal Treasure Vault (8/707)', tag: '8/707' }],
			target: { table: 'rooms', number: 0, key: '8/707' }
		},
		{ kind: 'no_limit', text: '', tag: 'nolimit', cells: [{ text: '... plus 3 more.', tag: '' }], target: null }
	];
	it('turns reference rows into monsters with a chance and other sources without one', () => {
		const rows = dropperRows(refs);
		expect(rows.map((r) => [r.kind, r.name, r.percent])).toEqual([
			['monster', 'huge black dragon(394)', 100],
			['monster', 'giant two-headed troll(393)', 15],
			['room', 'Royal Treasure Vault (8/707)', undefined]
		]);
		// the rows keep their targets, so a click opens the record
		expect(rows[0].target).toEqual({ table: 'monsters', number: 394 });
		expect(rows[2].target?.key).toBe('8/707');
	});
	it('labels the source and sorts the chance as a number', () => {
		const cols = dropperColumns();
		const rows = dropperRows(refs);
		expect(cols.map((c) => c.headerName)).toEqual(['Source', 'Name', 'Drop chance']);
		expect(cell(byId(cols, 'kind'), 'valueGetter', rows[2])).toBe('Room');
		expect(cell(byId(cols, 'chance'), 'valueGetter', rows[1])).toBe(15);
		expect(cell(byId(cols, 'chance'), 'valueFormatter', rows[1])).toBe('15%');
	});
});

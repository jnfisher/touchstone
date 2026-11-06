import { describe, expect, it } from 'vitest';
import { DEFAULT_FILTER } from './globalfilter';
import {
	createPaneClient,
	groupRows,
	itemSlug,
	linkFor,
	paneBody,
	paneKind,
	refCells,
	refColumns,
	type RefRow
} from './detail';

const ref = (o: Partial<RefRow>): RefRow => ({ kind: 'monster', text: '', tag: null, cells: [], target: null, ...o });

describe('detail panes', () => {
	it('maps tabs to panes', () => {
		expect(['weapons', 'armour', 'items'].map(paneKind)).toEqual(['items', 'items', 'items']);
		expect(paneKind('shops')).toBe('shops');
		expect(paneKind('monsters')).toBe('monsters');
		expect(paneKind('rooms')).toBeUndefined();
	});

	it('sends the character only while the bar mirrors it', () => {
		const doc = { class: 1, race: 1 } as never;
		expect(paneBody(doc, DEFAULT_FILTER).character).toBe(doc);
		expect(paneBody(doc, { ...DEFAULT_FILTER, use_character: false }).character).toBeUndefined();
		expect(paneBody(undefined, DEFAULT_FILTER).character).toBeUndefined();
		// a complete character the bar does not mirror still goes along for the monster pane
		const off = { ...DEFAULT_FILTER, use_character: false };
		expect(paneBody(doc, off, { keepCharacter: true }).character).toBe(doc);
		expect(paneBody({ class: null, race: 1 } as never, off, { keepCharacter: true }).character).toBeUndefined();
		expect(paneBody(doc, off, { options: { attack: 5 } }).options).toEqual({ attack: 5 });
		const b = paneBody(undefined, DEFAULT_FILTER, { charm: 40, showSell: true });
		expect([b.charm, b.show_sell, b.show_charm]).toEqual([40, true, undefined]);
	});

	it('groups rows by kind in order of appearance', () => {
		const g = groupRows([
			{ label: 'a', value: '1', kind: 'flags' },
			{ label: 'b', value: '2', kind: 'abilities' },
			{ label: 'c', value: '3', kind: 'flags' }
		]);
		expect(g.map((x) => [x.title, x.rows.length])).toEqual([
			['Flags', 2],
			['Abilities', 1]
		]);
	});

	it('picks the item tab as MME lists it', () => {
		expect(itemSlug({ item_type: 1, worn: 0 })).toBe('weapons');
		expect(itemSlug({ item_type: 0, worn: 5 })).toBe('armour');
		expect(itemSlug({ item_type: 0, worn: 0 })).toBe('items');
		expect(itemSlug({ item_type: 8, worn: 0 })).toBe('items');
	});

	it('resolves reference links', () => {
		const items = new Map([[7, { item_type: 1, worn: 0 }]]);
		expect(linkFor({ table: 'monsters', number: 25 })).toEqual({ slug: 'monsters', id: 25 });
		expect(linkFor({ table: 'rooms', number: 0, key: '1/405' })).toEqual({ slug: 'rooms', id: '1/405' });
		expect(linkFor({ table: 'items', number: 7 }, items)).toEqual({ slug: 'weapons', id: 7 });
		expect(linkFor({ table: 'items', number: 8 }, items)).toBeUndefined();
	});

	it('lays reference rows out in as many columns as the widest row', () => {
		const rows = [
			ref({ text: '10%', cells: [{ text: 'Monster: x(1)', tag: '1' }] }),
			ref({ text: 'Room: y' })
		];
		const n = refColumns(rows);
		expect(n).toBe(2);
		expect(rows.map((r) => refCells(r, n))).toEqual([
			['10%', 'Monster: x(1)'],
			['Room: y', '']
		]);
	});

	it('posts the body and reports a failure with its reason', async () => {
		let seen: { url: string; body: string } | undefined;
		const ok = createPaneClient((async (url: string, init: RequestInit) => {
			seen = { url, body: String(init.body) };
			return new Response(JSON.stringify({ kind: 'spells', number: 3, text: 't', rows: [], references: [], compare: [], errors: [] }));
		}) as never);
		const p = await ok.pane('spells', 3, paneBody(undefined, DEFAULT_FILTER));
		expect(p.text).toBe('t');
		expect(seen?.url).toBe('/api/detail/spells/3');
		const bad = createPaneClient((async () => new Response('no spell 9', { status: 404 })) as never);
		await expect(bad.pane('spells', 9, paneBody(undefined, DEFAULT_FILTER))).rejects.toThrow('no spell 9');
	});
});

import { describe, expect, it, vi } from 'vitest';
import {
	NO_CAUSE,
	NO_FILTERS,
	boundOf,
	causeText,
	coinText,
	copyName,
	createRealmClient,
	facets,
	filterRealm,
	lastOnText,
	pickerLabel,
	realmColumns,
	searchPicker,
	sheetGroups,
	sheetRow,
	storedOnly,
	unexplained,
	wornLines
} from './realm';
import { cormac, roster } from './realm.fixture';

const names = (rows: { first_name: string }[]) => rows.map((r) => r.first_name);

describe('realm list filters', () => {
	it('shows everything with no filters', () => {
		expect(filterRealm(roster, NO_FILTERS)).toHaveLength(4);
	});
	it('quick search matches every word anywhere in the row', () => {
		expect(names(filterRealm(roster, { ...NO_FILTERS, search: 'dwarf priest' }))).toEqual(['Cormac']);
		expect(names(filterRealm(roster, { ...NO_FILTERS, search: 'IRONWOOD' }))).toEqual(['Aldric', 'Cormac']);
		expect(filterRealm(roster, { ...NO_FILTERS, search: 'zzz' })).toEqual([]);
	});
	it('filters by race and class', () => {
		expect(names(filterRealm(roster, { ...NO_FILTERS, race: 1 }))).toEqual(['Aldric', 'Dara']);
		expect(names(filterRealm(roster, { ...NO_FILTERS, race: 1, class: 12 }))).toEqual(['Dara']);
	});
	it('filters by an inclusive level range; blank or junk is no bound', () => {
		expect(names(filterRealm(roster, { ...NO_FILTERS, levelMin: '9', levelMax: '16' }))).toEqual(['Brenna', 'Cormac']);
		expect(names(filterRealm(roster, { ...NO_FILTERS, levelMin: '17' }))).toEqual(['Dara']);
		expect(filterRealm(roster, { ...NO_FILTERS, levelMin: 'x', levelMax: '' })).toHaveLength(4);
		expect(boundOf(' 12 ')).toBe(12);
		expect(boundOf('-3')).toBeUndefined();
	});
	it('lists the races and classes of the roster for the boxes', () => {
		const f = facets(roster);
		expect(f.races.map((r) => r.name)).toEqual(['Dwarf', 'Elf', 'Human']);
		expect(f.classes.map((c) => c.name)).toEqual(['Mage', 'Priest', 'Thief', 'Warrior']);
	});
	it('words the picker rows and searches them like the list', () => {
		expect(pickerLabel(roster[2])).toBe('Cormac Fixture (Dwarf Priest L16)');
		expect(names(searchPicker(roster, 'elf'))).toEqual(['Brenna']);
	});
	it('formats last on in UTC, never for 0', () => {
		expect(lastOnText({ last_on: 1760010800, last_on_iso: '2025-10-09T11:53:20Z' })).toBe('2025-10-09 11:53');
		expect(lastOnText({ last_on: 0, last_on_iso: null })).toBe('never');
	});
	it('has a column for each field the card lists, sortable by value', () => {
		const ids = realmColumns().map((c) => c.colId);
		expect(ids).toEqual(['name', 'last', 'race', 'class', 'level', 'exp', 'gang', 'laston', 'lives', 'differs']);
	});
});

describe('realm sheet from a fixed response', () => {
	it('groups the comparison into stats, vitals and bonuses', () => {
		const g = sheetGroups(cormac.comparison);
		expect(g.map((x) => x.title)).toEqual(['Stats', 'Vitals and skills']);
		expect(g[1].rows.map((r) => r.field)).toEqual(['max_hp', 'picklocks', 'encumbrance_pct']);
	});
	it('shows a rolled value as MME range and flags a difference with its cause', () => {
		const rows = sheetGroups(cormac.comparison).flatMap((g) => g.rows);
		const hp = rows.find((r) => r.field === 'max_hp')!;
		expect(hp).toMatchObject({ touchstone: '107-152', game: '129', differs: false });
		const pick = rows.find((r) => r.field === 'picklocks')!;
		expect(pick).toMatchObject({ touchstone: '65', game: '0', differs: true });
		expect(pick.tip).toContain('picklocks only to a character with the ability');
		expect(rows.find((r) => r.field === 'encumbrance_pct')).toMatchObject({ touchstone: '30%', game: '30%' });
	});
	it('says so when a difference has no known cause or touchstone has no value', () => {
		const base = { field: 'x', label: 'X', game: 5, touchstone: 7, differs: true, tolerance: 'exact' };
		expect(sheetRow(base).tip).toBe(NO_CAUSE);
		expect(sheetRow({ ...base, touchstone: null }).touchstone).toBe('n/a');
		expect(causeText('active-spell')).toContain('running on the character');
		expect(unexplained([base, { ...base, cause: 'kai-mana' }, { ...base, differs: false }])).toBe(1);
	});
	it('lists worn gear by MME slot with the weapon last, and the stored-only values', () => {
		expect(wornLines(cormac).map((w) => [w.slot, w.name])).toEqual([
			['Head', 'silvery skullcap'],
			['Torso', 'shimmering white robes'],
			['Hands', 'padded gloves'],
			['Weapon', 'platinum mace']
		]);
		expect(storedOnly(cormac.game).map((s) => s.label)).toEqual(['HP now', 'Mana now', 'Martial arts']);
		expect(cormac.unmapped[0]).toMatchObject({ kind: 'worn', name: 'skullcap' });
	});
	it('words the coins', () => {
		expect(coinText(cormac.identity.coins)).toBe('163 gold, 51 copper');
		expect(coinText([0, 0, 0, 0, 0])).toBe('none');
	});
});

describe('copy name', () => {
	it('keeps a free name and counts up a taken one', () => {
		expect(copyName('Aldric', ['Bob'])).toBe('Aldric');
		expect(copyName('Aldric', ['Aldric'])).toBe('Aldric (copy)');
		expect(copyName('Aldric', ['Aldric', 'Aldric (copy)'])).toBe('Aldric (copy 2)');
	});
});

describe('realm client', () => {
	it('reads the three endpoints and reports an error body', async () => {
		const fetchFn = vi.fn(async (url: string | URL | Request) =>
			String(url).endsWith('/9') ? new Response('no realm character 9', { status: 404 }) : Response.json({ ok: String(url) })
		);
		const c = createRealmClient(fetchFn as unknown as typeof fetch);
		await expect(c.list()).resolves.toEqual({ ok: '/api/realm/characters' });
		await expect(c.input(3)).resolves.toEqual({ ok: '/api/realm/characters/3/input' });
		await expect(c.detail(9)).rejects.toMatchObject({ status: 404, message: 'no realm character 9' });
	});
});

import { describe, expect, it } from 'vitest';
import type { Chest, Counters, SpellCasts } from './api';
import {
	barIsOn,
	bookRequest,
	bookSeed,
	castRows,
	chestClipText,
	chestRows,
	counterRows,
	isTextColumn,
	spellBarOptions,
	spellFilterOptions,
	spellFiltersActive,
	DEFAULT_SPELL_FILTERS,
	spellOptionsEnabled
} from './spelltools';
import { DEFAULT_FILTER } from './globalfilter';

describe('the Spells tab option boxes (chkSpellOptions_Click)', () => {
	it('enables vs MR and vs Anti-Magic only with Calc. Combat', () => {
		expect(spellOptionsEnabled(true)).toEqual({ vsMr: true, antiMagic: true });
		expect(spellOptionsEnabled(false)).toEqual({ vsMr: false, antiMagic: false });
	});
	it('sends nothing for a disabled box', () => {
		expect(spellBarOptions({ calcCombat: false, vsMr: '50', antiMagic: true })).toEqual({});
		expect(spellBarOptions({ calcCombat: true, vsMr: '50', antiMagic: false })).toEqual({
			calc_combat: true,
			vs_mr: '50'
		});
		expect(spellBarOptions({ calcCombat: true, vsMr: '0', antiMagic: true })).toEqual({
			calc_combat: true,
			vs_mr: '0',
			anti_magic: true
		});
	});
});

describe('the Spells tab filter boxes (FilterSpells)', () => {
	it('sends nothing while every box is at its reset value', () => {
		expect(spellFiltersActive(DEFAULT_SPELL_FILTERS)).toBe(false);
		expect(spellFilterOptions(DEFAULT_SPELL_FILTERS)).toEqual({});
	});
	it('sends all the boxes once one filters', () => {
		const f = { ...DEFAULT_SPELL_FILTERS, magery: 2, target: 3 };
		expect(spellFiltersActive(f)).toBe(true);
		expect(spellFilterOptions(f)).toEqual({ spells: f });
		expect(spellFiltersActive({ ...DEFAULT_SPELL_FILTERS, learnable_only: true })).toBe(true);
	});
});

describe('results dialog rows', () => {
	it('links counters to their records', () => {
		const c: Counters = {
			ask_level: false,
			level: 0,
			rows: [
				{ table: 'items', number: 9, text: 'Item: grey robes (9)' },
				{ table: 'spells', number: 59, text: 'Spell: slow (59)' }
			]
		};
		expect(counterRows(c)).toEqual([
			{ cells: ['Item: grey robes (9)'], link: { table: 'items', number: 9 } },
			{ cells: ['Spell: slow (59)'], link: { table: 'spells', number: 59 } }
		]);
	});
	it('keeps the sub-items of a casts row', () => {
		const c: SpellCasts = {
			status: 'found',
			caption: 'x',
			rows: [
				{
					kind: 'monster',
					text: 'Monster: bat',
					tag: '5',
					cells: [{ text: '10%', tag: null }],
					target: { table: 'monsters', number: 5 }
				}
			]
		};
		expect(castRows(c)).toEqual([
			{ cells: ['Monster: bat', '10%'], link: { table: 'monsters', number: 5 } }
		]);
	});
	it('lists chest contents with the chance first', () => {
		const c: Chest = {
			status: 'rows',
			message: null,
			rows: [{ text: '15.4%', percent: 15.4, item_text: 'Item: azurite stone(881)', item: 881 }]
		};
		expect(chestRows(c)).toEqual([
			{ cells: ['15.4%', 'Item: azurite stone(881)'], link: { table: 'items', number: 881 } }
		]);
	});
});

describe('Copy Chest', () => {
	const chest = (texts: string[]): Chest => ({
		status: 'rows',
		message: null,
		rows: texts.map((text, i) => ({ text, percent: 1, item_text: '', item: i }))
	});
	it('copies the heading and one chance per line, as MME does', () => {
		expect(chestClipText('oak chest', 907, chest(['8.7%', '5.9%']))).toBe(
			'Chest info for oak chest (907):\r\n8.7%\r\n5.9%'
		);
	});
	it('copies nothing for an empty chest', () => {
		expect(chestClipText('oak chest', 907, chest([]))).toBeUndefined();
	});
});

describe('the Spell Book window', () => {
	it('opens from the builder at level 999 with the class magery', () => {
		expect(bookSeed({ action: 'open', class: 9, level: 999, magery: 4, magery_level: 2 })).toEqual({
			class: 9,
			level: '999',
			magery: 4,
			magery_level: 2
		});
		expect(bookSeed({ action: 'no_magic', class: null, level: null, magery: null, magery_level: null })).toBeUndefined();
	});
	it('treats the bar as on when it filters something', () => {
		expect(barIsOn(DEFAULT_FILTER)).toBe(false);
		expect(barIsOn({ ...DEFAULT_FILTER, class: 3 })).toBe(true);
		expect(barIsOn({ ...DEFAULT_FILTER, level: 40 })).toBe(true);
	});
	it('asks with the form, the bar and the character magery', () => {
		const req = bookRequest(
			{ class: 12, level: '30', alignment: 1, magery: 1, magery_level: 3 },
			{ ...DEFAULT_FILTER, class: 12, level: 30 },
			{ type: 1, level: 5 }
		);
		expect(req).toMatchObject({
			class: 12,
			level: '30',
			filter_on: true,
			global_class: 12,
			global_level: '30',
			char_magery: 1,
			char_magery_level: 3
		});
	});
	it('sorts Name, Short, Magery and Detail as text', () => {
		expect([0, 1, 2, 3, 4, 11].map(isTextColumn)).toEqual([false, true, true, true, false, true]);
	});
});

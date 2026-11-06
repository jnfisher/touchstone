import { describe, expect, it, vi } from 'vitest';
import type { Item } from './api';
import {
	BACKSTAB_DEFAULTS,
	CalcError,
	COIN_DEFAULTS,
	EXP_DEFAULTS,
	afterLabel,
	SWING_DEFAULTS,
	backstabRequest,
	coinButtonRequest,
	coinIndex,
	coinTypedRequest,
	createCalcClient,
	expRequest,
	filterWeapons,
	isCalc,
	swingRequest,
	weaponChoices,
	withSpeedOption
} from './calc';

function item(number: number, name: string, over: Partial<Item> = {}): Item {
	return {
		number,
		name,
		item_type: 1,
		in_game: true,
		abilities: [],
		...over
	} as Item;
}

describe('weapon list', () => {
	const items = [
		item(2, 'zweihander'),
		item(1, 'axe', { abilities: [{ ability: 116, value: -10 }] }),
		item(3, 'cloak', { item_type: 2 }),
		item(4, 'ghost blade', { in_game: false }),
		item(5, 'Bow')
	];

	it('keeps in-game weapons, sorted by name', () => {
		expect(weaponChoices(items).map((w) => w.label)).toEqual([
			'axe (1)',
			'Bow (5)',
			'zweihander (2)'
		]);
	});

	it('keeps only weapons with BS accuracy for the backstab calculator', () => {
		expect(weaponChoices(items, { backstabOnly: true }).map((w) => w.number)).toEqual([1]);
	});

	it('filters by every word of the query, case-insensitively', () => {
		const list = weaponChoices(items);
		expect(filterWeapons(list, 'BOW').map((w) => w.number)).toEqual([5]);
		expect(filterWeapons(list, '2 zwei').map((w) => w.number)).toEqual([2]);
		expect(filterWeapons(list, '  ')).toBe(list);
		expect(filterWeapons(list, 'nothing')).toEqual([]);
	});
});

describe('request mapping', () => {
	it('maps the exp form, leaving the table to the server unless typed over', () => {
		expect(expRequest(EXP_DEFAULTS)).toEqual({ class: 0, race: 0, from: '2', to: '255' });
		expect(expRequest({ ...EXP_DEFAULTS, classNumber: 4, raceNumber: 3, table: ' 130 ' })).toEqual({
			class: 4,
			race: 3,
			table: 130,
			from: '2',
			to: '255'
		});
		// A half-typed table box is ignored rather than rejected.
		expect(expRequest({ ...EXP_DEFAULTS, table: '-' })).not.toHaveProperty('table');
		expect(expRequest({ ...EXP_DEFAULTS, table: '' })).not.toHaveProperty('table');
	});

	it('converts the box that was typed in', () => {
		expect(coinTypedRequest(COIN_DEFAULTS, 0)).toEqual({
			source: 0,
			text: '1',
			top: 2,
			bottom: 0
		});
		expect(coinTypedRequest(COIN_DEFAULTS, 1)).toEqual({
			source: 1,
			text: '100',
			top: 2,
			bottom: 0
		});
	});

	it('converts a clicked row from the coin it held, in its own box', () => {
		// The top row held gold (index 2) and silver (1) was just picked.
		const form = { ...COIN_DEFAULTS, coin: [1, 0] as [number, number] };
		expect(coinButtonRequest(form, 0, 2)).toEqual({
			source: 0,
			destination: 0,
			text: '1',
			top: 1,
			bottom: 0,
			force_source: 100
		});
		expect(coinIndex(1_000_000)).toBe(4);
		expect(coinIndex(7)).toBe(0);
	});

	it('maps the swing form to the body', () => {
		expect(swingRequest({ ...SWING_DEFAULTS, weapon: 217, bashing: true })).toEqual({
			weapon: 217,
			combat: 3,
			slowness: false,
			bashing: true,
			level: '60',
			agility: '100',
			strength: '100',
			encum: '1000',
			max_encum: '4800',
			max_encum_auto: true,
			speed: '100',
			speed_option: 1
		});
	});

	it('ticks Slowness with the Slow speed option only', () => {
		const slow = withSpeedOption(SWING_DEFAULTS, 2);
		expect(slow.slowness).toBe(true);
		expect(swingRequest(slow).speed_option).toBe(2);
		expect(withSpeedOption(slow, 3).slowness).toBe(false);
	});

	it('maps the backstab form to the body', () => {
		expect(backstabRequest({ ...BACKSTAB_DEFAULTS, weapon: 875, classStealth: true })).toEqual({
			weapon: 875,
			level: '50',
			stealth: '100',
			strength: '100',
			bs_min: '0',
			bs_max: '0',
			max_dmg: '0',
			class_stealth: true
		});
	});

	it('knows the calculator slugs', () => {
		expect(isCalc('swing')).toBe(true);
		expect(isCalc('weapons')).toBe(false);
	});
});

describe('calc client', () => {
	it('posts JSON to /api/calc/<name>', async () => {
		const f = vi.fn(async (_p: string, _i?: RequestInit) => new Response('{"table":100}'));
		const c = createCalcClient(f as unknown as typeof fetch);
		const out = await c.exp(expRequest(EXP_DEFAULTS));
		expect(out.table).toBe(100);
		const [path, init] = f.mock.calls[0];
		expect(path).toBe('/api/calc/exp');
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual(expRequest(EXP_DEFAULTS));
	});

	it('turns a 400 into an error carrying the reason', async () => {
		const f = vi.fn(async () => new Response('combat must be 1 (Poor) to 5 (Excellent)', { status: 400 }));
		const c = createCalcClient(f as unknown as typeof fetch);
		const err = await c.swing(swingRequest(SWING_DEFAULTS)).catch((e: unknown) => e);
		expect(err).toBeInstanceOf(CalcError);
		expect((err as CalcError).status).toBe(400);
		expect((err as CalcError).message).toMatch(/combat must be/);
	});
});

describe('captions', () => {
	it('strips the label from a result caption', () => {
		expect(afterLabel('Energy per swing: 58')).toBe('58');
		expect(afterLabel('QND Crits: None')).toBe('None');
		expect(afterLabel('Raw swing: 0')).toBe('0');
		expect(afterLabel('')).toBe('');
	});
});

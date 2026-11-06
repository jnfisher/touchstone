import { describe, expect, it, vi } from 'vitest';
import type { Item, Monster } from './api';
import {
	BACKSTAB_DEFAULTS,
	CALCULATORS,
	HIT_DEFAULTS,
	applyHit,
	captionLines,
	hitRequest,
	monsterChoices,
	type HitResult,
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
		const f = vi.fn(
			async () => new Response('combat must be 1 (Poor) to 5 (Excellent)', { status: 400 })
		);
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

describe('hit calculator', () => {
	const answer: HitResult = {
		calc_type: 'normal',
		attacker: 'manual',
		defender: 'monster',
		attacker_monster_enabled: true,
		text: ['80', '50', '0', '0', '0', '25'],
		shadow: false,
		see_hidden: true,
		evil_index: 0,
		enabled: {
			prot_evil: false,
			vile_ward: false,
			percep: true,
			shadow: false,
			see_hidden: true,
			monster_list: true
		},
		bs_label: 'vs BS Defense',
		main_text: '',
		sub_left: '',
		sub_right: '',
		sub_right_bold: false,
		hit: 8,
		dodge: 0,
		overall_hit: 8,
		state: { ...HIT_DEFAULTS.state, monster_ac: 50, defender_ac: 7 },
		errors: []
	};

	it('is the fifth entry of the calculators menu', () => {
		expect(CALCULATORS.map((c) => c.slug)).toEqual(['exp', 'coin', 'swing', 'backstab', 'hit']);
		expect(isCalc('hit')).toBe(true);
	});

	it('maps the form and the event to the body', () => {
		const body = hitRequest(HIT_DEFAULTS, { event: 'refresh' });
		expect(body).toMatchObject({
			event: 'refresh',
			calc_type: 'normal',
			attacker: 'character',
			defender: 'character',
			text: ['100', '0', '10', '0', '0', '0'],
			shadow: false,
			see_hidden: false,
			evil_index: 0
		});
		// Nothing chosen yet: no monster, no class, no weapon, and no flags for the server to trust.
		expect(body).not.toHaveProperty('monster');
		expect(body).not.toHaveProperty('enabled');
		expect(body.character).not.toHaveProperty('class');
		expect(body.character).not.toHaveProperty('bs_weapon');
		expect(body.character).toMatchObject({
			accuracy: '0',
			shadow_ac: 0,
			strike: 'normal',
			accy_adj: 0
		});
	});

	it('carries the arrow, the monster, the class and the weapon when chosen', () => {
		const form = {
			...HIT_DEFAULTS,
			monster: 13,
			character: {
				...HIT_DEFAULTS.character,
				classNumber: 8,
				bsWeapon: 875,
				shadowAc: ' 10 ',
				accyAdj: 'x',
				strike: 'bash' as const
			}
		};
		const body = hitRequest(form, { event: 'modify', index: 3 });
		expect(body).toMatchObject({ event: 'modify', index: 3, monster: 13 });
		expect(body.character).toMatchObject({
			class: 8,
			bs_weapon: 875,
			shadow_ac: 10,
			accy_adj: 0,
			strike: 'bash'
		});
	});

	it('applies an answer: boxes only when the event rewrites them', () => {
		const typed = { ...HIT_DEFAULTS, text: ['1', '2', '3', '4', '5', '6'] };
		const kept = applyHit(typed, answer, { event: 'calc' });
		expect(kept.text).toEqual(['1', '2', '3', '4', '5', '6']);
		expect(kept.attacker).toBe('manual');
		expect(kept.enabled).toEqual(answer.enabled);
		expect(kept.state.defender_ac).toBe(7);

		const filled = applyHit(typed, answer, { event: 'monster' });
		expect(filled.text).toEqual(answer.text);
		expect(filled.seeHidden).toBe(true);
		expect(applyHit(typed, answer, { event: 'modify', index: 0 }).text).toEqual(answer.text);
	});

	it('lists named, in-game monsters as "name (number)"', () => {
		const m = (number: number, name: string | null, in_game = true) =>
			({ number, name, in_game }) as Monster;
		expect(
			monsterChoices([
				m(1, 'rat'),
				m(2, ''),
				m(3, null),
				m(4, 'sdf test'),
				m(5, 'ghost', false),
				m(6, 'orc')
			])
		).toEqual([
			{ number: 1, name: 'rat', label: 'rat (1)' },
			{ number: 6, name: 'orc', label: 'orc (6)' }
		]);
	});

	it('splits MME captions on CRLF', () => {
		expect(captionLines('Hit Min-Cap:\r\n8% - 99%\r\nDodge Cap:\r\n95%')).toEqual([
			'Hit Min-Cap:',
			'8% - 99%',
			'Dodge Cap:',
			'95%'
		]);
		expect(captionLines('')).toEqual([]);
	});
});

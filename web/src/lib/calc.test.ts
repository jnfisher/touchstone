import { describe, expect, it, vi } from 'vitest';
import type { ClassRec, Item, Monster } from './api';
import { newCharacter, type EvalState } from './character';
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
	backstabFromCharacter,
	backstabRequest,
	calcCharacter,
	captionNumber,
	classCombat,
	classHasStealth,
	coinCharm,
	hitFromCharacter,
	megaFromFields,
	strikeOf,
	swingFromCharacter,
	coinButtonRequest,
	coinIndex,
	coinTypedRequest,
	createCalcClient,
	pctText,
	SIM_DEFAULTS,
	simRequest,
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
			speed_option: 1,
			hit_pct: '',
			hit_avg: '',
			extra_pct: '',
			extra_avg: '',
			crit_pct: '',
			crit_avg: ''
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

	it('is the sixth entry of the calculators menu', () => {
		expect(CALCULATORS.map((c) => c.slug)).toEqual(['exp', 'coin', 'swing', 'backstab', 'hit', 'monster-sim', 'weapon-sim']);
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


// ---------- the current character ----------

const warrior: ClassRec = {
	number: 1,
	name: 'Warrior',
	min_hits: 0,
	max_hits: 0,
	exp_table: 0,
	magery_type: 0,
	magery_lvl: 0,
	weapon_type: 0,
	armour_type: 0,
	combat_lvl: 6,
	abilities: []
};
const thief: ClassRec = { ...warrior, number: 8, name: 'Thief', combat_lvl: 4, abilities: [{ ability: 103, value: 1 }] };

/** An evaluation with the stat captions and tags the mapping reads. */
function evaluation(over: { stats?: Record<number, string>; tags?: Record<number, number> } = {}): EvalState {
	const stats = Array<string>(47).fill('0');
	for (const [i, v] of Object.entries(over.stats ?? {})) stats[Number(i)] = v;
	const tags = Array<number>(47).fill(0);
	for (const [i, v] of Object.entries(over.tags ?? {})) tags[Number(i)] = v;
	return {
		stat_values: [120, 60, 60, 90, 100, 70],
		stats,
		calc: {
			stat_tags: tags,
			shadow_ac: 10,
			vile_ward: 0,
			accy_abilities: 2,
			accy_other: 1,
			accy_items: 3,
			attack_accy_adj: -15,
			weapon_number: 875,
			weapon_accy: 4,
			weapon_bs_accy: 5
		}
	} as unknown as EvalState;
}

function loaded(over: Partial<ReturnType<typeof newCharacter>> = {}) {
	const doc = newCharacter('Brisa');
	Object.assign(doc, { class: 1, race: 2, level: 50, equipped: doc.equipped.map((v, i) => (i === 16 ? 875 : v)) }, over);
	return doc;
}

describe('captionNumber and class helpers', () => {
	it('reads captions like Val', () => {
		expect(captionNumber('1,234')).toBe(1234);
		expect(captionNumber('-10')).toBe(-10);
		expect(captionNumber('12.5%')).toBe(12.5);
		expect(captionNumber(undefined)).toBe(0);
		expect(captionNumber('abc')).toBe(0);
	});
	it('ports GetClassCombat and GetClassStealth', () => {
		expect(classCombat(warrior)).toBe(4);
		expect(classCombat(undefined)).toBe(1);
		expect(classHasStealth(thief)).toBe(true);
		expect(classHasStealth(warrior)).toBe(false);
	});
	it('names the strike from the attack choice', () => {
		expect(strikeOf(1, 0)).toBe('normal');
		expect(strikeOf(6, 0)).toBe('bash');
		expect(strikeOf(7, 0)).toBe('smash');
		expect(strikeOf(4, 2)).toBe('kick');
		expect(strikeOf(4, 3)).toBe('jumpkick');
		expect(strikeOf(4, 1)).toBe('normal');
	});
});

describe('calcCharacter', () => {
	const ev = evaluation({
		stats: { 0: '125', 1: '7,776', 11: '7', 14: '3', 15: '9', 19: '40' },
		tags: { 2: 12.9, 8: 37, 10: 187, 13: -10, 20: 5 }
	});

	it('is undefined without class and race or without an evaluation', () => {
		expect(calcCharacter(newCharacter('x'), ev, [warrior])).toBeUndefined();
		expect(calcCharacter(loaded(), undefined, [warrior])).toBeUndefined();
	});

	it('takes stats from the evaluation, not from the typed document', () => {
		const c = calcCharacter(loaded(), ev, [warrior, thief])!;
		expect(c).toMatchObject({
			classNumber: 1,
			combat: 4,
			classStealth: false,
			level: '50',
			// the stats with bless and gear applied (txtCharStats(i).Tag)
			strength: '120',
			agility: '90',
			charm: 70,
			encum: '125',
			maxEncum: '7776',
			maxDamage: '7',
			stealth: '40',
			bsMin: '3',
			bsMax: '9',
			accuracy: '187',
			ac: '12',
			dodge: '37',
			protEvil: '5',
			bsAccuracy: '-10',
			weapon: 875,
			bsWeapon: 875,
			accyAdj: -15,
			shadowAc: 10,
			weaponNumber: 875
		});
	});

	it('uses the chosen backstab weapon only while Backstab is on', () => {
		const doc = loaded();
		doc.attack.backstab_weapon = 68;
		expect(calcCharacter(doc, ev, [warrior])!.bsWeapon).toBe(875);
		doc.attack.backstab = true;
		expect(calcCharacter(doc, ev, [warrior])!.bsWeapon).toBe(68);
		doc.equipped[16] = 0;
		doc.attack.backstab = false;
		expect(calcCharacter(doc, ev, [warrior])!.bsWeapon).toBeNull();
	});

	it('keeps combat in 1..5, 3 otherwise', () => {
		const odd: ClassRec = { ...warrior, combat_lvl: 9 };
		expect(calcCharacter(loaded(), ev, [odd])!.combat).toBe(3);
	});
});

describe('filling the calculators', () => {
	const ev = evaluation({ stats: { 0: '125', 1: '7776', 11: '7', 14: '3', 15: '9', 19: '40' }, tags: { 10: 187 } });
	const c = calcCharacter(loaded(), ev, [thief].map((t) => ({ ...t, number: 1 })))!;

	it('swing copies what Form_Load copies', () => {
		const f = swingFromCharacter(c, { ...SWING_DEFAULTS });
		expect(f).toMatchObject({
			combat: 2,
			level: '50',
			strength: '120',
			agility: '90',
			encum: '125',
			maxEncum: '7776',
			maxEncumAuto: false,
			weapon: 875
		});
	});

	it('swing keeps a box whose character value is 0 and the weapon when none is equipped', () => {
		const bare = calcCharacter(loaded({ equipped: Array<number>(20).fill(0) }), evaluation(), [warrior])!;
		const f = swingFromCharacter(bare, { ...SWING_DEFAULTS, weapon: 7, encum: '1000' });
		expect(f.encum).toBe('1000');
		expect(f.maxEncum).toBe(SWING_DEFAULTS.maxEncum);
		expect(f.maxEncumAuto).toBe(true);
		expect(f.weapon).toBe(7);
	});

	it('backstab copies strength, level, damage boxes, class stealth and weapon', () => {
		const f = backstabFromCharacter(c, { ...BACKSTAB_DEFAULTS });
		expect(f).toMatchObject({
			strength: '120',
			level: '50',
			maxDmg: '7',
			stealth: '40',
			classStealth: true,
			bsMin: '3',
			bsMax: '9',
			weapon: 875
		});
	});

	it('hit fills the character section, including what SetHitCalcVals reads from the globals', () => {
		const h = hitFromCharacter(c, HIT_DEFAULTS.character);
		expect(h).toMatchObject({
			accuracy: '187',
			classNumber: 1,
			bsWeapon: 875,
			strike: 'normal',
			accyAdj: '-15',
			shadowAc: '10',
			strength: '120',
			level: '50',
			accyAbilities: '2',
			accyOther: '1',
			accyItems: '3',
			weaponAccy: '4',
			weaponBsAccy: '5'
		});
		const body = hitRequest({ ...HIT_DEFAULTS, character: h }, { event: 'refresh' });
		expect(body.character).toMatchObject({
			accuracy: '187',
			class: 1,
			bs_weapon: 875,
			accy_adj: -15,
			shadow_ac: 10,
			strength: '120',
			level: '50',
			accy_abilities: 2,
			weapon_number: 875
		});
	});

	it('coin sends the charm only while the character is used and its global filter is on', () => {
		expect(coinCharm(undefined)).toBeUndefined();
		expect(coinCharm({ ...c, globalFilter: false })).toBeUndefined();
		expect(coinCharm({ ...c, globalFilter: true })).toBe(70);
	});
});

describe('MegaMUD paste', () => {
	it('maps the six fields to the boxes in order', () => {
		expect(megaFromFields(['43', '34', '12', '6', '5', '55'])).toEqual({
			hitPct: '43',
			hitAvg: '34',
			extraPct: '12',
			extraAvg: '6',
			critPct: '5',
			critAvg: '55'
		});
	});
	it('goes to the swing request', () => {
		const body = swingRequest({ ...SWING_DEFAULTS, mega: megaFromFields(['43', '34', '0', '0', '5', '55']) });
		expect(body).toMatchObject({ hit_pct: '43', hit_avg: '34', crit_pct: '5', crit_avg: '55', extra_pct: '0' });
	});
	it('posts the text to /api/calc/mega-stat', async () => {
		const fetchFn = vi.fn(async () => new Response(JSON.stringify({ ok: false, fields: null, message: 'm', error: null })));
		const client = createCalcClient(fetchFn as unknown as typeof fetch);
		const r = await client.megaStat('Hit: 1%');
		expect(r.message).toBe('m');
		const [url, init] = fetchFn.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/calc/mega-stat');
		expect(JSON.parse(init.body as string)).toEqual({ text: 'Hit: 1%' });
	});
});

describe('monster attack simulator', () => {
	it('opens with the reset defenses and 500 rounds', () => {
		expect(SIM_DEFAULTS.rounds).toBe('500');
		expect(SIM_DEFAULTS.boxes).toMatchObject({ ac: '0', mr: '50', anti_magic: false });
	});

	it('sends the typed boxes and leaves an empty seed to the server', () => {
		const body = simRequest({ ...SIM_DEFAULTS, monster: 13, classNumber: 2, boxes: { ...SIM_DEFAULTS.boxes, ac: '40' } });
		expect(body).toMatchObject({
			monster: 13,
			rounds: '500',
			dynamic: false,
			log_rounds: 100,
			defender: { ac: '40', mr: '50', class: 2 }
		});
		expect(body).not.toHaveProperty('seed');
	});

	it('sends the character instead of the boxes when one is given', () => {
		const body = simRequest({ ...SIM_DEFAULTS, monster: 5, seed: '90210', logRounds: '5000' }, { name: 'x' });
		expect(body.defender).toEqual({ character: { name: 'x' } });
		expect(body).toMatchObject({ seed: 90210, log_rounds: 1000 });
	});

	it('shows whole percentages plain and others with one decimal', () => {
		expect(pctText(100)).toBe('100');
		expect(pctText(33.3)).toBe('33.3');
	});

	it('posts to /api/calc/monster-sim', async () => {
		const fetchFn = vi.fn(async () => new Response('{}'));
		const client = createCalcClient(fetchFn as unknown as typeof fetch);
		await client.monsterSim(simRequest({ ...SIM_DEFAULTS, monster: 1 }));
		await client.monsterSimDefender({ name: 'x' });
		const urls = fetchFn.mock.calls.map((c) => (c as unknown as [string])[0]);
		expect(urls).toEqual(['/api/calc/monster-sim', '/api/calc/monster-sim/defender']);
	});
});

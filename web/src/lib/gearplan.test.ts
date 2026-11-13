import { describe, expect, it, vi } from 'vitest';
import {
	GEAR_PLAN_DEFAULTS,
	chanceText,
	changePct,
	changedSlots,
	closedText,
	createGearPlanClient,
	gearPlanRequest,
	goalSwings,
	itemNote,
	killList,
	num,
	orderedSlots,
	otherText,
	otherWays,
	shopList,
	signed,
	skipItem,
	type Acquisition,
	type GearPlanForm,
	type GearPlanResult,
	type PlanItem,
	type PlanSlot,
	type Source
} from './gearplan';

const item = (number: number, o: Partial<PlanItem> = {}): PlanItem => ({
	number,
	name: `item ${number}`,
	level: 0,
	armour_class: 30,
	damage_resist: 5,
	encum: 100,
	limited: false,
	needs_level: false,
	wrong_alignment: false,
	unobtainable: false,
	...o
});

const src = (kind: Source['kind'], number: number, o: Partial<Source> = {}): Source => ({
	kind,
	number,
	map: 0,
	name: `${kind} ${number}`,
	pct: null,
	kills: null,
	chest: null,
	chest_name: null,
	chest_pct: null,
	one_time: false,
	...o
});

const get = (n: number, sources: Source[]): Acquisition => ({ item: n, name: `item ${n}`, sources, more: 0, orphan_chests: 0 });

const slot = (n: number, name: string, o: Partial<PlanSlot> = {}): PlanSlot => ({
	slot: n,
	name,
	closed: null,
	current: null,
	suggested: null,
	changed: false,
	gain: 0,
	alternatives: [],
	acquisition: null,
	...o
});

const plan = (slots: PlanSlot[]): GearPlanResult => ({
	goal: 'melee',
	metric: 'Damage a round',
	before: 100,
	after: 150,
	slots,
	stats: [],
	candidates: 200,
	evaluations: 500,
	passes: 2,
	level: 25,
	skipped: [],
	unmet: [],
	elapsed_ms: 20
});

const form = (o: Partial<GearPlanForm> = {}): GearPlanForm => ({ ...structuredClone(GEAR_PLAN_DEFAULTS), ...o });

describe('request', () => {
	it('sends the defaults with the character', () => {
		expect(gearPlanRequest(form(), { class: 8 })).toEqual({
			character: { class: 8 },
			goal: 'melee',
			ignore_level: false,
			ignore_alignment: false,
			include_limited: true,
			only_obtainable: true,
			second_wrist: false,
			target: { ac: 0, dr: 0, dodge: 0, bs_defense: 0 },
			locked: []
		});
	});

	it('sends the limits that hold a number and the locks in order', () => {
		const body = gearPlanRequest(form({ minAc: ' 45 ', maxEncum: 'abc', locked: [9, 0, 4] }), {});
		expect(body.min_ac).toBe(45);
		expect('max_encum' in body).toBe(false);
		expect(body.locked).toEqual([0, 4, 9]);
		expect(gearPlanRequest(form({ maxEncum: '33.5', minAc: '-2' }), {})).toMatchObject({ max_encum: 33.5 });
		expect('min_ac' in gearPlanRequest(form({ minAc: '-2' }), {})).toBe(false);
	});

	it('skips items for "next best", once each', () => {
		expect('skipped' in gearPlanRequest(form(), {})).toBe(false);
		expect(gearPlanRequest(form({ skipped: [7, 3] }), {})).toMatchObject({ skipped: [7, 3] });
		expect(skipItem([7], 3)).toEqual([7, 3]);
		const same = [7, 3];
		expect(skipItem(same, 3)).toBe(same);
		const full = Array.from({ length: 200 }, (_, i) => i);
		expect(skipItem(full, 999)).toBe(full);
	});

	it('sends the target only for a goal that swings at it', () => {
		const target = { ac: '80', dr: '35', dodge: '10', bs_defense: '50', hp: '3200' };
		expect(gearPlanRequest(form({ goal: 'backstab', target }), {}).target).toEqual({ ac: 80, dr: 35, dodge: 10, bs_defense: 50 });
		expect(gearPlanRequest(form({ goal: 'armour', target }), {}).target).toEqual({ ac: 0, dr: 0, dodge: 0, bs_defense: 0 });
		expect([goalSwings('melee'), goalSwings('backstab'), goalSwings('caster'), goalSwings('mana_regen')]).toEqual([true, true, false, false]);
	});
});

describe('wording', () => {
	it('formats numbers, gains and the change', () => {
		expect([num(12), num(12.25), signed(3), signed(-1.5), signed(0)]).toEqual(['12', '12.3', '+3', '-1.5', '0']);
		expect([changePct(100, 153), changePct(100, 90), changePct(0, 50), changePct(40, 40)]).toEqual(['+53%', '-10%', '', '']);
	});

	it('says what stops an item being worn today', () => {
		expect(itemNote(item(1), 25)).toBe('');
		expect(itemNote(null, 25)).toBe('');
		expect(itemNote(item(1, { needs_level: true, level: 40, wrong_alignment: true, limited: true, unobtainable: true }), 25)).toBe(
			'level 40 (+15), alignment, limited, no known source'
		);
	});

	it('words a closed slot and a chance', () => {
		expect([closedText('locked'), closedText('two_handed'), closedText(null)]).toEqual(['locked', 'the weapon takes both hands', '']);
		expect(chanceText({ pct: 5, kills: 20 })).toBe('5% (about 20 kills)');
		expect(chanceText({ pct: 0.275, kills: 363.6 })).toBe('0.28% (about 364 kills)');
		expect(chanceText({ pct: 37.6, kills: 2.66 })).toBe('38% (about 3 kills)');
		expect(chanceText({ pct: 100, kills: 1 })).toBe('100%');
		expect(chanceText({ pct: null, kills: null })).toBe('');
	});

	it('words the other sources', () => {
		expect(otherText(src('room', 7, { map: 1, name: 'Dark Cave' }))).toBe('Room 1/7: Dark Cave');
		expect(otherText(src('room', 7, { map: 1, name: '' }))).toBe('Room 1/7');
		expect(otherText(src('npc', 6, { name: 'Jorah' }))).toBe('Given by Jorah');
		expect(otherText(src('script', 9, { pct: 2 }))).toBe('A script gives it (2%)');
		expect(otherText(src('script', 9))).toBe('A script gives it');
	});
});

describe('lists', () => {
	const slots = [
		slot(0, 'Head', {
			changed: true,
			gain: 4,
			suggested: item(10),
			acquisition: get(10, [
				src('shop', 3, { name: 'Armoury' }),
				src('monster', 5, { name: 'ogre', pct: 10, kills: 10 }),
				src('chest', 5, { name: 'ogre', pct: 25, kills: 4, chest: 300, chest_name: 'oak chest', chest_pct: 50 })
			])
		}),
		slot(2, 'Neck', {
			changed: true,
			gain: 9,
			suggested: item(11),
			acquisition: get(11, [
				src('monster', 5, { name: 'ogre', pct: 2, kills: 50 }),
				src('monster', 8, { name: 'troll', pct: 40, kills: 2.5 }),
				src('shop', 3, { name: 'Armoury', one_time: true }),
				src('shop', 4, { name: 'Bazaar' }),
				src('room', 7, { map: 1 }),
				src('script', 9, { pct: 1 })
			])
		}),
		slot(4, 'Torso', { changed: true, gain: 1, suggested: item(12), acquisition: get(12, []) }),
		slot(5, 'Arms', { current: item(13), suggested: item(13) }),
		slot(16, 'Weapon', { closed: 'weapon' })
	];
	const p = plan(slots);

	it('orders by slot, or the biggest gain first with unchanged slots last', () => {
		expect(orderedSlots(p, 'slot').map((s) => s.slot)).toEqual([0, 2, 4, 5, 16]);
		expect(orderedSlots(p, 'gain').map((s) => s.slot)).toEqual([2, 0, 4, 5, 16]);
		expect(changedSlots(p).map((s) => s.slot)).toEqual([0, 2, 4]);
	});

	it('groups the kills by monster, the one that gives most first', () => {
		const kills = killList(p);
		expect(kills.map((k) => [k.monster, k.name])).toEqual([
			[5, 'ogre'],
			[8, 'troll']
		]);
		// the ogre gives both items; for the head item its chest is the surer way, so that is the line shown
		expect(kills[0].drops).toEqual([
			{ item: 10, itemName: 'item 10', slot: 'Head', pct: 25, kills: 4, chestName: 'oak chest', chestPct: 50 },
			{ item: 11, itemName: 'item 11', slot: 'Neck', pct: 2, kills: 50, chestName: null, chestPct: null }
		]);
		expect(kills[1].drops.map((d) => d.item)).toEqual([11]);
	});

	it('groups the shops, the one that sells most first', () => {
		expect(shopList(p)).toEqual([
			{
				shop: 3,
				name: 'Armoury',
				items: [
					{ item: 10, itemName: 'item 10', slot: 'Head', oneTime: false },
					{ item: 11, itemName: 'item 11', slot: 'Neck', oneTime: true }
				]
			},
			{ shop: 4, name: 'Bazaar', items: [{ item: 11, itemName: 'item 11', slot: 'Neck', oneTime: false }] }
		]);
	});

	it('lists what is found, given or scripted, and what has no source at all', () => {
		const ways = otherWays(p);
		expect(ways.map((w) => [w.item, w.sources.map((s) => s.kind), w.nothing])).toEqual([
			[11, ['room', 'script'], false],
			[12, [], true]
		]);
	});
});

describe('client', () => {
	it('posts the body and returns the plan', async () => {
		const answer = plan([]);
		const fetchFn = vi.fn(async () => new Response(JSON.stringify(answer)));
		const body = gearPlanRequest(form(), {});
		await expect(createGearPlanClient(fetchFn as typeof fetch).run(body)).resolves.toEqual(answer);
		expect(fetchFn).toHaveBeenCalledWith('/api/calc/gear-plan', expect.objectContaining({ method: 'POST', body: JSON.stringify(body) }));
	});

	it('reports the server reason', async () => {
		const fetchFn = vi.fn(async () => new Response('a limit is out of range', { status: 400 }));
		await expect(createGearPlanClient(fetchFn as typeof fetch).run(gearPlanRequest(form(), {}))).rejects.toThrow('a limit is out of range');
	});
});

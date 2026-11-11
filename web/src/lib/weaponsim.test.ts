import { describe, expect, it, vi } from 'vitest';
import type { ColDef } from 'ag-grid-community';
import type { Monster } from './api';
import {
	WEAPON_SIM_DEFAULTS,
	bestRegular,
	createWeaponSimClient,
	killText,
	rowNote,
	signedPct,
	targetFromMonster,
	targetOf,
	versusEquipped,
	viewMode,
	viewRows,
	weaponSimColumns,
	weaponSimRequest,
	type SimMode,
	type WeaponSimForm,
	type WeaponSimResult,
	type WeaponSimRow
} from './weaponsim';

const mode = (mean: number, o: Partial<SimMode> = {}): SimMode => ({
	expected: Math.round(mean),
	sim: { rounds: 5000, mean, sd: 10, min: 0, p10: mean / 2, median: mean, p90: mean * 2, max: mean * 4, zero_pct: 0 },
	swings: 3.2,
	hit: 90,
	crit: 12,
	min_dmg: 4,
	max_dmg: 10,
	avg_hit: 7,
	avg_crit: 21,
	max_crit: 40,
	extra: 0,
	detail: 'Swings: 3.2, Avg Hit: 7',
	kill: null,
	...o
});

const row = (number: number, o: Partial<WeaponSimRow> = {}): WeaponSimRow => ({
	number,
	name: `weapon ${number}`,
	level: 0,
	weapon_type: 2,
	two_handed: false,
	min: 4,
	max: 10,
	speed: 1000,
	str_req: 0,
	accy: 0,
	limit: 0,
	equipped: false,
	pinned: false,
	needs_level: false,
	wrong_alignment: false,
	unobtainable: false,
	attack: mode(100),
	bash: mode(80),
	backstab: null,
	...o
});

const result = (rows: WeaponSimRow[], o: Partial<WeaponSimResult> = {}): WeaponSimResult => ({
	rows,
	usable: 40,
	simulated: rows.length,
	rounds: 5000,
	candidates: 25,
	seed: 1,
	target: { ac: 0, dr: 0, dodge: 0, bs_defense: 0, hp: 0 },
	can_backstab: true,
	equipped_weapon: rows.find((r) => r.equipped)?.number ?? 0,
	level: 25,
	elapsed_ms: 20,
	...o
});

const form = (o: Partial<WeaponSimForm> = {}): WeaponSimForm => ({ ...structuredClone(WEAPON_SIM_DEFAULTS), ...o });

describe('request', () => {
	it('sends the defaults with the character', () => {
		expect(weaponSimRequest(form(), { class: 8 })).toEqual({
			character: { class: 8 },
			ignore_level: false,
			ignore_alignment: false,
			include_limited: true,
			only_obtainable: true,
			target: { ac: 0, dr: 0, dodge: 0, bs_defense: 0, hp: 0 },
			rounds: 5000,
			candidates: 25,
			seed: 1,
			pinned: []
		});
	});

	it('reads the target boxes as whole numbers within what the server takes', () => {
		const f = form({ target: { ac: '55.9', dr: 'abc', dodge: '-250', bs_defense: '-4', hp: '99999999' } });
		expect(targetOf(f)).toEqual({ ac: 55, dr: 0, dodge: -100, bs_defense: 0, hp: 1_000_000 });
	});

	it('sends at most twenty pinned weapons', () => {
		const pinned = Array.from({ length: 30 }, (_, i) => i + 1);
		expect(weaponSimRequest(form({ pinned }), {}).pinned).toHaveLength(20);
	});

	it('fills the target from a monster', () => {
		const m = {
			armour_class: 80,
			damage_resist: 35,
			bs_defense: 50,
			hp: 3200,
			abilities: [
				{ ability: 28, value: 3 },
				{ ability: 34, value: 15 }
			]
		} as Monster;
		expect(targetFromMonster(m)).toEqual({ ac: '80', dr: '35', dodge: '15', bs_defense: '50', hp: '3200' });
		expect(targetFromMonster({ ...m, abilities: [] } as Monster).dodge).toBe('0');
	});
});

describe('views', () => {
	const basher = row(2, { attack: mode(60), bash: mode(90) });
	const stabber = row(3, { equipped: true, attack: mode(50), bash: mode(40), backstab: mode(200) });
	const r = result([row(1), basher, stabber]);

	it('ranks a regular round by the better of attack and bash', () => {
		expect(bestRegular(row(1))?.key).toBe('attack');
		expect(bestRegular(basher)?.key).toBe('bash');
		expect(bestRegular(row(4, { attack: null }))?.key).toBe('bash');
		expect(bestRegular(row(5, { attack: null, bash: null }))).toBeUndefined();
		expect(viewMode(basher, 'regular')?.sim.mean).toBe(90);
		expect(viewMode(stabber, 'backstab')?.sim.mean).toBe(200);
		expect(viewMode(basher, 'backstab')).toBeUndefined();
	});

	it('lists only backstab weapons in the backstab view', () => {
		expect(viewRows(r, 'regular').map((x) => x.number)).toEqual([1, 2, 3]);
		expect(viewRows(r, 'backstab').map((x) => x.number)).toEqual([3]);
	});

	it('compares with the weapon in hand in the same view', () => {
		expect(versusEquipped(row(1), r, 'regular')).toBe(100);
		expect(versusEquipped(stabber, r, 'regular')).toBe(0);
		expect(versusEquipped(row(1), r, 'backstab')).toBeUndefined();
		expect(versusEquipped(row(1), result([row(1)]), 'regular')).toBeUndefined();
		expect([signedPct(100), signedPct(-12.4), signedPct(0.2), signedPct(undefined)]).toEqual(['+100%', '-12%', '0%', '']);
	});

	it('words the kill time and the row note', () => {
		expect(killText({ mean: 2.44, median: 2, p90: 3, estimated: false })).toBe('2.4');
		expect(killText({ mean: 136.6, median: 137, p90: 137, estimated: true })).toBe('~137');
		expect(killText(null)).toBe('');
		expect(rowNote(row(1, { equipped: true, pinned: true }), 25)).toBe('equipped');
		expect(rowNote(row(1, { needs_level: true, level: 40, wrong_alignment: true, limit: 1, pinned: true }), 25)).toBe(
			'level 40 (+15), alignment, limited, pinned'
		);
		expect(rowNote(row(1, { unobtainable: true }), 25)).toBe('no known source');
		expect(rowNote(row(1), 25)).toBe('');
	});
});

describe('grid', () => {
	const stabber = row(3, { equipped: true, attack: mode(50), bash: mode(40), backstab: mode(200, { hit: 71 }) });
	const r = result([row(1), stabber]);
	const ids = (cols: ColDef<WeaponSimRow>[]) => cols.map((c) => c.colId);
	const value = (cols: ColDef<WeaponSimRow>[], id: string, data: WeaponSimRow) => {
		const c = cols.find((x) => x.colId === id)!;
		const get = c.valueGetter as (p: { data: WeaponSimRow }) => unknown;
		return get({ data });
	};

	it('shows attack and bash side by side for regular rounds, sorted by damage', () => {
		const cols = weaponSimColumns('regular', r);
		expect(ids(cols)).toEqual(['name', 'damage', 'versus', 'best', 'attack', 'bash', 'swings', 'low', 'high', 'hit', 'crit', 'level', 'hands', 'note']);
		expect(cols.find((c) => c.colId === 'damage')?.sort).toBe('desc');
		expect(value(cols, 'damage', row(1))).toBe(100);
		expect(value(cols, 'best', row(1))).toBe('Attack');
		expect(value(cols, 'versus', row(1))).toBe(100);
	});

	it('ranks by the opening round for backstab rounds', () => {
		const cols = weaponSimColumns('backstab', r);
		expect(ids(cols)).toEqual(['name', 'damage', 'versus', 'low', 'high', 'hit', 'level', 'hands', 'note']);
		expect(cols.find((c) => c.colId === 'damage')?.headerName).toBe('Backstab');
		expect(value(cols, 'damage', stabber)).toBe(200);
		expect(value(cols, 'hit', stabber)).toBe(71);
	});

	it('adds the kill column when the target has hit points', () => {
		const hp = result([row(1)], { target: { ac: 0, dr: 0, dodge: 0, bs_defense: 0, hp: 300 } });
		expect(ids(weaponSimColumns('regular', hp))).toContain('kill');
		expect(ids(weaponSimColumns('regular', r))).not.toContain('kill');
	});
});

describe('client', () => {
	it('posts the body and returns the result', async () => {
		const answer = result([]);
		const fetchFn = vi.fn(async () => new Response(JSON.stringify(answer)));
		const body = weaponSimRequest(form(), {});
		await expect(createWeaponSimClient(fetchFn as typeof fetch).run(body)).resolves.toEqual(answer);
		expect(fetchFn).toHaveBeenCalledWith('/api/calc/weapon-sim', expect.objectContaining({ method: 'POST', body: JSON.stringify(body) }));
	});

	it('reports the server reason', async () => {
		const fetchFn = vi.fn(async () => new Response('rounds must be 1 to 50000', { status: 400 }));
		await expect(createWeaponSimClient(fetchFn as typeof fetch).run(weaponSimRequest(form(), {}))).rejects.toThrow('rounds must be 1 to 50000');
	});
});

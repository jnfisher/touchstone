// The Weapon Simulator (touchstone's own; MME has no such window): the request the form makes, the
// response's shape, and the grid's columns for the two views, regular rounds and backstab rounds.
import type { ColDef } from 'ag-grid-community';
import type { Monster } from './api';

/** Ability 34 on a monster: its dodge. */
const DODGE = 34;

export interface SimTarget {
	ac: number;
	dr: number;
	dodge: number;
	bs_defense: number;
	hp: number;
}

export type TargetField = keyof SimTarget;

export const TARGET_FIELDS: { key: TargetField; label: string; hint: string }[] = [
	{ key: 'ac', label: 'AC', hint: 'Armour class: lowers the chance to hit' },
	{ key: 'dr', label: 'DR', hint: 'Damage resistance: taken off every blow' },
	{ key: 'dodge', label: 'Dodge', hint: 'Dodge: blows that land are avoided this often' },
	{ key: 'bs_defense', label: 'BS def', hint: 'Backstab defense: lowers the chance a backstab lands' },
	{ key: 'hp', label: 'HP', hint: 'Hit points: with a value, the rounds to kill are simulated too' }
];

export interface WeaponSimForm {
	ignoreLevel: boolean;
	ignoreAlignment: boolean;
	includeLimited: boolean;
	/** The target boxes as typed. */
	target: Record<TargetField, string>;
	/** The monster the target boxes were filled from, if any. */
	monster: number | null;
	rounds: number;
	candidates: number;
	seed: number;
	pinned: number[];
}

export const ROUND_CHOICES = [1000, 5000, 20000] as const;
export const CANDIDATE_CHOICES = [10, 25, 50] as const;
export const MAX_PINNED = 20;

export const WEAPON_SIM_DEFAULTS: WeaponSimForm = {
	ignoreLevel: false,
	ignoreAlignment: false,
	includeLimited: true,
	target: { ac: '0', dr: '0', dodge: '0', bs_defense: '0', hp: '0' },
	monster: null,
	rounds: 5000,
	candidates: 25,
	seed: 1,
	pinned: []
};

/** A box's whole number, 0 for anything else; negative only where the server takes one (dodge). */
function whole(text: string, min: number): number {
	const n = Math.trunc(Number(text.trim()));
	return Number.isFinite(n) ? Math.min(Math.max(n, min), 1_000_000) : 0;
}

export function targetOf(form: WeaponSimForm): SimTarget {
	return {
		ac: whole(form.target.ac, 0),
		dr: whole(form.target.dr, 0),
		dodge: Math.min(whole(form.target.dodge, -100), 1000),
		bs_defense: whole(form.target.bs_defense, 0),
		hp: whole(form.target.hp, 0)
	};
}

/** The boxes a monster fills: its armour, damage resistance, dodge, backstab defense and hit points. */
export function targetFromMonster(m: Monster): Record<TargetField, string> {
	const dodge = m.abilities.find((a) => a.ability === DODGE && a.value > 0)?.value ?? 0;
	return {
		ac: String(m.armour_class),
		dr: String(m.damage_resist),
		dodge: String(dodge),
		bs_defense: String(m.bs_defense),
		hp: String(m.hp)
	};
}

/** `POST /api/calc/weapon-sim`; `character` is the evaluate body of the character in use. */
export function weaponSimRequest(form: WeaponSimForm, character: object) {
	return {
		character,
		ignore_level: form.ignoreLevel,
		ignore_alignment: form.ignoreAlignment,
		include_limited: form.includeLimited,
		target: targetOf(form),
		rounds: form.rounds,
		candidates: form.candidates,
		seed: form.seed,
		pinned: form.pinned.slice(0, MAX_PINNED)
	};
}

// ---------- the response ----------

export interface RoundStats {
	rounds: number;
	mean: number;
	sd: number;
	min: number;
	p10: number;
	median: number;
	p90: number;
	max: number;
	zero_pct: number;
}

export interface KillStats {
	mean: number;
	median: number;
	/** Nine fights in ten are over by this round. */
	p90: number;
	/** Too long a fight to roll: hit points over the damage a round. */
	estimated: boolean;
}

export interface SimMode {
	/** MME's damage a round for the same attack. */
	expected: number;
	sim: RoundStats;
	swings: number;
	hit: number;
	crit: number;
	min_dmg: number;
	max_dmg: number;
	avg_hit: number;
	avg_crit: number;
	max_crit: number;
	extra: number;
	detail: string;
	kill: KillStats | null;
}

export type ModeKey = 'attack' | 'bash' | 'backstab';
export const MODE_LABEL: Record<ModeKey, string> = { attack: 'Attack', bash: 'Bash', backstab: 'Backstab' };

export interface WeaponSimRow {
	number: number;
	name: string;
	level: number;
	weapon_type: number;
	two_handed: boolean;
	min: number;
	max: number;
	speed: number;
	str_req: number;
	accy: number;
	limit: number;
	equipped: boolean;
	pinned: boolean;
	needs_level: boolean;
	wrong_alignment: boolean;
	attack: SimMode | null;
	bash: SimMode | null;
	backstab: SimMode | null;
}

export interface WeaponSimResult {
	rows: WeaponSimRow[];
	usable: number;
	simulated: number;
	rounds: number;
	candidates: number;
	seed: number;
	target: SimTarget;
	can_backstab: boolean;
	equipped_weapon: number;
	level: number;
	elapsed_ms: number;
}

// ---------- reading a row ----------

export type SimView = 'regular' | 'backstab';

/** The better of attack and bash for a regular round, by simulated damage. */
export function bestRegular(r: WeaponSimRow): { key: 'attack' | 'bash'; mode: SimMode } | undefined {
	const a = r.attack;
	const b = r.bash;
	if (a && (!b || a.sim.mean >= b.sim.mean)) return { key: 'attack', mode: a };
	return b ? { key: 'bash', mode: b } : undefined;
}

/** The mode a view ranks a weapon by. */
export function viewMode(r: WeaponSimRow, view: SimView): SimMode | undefined {
	return view === 'backstab' ? (r.backstab ?? undefined) : bestRegular(r)?.mode;
}

/** The rows a view lists: the backstab view has only weapons that can backstab. */
export function viewRows(result: WeaponSimResult, view: SimView): WeaponSimRow[] {
	return view === 'backstab' ? result.rows.filter((r) => r.backstab) : result.rows;
}

/** Percent above or below the weapon in hand in this view; undefined without one to compare with. */
export function versusEquipped(r: WeaponSimRow, result: WeaponSimResult, view: SimView): number | undefined {
	const base = result.rows.find((x) => x.equipped);
	const mine = viewMode(r, view)?.sim.mean;
	const theirs = base && viewMode(base, view)?.sim.mean;
	if (mine === undefined || !theirs) return undefined;
	return ((mine - theirs) / theirs) * 100;
}

export function signedPct(n: number | undefined): string {
	if (n === undefined) return '';
	const r = Math.round(n);
	return r === 0 ? '0%' : `${r > 0 ? '+' : ''}${r}%`;
}

/** "2.4" for simulated fights, "~137" for a fight too long to roll (hit points over damage a round). */
export function killText(k: KillStats | null | undefined): string {
	if (!k) return '';
	return k.estimated ? `~${Math.round(k.mean)}` : k.mean.toFixed(1);
}

/** What stops the character wielding the weapon today, for the list's note column. */
export function rowNote(r: WeaponSimRow, level: number): string {
	const notes: string[] = [];
	if (r.equipped) notes.push('equipped');
	if (r.needs_level) notes.push(`level ${r.level}${r.level > level ? ` (+${r.level - level})` : ''}`);
	if (r.wrong_alignment) notes.push('alignment');
	if (r.limit !== 0) notes.push('limited');
	if (r.pinned && !r.equipped) notes.push('pinned');
	return notes.join(', ');
}

// ---------- the grid ----------

const num = { type: 'rightAligned', filter: 'agNumberColumnFilter' } as const;
const one = (n: number | undefined) => (n === undefined ? '' : n.toFixed(1));

/** Colours a "vs equipped" cell; the right-aligned class is repeated because `cellClass` replaces the type's. */
function versusClass(n: number | undefined): string[] {
	const cls = ['ag-right-aligned-cell'];
	if (n !== undefined && Math.round(n) > 0) cls.push('ts-cell-above');
	if (n !== undefined && Math.round(n) < 0) cls.push('ts-cell-below');
	return cls;
}

/**
 * The list's columns. Regular rounds rank by the better of attack and bash and show both; backstab rounds rank
 * by the opening round. Either way the list starts sorted by the view's simulated damage, best first.
 */
export function weaponSimColumns(view: SimView, result: WeaponSimResult): ColDef<WeaponSimRow>[] {
	const mode = (r: WeaponSimRow | undefined) => (r ? viewMode(r, view) : undefined);
	const hasHp = result.target.hp > 0;
	const cols: ColDef<WeaponSimRow>[] = [
		{ colId: 'name', headerName: 'Weapon', field: 'name', minWidth: 170, maxWidth: 260 },
		{
			colId: 'damage',
			headerName: view === 'backstab' ? 'Backstab' : 'Dmg/Rnd',
			headerTooltip:
				view === 'backstab'
					? 'Simulated damage of the opening round'
					: 'Simulated damage a round with the better of attack and bash',
			valueGetter: (p) => mode(p.data)?.sim.mean,
			valueFormatter: (p) => one(p.value as number | undefined),
			sort: 'desc',
			...num
		},
		{
			colId: 'versus',
			headerName: 'vs equipped',
			headerTooltip: 'Compared with the weapon in hand, in this view',
			valueGetter: (p) => (p.data ? versusEquipped(p.data, result, view) : undefined),
			valueFormatter: (p) => signedPct(p.value as number | undefined),
			cellClass: (p) => versusClass(p.value as number | undefined),
			...num
		}
	];
	if (view === 'regular') {
		cols.push(
			{
				colId: 'best',
				headerName: 'Best',
				headerTooltip: 'Which of attack and bash deals more',
				valueGetter: (p) => (p.data ? MODE_LABEL[bestRegular(p.data)?.key ?? 'attack'] : ''),
				filter: false
			},
			{
				colId: 'attack',
				headerName: 'Attack',
				valueGetter: (p) => p.data?.attack?.sim.mean,
				valueFormatter: (p) => one(p.value as number | undefined),
				...num
			},
			{
				colId: 'bash',
				headerName: 'Bash',
				valueGetter: (p) => p.data?.bash?.sim.mean,
				valueFormatter: (p) => one(p.value as number | undefined),
				...num
			},
			{
				colId: 'swings',
				headerName: 'Swings',
				headerTooltip: 'Swings a round of the better attack',
				valueGetter: (p) => mode(p.data)?.swings,
				valueFormatter: (p) => one(p.value as number | undefined),
				...num
			}
		);
	}
	cols.push(
		{
			colId: 'low',
			headerName: 'Low',
			headerTooltip: 'One round in ten deals this or less',
			valueGetter: (p) => mode(p.data)?.sim.p10,
			...num
		},
		{
			colId: 'high',
			headerName: 'High',
			headerTooltip: 'One round in ten deals this or more',
			valueGetter: (p) => mode(p.data)?.sim.p90,
			...num
		},
		{
			colId: 'hit',
			headerName: 'Hit %',
			valueGetter: (p) => mode(p.data)?.hit,
			...num
		}
	);
	if (view === 'regular') {
		cols.push({ colId: 'crit', headerName: 'Crit %', valueGetter: (p) => mode(p.data)?.crit, ...num });
	}
	if (hasHp) {
		cols.push({
			colId: 'kill',
			headerName: 'Kill rnds',
			headerTooltip:
				view === 'backstab'
					? 'Mean rounds to bring the target down: the backstab, then normal attacks'
					: 'Mean rounds to bring the target down with the better attack',
			valueGetter: (p) => mode(p.data)?.kill?.mean,
			valueFormatter: (p) => killText(mode(p.data)?.kill),
			...num
		});
	}
	cols.push(
		{ colId: 'level', headerName: 'Lvl', field: 'level', ...num },
		{
			colId: 'hands',
			headerName: 'Hands',
			valueGetter: (p) => (p.data?.two_handed ? '2H' : '1H'),
			filter: false
		},
		{
			colId: 'note',
			headerName: 'Note',
			valueGetter: (p) => (p.data ? rowNote(p.data, result.level) : ''),
			filter: false
		}
	);
	return cols;
}

// ---------- transport ----------

type FetchFn = typeof fetch;

export function createWeaponSimClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async run(body: ReturnType<typeof weaponSimRequest>): Promise<WeaponSimResult> {
			const res = await fetchFn('/api/calc/weapon-sim', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body)
			});
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new Error(reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as WeaponSimResult;
		}
	};
}

export const weaponSimClient = createWeaponSimClient();

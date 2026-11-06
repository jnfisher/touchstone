// The Monsters tab's filter controls (MME's frmMain boxes, `optMonsterFilter`, the lair/party frame
// and the "Monster Filters" window) and the pane's line colours. The server runs FilterMonsters and
// PullMonsterDetail; this module only holds the controls, keeps them sane and shapes the request.
//
// The values live in this browser's local storage like the other view options; every access is
// wrapped because storage can be blocked.

export const MONSTER_FILTER_KEY = 'touchstone.monsterfilter';

/** The lair/party frame: `txtMonsterLairFilter(0..9)` and the two damage-output boxes. */
export interface PartyBoxes {
	size: string;
	ac: string;
	dr: string;
	mr: string;
	dodge: string;
	hp: string;
	anti_magic: string;
	rest_hp: string;
	accuracy: string;
	swings: string;
	damage_out_physical: string;
	damage_out_magic: string;
}

/** The "Monster Filters" window. 9999 is MME's "no limit". */
export interface ExtraFilters {
	enabled: boolean;
	armour_class: number;
	damage_resist: number;
	magic_res: number;
	bs_def: number;
	game_limit: number;
	avg_lair_exp: number;
	atk_accuracy_maj: number;
	atk_accuracy_max: number;
	dodge: number;
	num_lairs: number;
	num_mobs_lte: number;
	num_mobs_gte: number;
	follow_gte: number;
	follow_lte: number;
	at_least_lairs: number;
	at_least_lairs_on: boolean;
	/** Three rows of [ability number, operator (0 at most, 1 at least), value]. */
	abilities: [number, number, number][];
	is_undead: boolean;
	non_hostile_vs_evil: boolean;
	non_hostile_vs_ng: boolean;
	reachable_only: boolean;
	drops_cash: boolean;
	drops_r: boolean;
	drops_p: boolean;
	drops_g: boolean;
	drops_s: boolean;
	atk_no_poison: boolean;
	atk_no_confusion: boolean;
	atk_no_fear: boolean;
	show_all: boolean;
}

export interface MonsterControls {
	/** `optMonsterFilter(1)`: by lair (a party when "# Party" is above one) instead of by mob. */
	lair: boolean;
	/** `cmbMonsterRegen`: 0 regen at most the box, 1 at least. */
	regen_op: 0 | 1;
	regen: string;
	hp: string;
	/** A trailing K or M multiplies. */
	exp: string;
	damage: string;
	magic: boolean;
	magic_value: string;
	party: PartyBoxes;
	extras: ExtraFilters;
	/** Request only (never stored): let the list ask whether to calculate the monster damage first. */
	ask_damage?: boolean;
}

export const NO_LIMIT = 9999;

export const DEFAULT_PARTY: PartyBoxes = {
	size: '1',
	ac: '60',
	dr: '20',
	mr: '50',
	dodge: '50',
	hp: '400',
	anti_magic: '0',
	rest_hp: '12',
	accuracy: '250',
	swings: '1',
	damage_out_physical: '99999',
	damage_out_magic: '99999'
};

export const DEFAULT_EXTRAS: ExtraFilters = {
	enabled: false,
	armour_class: NO_LIMIT,
	damage_resist: NO_LIMIT,
	magic_res: NO_LIMIT,
	bs_def: NO_LIMIT,
	game_limit: NO_LIMIT,
	avg_lair_exp: 0,
	atk_accuracy_maj: NO_LIMIT,
	atk_accuracy_max: NO_LIMIT,
	dodge: NO_LIMIT,
	num_lairs: 0,
	num_mobs_lte: NO_LIMIT,
	num_mobs_gte: 0,
	follow_gte: 0,
	follow_lte: NO_LIMIT,
	at_least_lairs: 1,
	at_least_lairs_on: false,
	abilities: [
		[0, 1, 1],
		[0, 1, 1],
		[0, 1, 1]
	],
	is_undead: false,
	non_hostile_vs_evil: false,
	non_hostile_vs_ng: false,
	reachable_only: false,
	drops_cash: false,
	drops_r: false,
	drops_p: false,
	drops_g: false,
	drops_s: false,
	atk_no_poison: false,
	atk_no_confusion: false,
	atk_no_fear: false,
	show_all: false
};

/** What `RemoveFilters` writes into the boxes. */
export const DEFAULT_CONTROLS: MonsterControls = {
	lair: false,
	regen_op: 0,
	regen: '999',
	hp: '99999',
	exp: '0',
	damage: '99999',
	magic: false,
	magic_value: '999',
	party: DEFAULT_PARTY,
	extras: DEFAULT_EXTRAS
};

export function freshControls(): MonsterControls {
	return structuredClone(DEFAULT_CONTROLS);
}

const text = (v: unknown, fallback: string, max = 14) =>
	typeof v === 'string' ? v.slice(0, max) : typeof v === 'number' && Number.isFinite(v) ? String(v) : fallback;
const flag = (v: unknown, fallback: boolean) => (typeof v === 'boolean' ? v : fallback);
const whole = (v: unknown, fallback: number) =>
	typeof v === 'number' && Number.isFinite(v) ? Math.trunc(Math.min(1e9, Math.max(-1e9, v))) : fallback;
const real = (v: unknown, fallback: number) =>
	typeof v === 'number' && Number.isFinite(v) ? Math.min(1e9, Math.max(-1e9, v)) : fallback;

function sanitizeParty(v: unknown): PartyBoxes {
	const o = (typeof v === 'object' && v !== null ? v : {}) as Record<string, unknown>;
	const out = { ...DEFAULT_PARTY };
	for (const k of Object.keys(out) as (keyof PartyBoxes)[]) out[k] = text(o[k], DEFAULT_PARTY[k], 6);
	return out;
}

function sanitizeExtras(v: unknown): ExtraFilters {
	const o = (typeof v === 'object' && v !== null ? v : {}) as Record<string, unknown>;
	const d = DEFAULT_EXTRAS;
	const out = structuredClone(d);
	for (const k of Object.keys(d) as (keyof ExtraFilters)[]) {
		const dv = d[k];
		if (k === 'abilities') continue;
		if (typeof dv === 'boolean') (out[k] as boolean) = flag(o[k], dv);
		else if (k === 'avg_lair_exp' || k === 'num_mobs_lte' || k === 'num_mobs_gte') (out[k] as number) = real(o[k], dv as number);
		else (out[k] as number) = whole(o[k], dv as number);
	}
	if (Array.isArray(o.abilities)) {
		out.abilities = d.abilities.map((row, i) => {
			const r = Array.isArray(o.abilities) ? (o.abilities[i] as unknown) : undefined;
			return Array.isArray(r) ? [whole(r[0], row[0]), whole(r[1], row[1]) === 0 ? 0 : 1, whole(r[2], row[2])] : [...row];
		}) as ExtraFilters['abilities'];
	}
	return out;
}

/** A damaged or foreign value falls back to the default for that field. */
export function sanitize(v: unknown): MonsterControls {
	const o = (typeof v === 'object' && v !== null ? v : {}) as Record<string, unknown>;
	const d = DEFAULT_CONTROLS;
	return {
		lair: flag(o.lair, d.lair),
		regen_op: o.regen_op === 1 ? 1 : 0,
		regen: text(o.regen, d.regen, 4),
		hp: text(o.hp, d.hp),
		exp: text(o.exp, d.exp),
		damage: text(o.damage, d.damage),
		magic: flag(o.magic, d.magic),
		magic_value: text(o.magic_value, d.magic_value, 6),
		party: sanitizeParty(o.party),
		extras: sanitizeExtras(o.extras)
	};
}

export function readControls(storage: Pick<Storage, 'getItem'> | undefined): MonsterControls {
	try {
		const raw = storage?.getItem(MONSTER_FILTER_KEY);
		return raw ? sanitize(JSON.parse(raw)) : freshControls();
	} catch {
		return freshControls();
	}
}

export function writeControls(storage: Pick<Storage, 'setItem'> | undefined, c: MonsterControls): void {
	try {
		storage?.setItem(MONSTER_FILTER_KEY, JSON.stringify(c));
	} catch {
		// Private window or blocked storage: the boxes last for this page only.
	}
}

/** Whether any filter box differs from what `RemoveFilters` leaves (the "*Monsters*" state). */
export function isFiltering(c: MonsterControls): boolean {
	const d = DEFAULT_CONTROLS;
	const blank = (s: string) => s.trim() === '';
	const same = (a: string, b: string) => blank(a) || a.trim() === b;
	if (c.lair) return true;
	if (!same(c.regen, d.regen) || !same(c.hp, d.hp) || !same(c.exp, d.exp) || !same(c.damage, d.damage)) return true;
	if (c.magic) return true;
	return c.extras.enabled;
}

// ---------- pane line colours ----------

/** The VB `ForeColor` values (BGR integers) MME gives detail lines and list cells, as theme classes. */
const COLOR_CLASS = new Map<number, string>([
	// reds: the stored (not character-specific) damage, hard-coded warnings
	[204, 'text-destructive'],
	[0xc0, 'text-destructive'],
	[0xff, 'text-destructive'],
	// green: greeting commands, good news
	[0x8000, 'text-positive'],
	// blue: BS defense and abilities
	[0xcc6600, 'text-info'],
	// orange and brown: scripting figures
	[0x80ff, 'text-warning'],
	[0x40c0, 'text-warning'],
	// purple: guards, and everything calculated against this character or party
	[0x800080, 'text-violet'],
	[144 + 4 * 256 + 214 * 65536, 'text-violet'],
	[193 + 232 * 65536, 'text-violet'],
	// grey: a monster that does not match the filter
	[192 + 192 * 256 + 192 * 65536, 'text-muted-foreground']
]);

/** Theme class for a pane line colour; colours MME does not use are left to the pane. */
export function lineColorClass(color: number | undefined | null): string {
	if (color === undefined || color === null) return '';
	if (color < 0) return 'text-muted-foreground';
	return COLOR_CLASS.get(color) ?? '';
}

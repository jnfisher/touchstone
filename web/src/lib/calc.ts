// The four calculator windows (frmExpCalc, frmCoinConvert, frmSwingCalc, frmBSCalc): their
// defaults, the mapping from form state to the `POST /api/calc/*` bodies, and the response shapes.
import type { Item } from './api';

export const CALCULATORS = [
	{ slug: 'exp', label: 'Exp Calculator' },
	{ slug: 'coin', label: 'Coin Converter' },
	{ slug: 'swing', label: 'Swing Calculator' },
	{ slug: 'backstab', label: 'Backstab Calculator' }
] as const;
export type CalcSlug = (typeof CALCULATORS)[number]['slug'];

export function isCalc(s: string): s is CalcSlug {
	return CALCULATORS.some((c) => c.slug === s);
}

// ---------- responses ----------

interface Failable {
	/** VB run-time error number MME's handler would have caught, with the partial result kept. */
	error: number | null;
	error_text: string | null;
}

export interface ExpCalcRow {
	level: number;
	exp: number;
	exp_text: string;
	needed: number;
	needed_text: string;
}
export interface ExpCalcResult extends Failable {
	/** The exp table number; null when MME's Integer sum overflowed. */
	table: number | null;
	from: number;
	to: number;
	rows: ExpCalcRow[];
}

export interface CoinResult {
	destination: 0 | 1;
	destination_text: string;
	/** Copper value of the destination's coin. */
	current_coin: number;
	weights: [string, string];
}

export interface SwingResult extends Failable {
	qnd: string;
	energy: string;
	encum: string;
	encum_pct: number | null;
	raw_swing: string;
	true_avg: string;
	eu: string[];
	swings: string[];
	damage: string;
	damage_caption: string;
	speed: string;
	max_encum: string;
}

export interface BackstabResult extends Failable {
	damage: string;
	min_dmg: number;
	max_dmg: number;
	max_adj: string;
	bs_min_adj: string;
	bs_max_adj: string;
	stealth_adj: string;
	bs_min: string;
	bs_max: string;
	level: string;
	stealth: string;
	max_dmg_box: string;
}

// ---------- coins ----------

export const COINS = [
	{ name: 'Copper', copper: 1 },
	{ name: 'Silver', copper: 10 },
	{ name: 'Gold', copper: 100 },
	{ name: 'Platinum', copper: 10_000 },
	{ name: 'Runic', copper: 1_000_000 }
] as const;

/** The coin button index for a copper value (`eCoins`); copper when it is not a coin. */
export function coinIndex(copper: number): number {
	const i = COINS.findIndex((c) => c.copper === copper);
	return i < 0 ? 0 : i;
}

export interface CoinForm {
	/** Amounts as typed, top box then bottom box. */
	text: [string, string];
	/** Selected coin button of each row. */
	coin: [number, number];
}

/** Form_Load: one gold on top, copper below. */
export const COIN_DEFAULTS: CoinForm = { text: ['1', '100'], coin: [2, 0] };

/** Typing in a box converts into the other one. */
export function coinTypedRequest(form: CoinForm, box: 0 | 1) {
	return {
		source: box,
		text: form.text[box],
		top: form.coin[0],
		bottom: form.coin[1]
	};
}

/**
 * Clicking a coin button converts that row's amount from the coin it was in (`eCurrentCoin`) to
 * the new one, in the same box.
 */
export function coinButtonRequest(form: CoinForm, box: 0 | 1, previousCoin: number) {
	return {
		source: box,
		destination: box,
		text: form.text[box],
		top: form.coin[0],
		bottom: form.coin[1],
		force_source: COINS[previousCoin]?.copper ?? 1
	};
}

// ---------- exp ----------

export interface ExpForm {
	/** Class and race record numbers; 0 is the combo's "custom" entry. */
	classNumber: number;
	raceNumber: number;
	/** The exp table box when typed over; null follows class and race. */
	table: string | null;
	from: string;
	to: string;
}

export const EXP_DEFAULTS: ExpForm = {
	classNumber: 0,
	raceNumber: 0,
	table: null,
	from: '2',
	to: '255'
};

/** A whole number, or undefined while the box is empty or not a number. */
function wholeNumber(s: string): number | undefined {
	const t = s.trim();
	if (!/^-?\d+$/.test(t)) return undefined;
	return Number(t);
}

export function expRequest(form: ExpForm) {
	const typed = form.table === null ? undefined : wholeNumber(form.table);
	return {
		class: form.classNumber,
		race: form.raceNumber,
		...(typed === undefined ? {} : { table: typed }),
		from: form.from,
		to: form.to
	};
}

// ---------- weapons ----------

export interface WeaponChoice {
	number: number;
	name: string;
	label: string;
}

/** The ability that gives a weapon a backstab (BS accuracy). */
export const BS_ACCURACY = 116;

/**
 * The weapon combo's rows: in-game weapons (item type 1), sorted by name as MME's combos are, and
 * for the backstab calculator only those with BS accuracy.
 */
export function weaponChoices(items: Item[], opts: { backstabOnly?: boolean } = {}): WeaponChoice[] {
	return items
		.filter(
			(i) =>
				i.item_type === 1 &&
				i.in_game &&
				(!opts.backstabOnly || i.abilities.some((a) => a.ability === BS_ACCURACY))
		)
		.map((i) => ({ number: i.number, name: i.name, label: `${i.name} (${i.number})` }))
		.sort((a, b) => a.name.localeCompare(b.name) || a.number - b.number);
}

/** Case-insensitive match on the label, every word of the query in order of appearance ignored. */
export function filterWeapons(list: WeaponChoice[], query: string): WeaponChoice[] {
	const words = query.toLowerCase().split(/\s+/).filter(Boolean);
	if (words.length === 0) return list;
	return list.filter((w) => {
		const l = w.label.toLowerCase();
		return words.every((x) => l.includes(x));
	});
}

// ---------- swing ----------

export const COMBAT_LEVELS = [
	{ value: 1, label: '1 (Poor)' },
	{ value: 2, label: '2 (Fair)' },
	{ value: 3, label: '3 (Average)' },
	{ value: 4, label: '4 (Good)' },
	{ value: 5, label: '5 (Excellent)' }
] as const;

export const SPEED_OPTIONS = [
	{ value: 0, label: 'Fast (85)' },
	{ value: 1, label: 'Normal (100)' },
	{ value: 2, label: 'Slow (125)' },
	{ value: 3, label: 'Custom' }
] as const;

export interface SwingForm {
	weapon: number | null;
	combat: number;
	slowness: boolean;
	bashing: boolean;
	level: string;
	agility: string;
	strength: string;
	encum: string;
	maxEncum: string;
	/** Max Enc. follows strength, as MME's strength box rewrites it on every change. */
	maxEncumAuto: boolean;
	speed: string;
	speedOption: number;
}

/** frmSwingCalc's design-time values; Max Enc. is 100 strength's capacity. */
export const SWING_DEFAULTS: SwingForm = {
	weapon: null,
	combat: 3,
	slowness: false,
	bashing: false,
	level: '60',
	agility: '100',
	strength: '100',
	encum: '1000',
	maxEncum: '4800',
	maxEncumAuto: true,
	speed: '100',
	speedOption: 1
};

export function swingRequest(f: SwingForm) {
	return {
		weapon: f.weapon,
		combat: f.combat,
		// optSpeed(2) ticks the Slowness box and every other speed option clears it.
		slowness: f.slowness,
		bashing: f.bashing,
		level: f.level,
		agility: f.agility,
		strength: f.strength,
		encum: f.encum,
		max_encum: f.maxEncum,
		max_encum_auto: f.maxEncumAuto,
		speed: f.speed,
		speed_option: f.speedOption
	};
}

/** Choosing a speed option sets Slowness the way `optSpeed_Click` does. */
export function withSpeedOption(f: SwingForm, option: number): SwingForm {
	return { ...f, speedOption: option, slowness: option === 2 };
}

// ---------- backstab ----------

export interface BackstabForm {
	weapon: number | null;
	level: string;
	stealth: string;
	strength: string;
	bsMin: string;
	bsMax: string;
	maxDmg: string;
	classStealth: boolean;
}

/** frmBSCalc's design-time values. */
export const BACKSTAB_DEFAULTS: BackstabForm = {
	weapon: null,
	level: '50',
	stealth: '100',
	strength: '100',
	bsMin: '0',
	bsMax: '0',
	maxDmg: '0',
	classStealth: false
};

export function backstabRequest(f: BackstabForm) {
	return {
		weapon: f.weapon,
		level: f.level,
		stealth: f.stealth,
		strength: f.strength,
		bs_min: f.bsMin,
		bs_max: f.bsMax,
		max_dmg: f.maxDmg,
		class_stealth: f.classStealth
	};
}

// ---------- transport ----------

type FetchFn = typeof fetch;

export class CalcError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
	}
}

/** `POST /api/calc/<name>`; a 400 carries the server's reason as the error message. */
export function createCalcClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function post<T>(name: CalcSlug, body: unknown): Promise<T> {
		const res = await fetchFn(`/api/calc/${name}`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(body)
		});
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new CalcError(res.status, reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as T;
	}
	return {
		exp: (body: ReturnType<typeof expRequest>) => post<ExpCalcResult>('exp', body),
		coin: (body: object) => post<CoinResult>('coin', body),
		swing: (body: ReturnType<typeof swingRequest>) => post<SwingResult>('swing', body),
		backstab: (body: ReturnType<typeof backstabRequest>) => post<BackstabResult>('backstab', body)
	};
}

export const calcClient = createCalcClient();

/** "Energy per swing: 58" -> "58": the part of a result caption after its label. */
export function afterLabel(caption: string): string {
	const i = caption.indexOf(': ');
	return i < 0 ? caption : caption.slice(i + 2);
}

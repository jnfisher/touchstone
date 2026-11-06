// The six calculator windows (frmExpCalc, frmCoinConvert, frmSwingCalc, frmBSCalc, frmHitCalc,
// frmMonsterAttackSim): their
// defaults, the mapping from form state to the `POST /api/calc/*` bodies, and the response shapes.
import type { ClassRec, Item, Monster } from './api';
import { WEAPON_SLOT, type CharacterDoc, type EvalState } from './character';

export const CALCULATORS = [
	{ slug: 'exp', label: 'Exp Calculator' },
	{ slug: 'coin', label: 'Coin Converter' },
	{ slug: 'swing', label: 'Swing Calculator' },
	{ slug: 'backstab', label: 'Backstab Calculator' },
	{ slug: 'hit', label: 'Hit Calculator' },
	{ slug: 'monster-sim', label: 'Monster Attack Simulator' }
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
	charm: CharmButton;
}

/** `cmdCharm`: its caption (CRLF between lines) and the percent of the amount it leaves. */
export interface CharmButton {
	enabled: boolean;
	caption: string;
	percent: number | null;
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
	/** `txtTrueAVG(7)`; null when a box did not fit a Long. */
	true_average: string | null;
	true_average_error: string | null;
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
export function weaponChoices(
	items: Item[],
	opts: { backstabOnly?: boolean } = {}
): WeaponChoice[] {
	return items
		.filter(
			(i) =>
				i.item_type === 1 &&
				i.in_game &&
				(!opts.backstabOnly || i.abilities.some((a) => a.ability === BS_ACCURACY))
		)
		.map((i) => ({
			number: i.number,
			name: i.name,
			label: `${i.name} (${i.number})`
		}))
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
	/** `txtTrueAVG(0..5)`: the MegaMUD Hit, Extra and Crit percentages and averages. */
	mega: MegaBoxes;
}

export interface MegaBoxes {
	hitPct: string;
	hitAvg: string;
	extraPct: string;
	extraAvg: string;
	critPct: string;
	critAvg: string;
}

export const MEGA_EMPTY: MegaBoxes = {
	hitPct: '',
	hitAvg: '',
	extraPct: '',
	extraAvg: '',
	critPct: '',
	critAvg: ''
};

/** The six strings `cmdPasteMega_Click` writes, in box order, as the form's boxes. */
export function megaFromFields(f: readonly string[]): MegaBoxes {
	return {
		hitPct: f[0] ?? '',
		hitAvg: f[1] ?? '',
		extraPct: f[2] ?? '',
		extraAvg: f[3] ?? '',
		critPct: f[4] ?? '',
		critAvg: f[5] ?? ''
	};
}

export interface MegaResult {
	ok: boolean;
	fields: string[] | null;
	/** The message box MME would show. */
	message: string | null;
	error: number | null;
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
	speedOption: 1,
	mega: MEGA_EMPTY
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
		speed_option: f.speedOption,
		hit_pct: f.mega.hitPct,
		hit_avg: f.mega.hitAvg,
		extra_pct: f.mega.extraPct,
		extra_avg: f.mega.extraAvg,
		crit_pct: f.mega.critPct,
		crit_avg: f.mega.critAvg
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

// ---------- hit ----------

export type HitType = 'normal' | 'backstab';
export type HitAttacker = 'character' | 'monster' | 'manual';
export type HitDefender = 'character' | 'monster' | 'player' | 'manual';
export type HitStrike = 'normal' | 'bash' | 'smash' | 'kick' | 'jumpkick';

/** Which handler MME would run: a box changed, an option clicked, a monster picked, an arrow. */
export type HitEvent =
	| { event: 'calc' }
	| { event: 'refresh' }
	| { event: 'monster' }
	| { event: 'modify'; index: number };

export interface HitEnabled {
	prot_evil: boolean;
	vile_ward: boolean;
	percep: boolean;
	shadow: boolean;
	see_hidden: boolean;
	monster_list: boolean;
}

/** The window's module variables; the page hands back what the last answer said. */
export interface HitState {
	monster_accy: number;
	monster_ac: number;
	monster_dodge: number;
	monster_bs_defense: number;
	monster_evil: boolean;
	monster_see_hidden: boolean;
	defender_ac: number;
	defender_dodge: number;
	defender_perception: number;
	defender_prot_evil: number;
	defender_vile_ward: number;
	defender_evilness: number;
	defender_shadow: boolean;
	char_perception: number;
}

/** Stand-in for the loaded character until the Character tab exists: plain boxes. */
export interface HitCharacter {
	accuracy: string;
	ac: string;
	dodge: string;
	protEvil: string;
	stealth: string;
	bsAccuracy: string;
	agility: string;
	shadowAc: string;
	classNumber: number | null;
	bsWeapon: number | null;
	strike: HitStrike;
	accyAdj: string;
	/** `txtCharStats(0).Tag` and the level box, which the backstab accuracy reads. */
	strength: string;
	level: string;
	/** `nGlobalCharAccyAbils`, `...Other`, `...Items`, vile ward and the equipped weapon's numbers. */
	accyAbilities: string;
	accyOther: string;
	accyItems: string;
	vileWard: string;
	weaponNumber: string;
	weaponAccy: string;
	weaponBsAccy: string;
}

export interface HitForm {
	calcType: HitType;
	attacker: HitAttacker;
	defender: HitDefender;
	monster: number | null;
	/** `txtHitCalc(0..5)`: accuracy, AC, dodge, protection from evil, vile ward, perception. */
	text: string[];
	shadow: boolean;
	seeHidden: boolean;
	evilIndex: number;
	/** Null until the first answer: the server works the flags out. */
	enabled: HitEnabled | null;
	state: HitState;
	character: HitCharacter;
}

export interface HitResult {
	calc_type: HitType;
	attacker: HitAttacker;
	defender: HitDefender;
	attacker_monster_enabled: boolean;
	text: string[];
	shadow: boolean;
	see_hidden: boolean;
	evil_index: number;
	enabled: HitEnabled;
	bs_label: string;
	main_text: string;
	sub_left: string;
	sub_right: string;
	sub_right_bold: boolean;
	hit: number | null;
	dodge: number | null;
	overall_hit: number | null;
	state: HitState;
	errors: { source: string; number: number; text: string }[];
}

const NO_HIT_STATE: HitState = {
	monster_accy: 0,
	monster_ac: 0,
	monster_dodge: 0,
	monster_bs_defense: 0,
	monster_evil: false,
	monster_see_hidden: false,
	defender_ac: 0,
	defender_dodge: 0,
	defender_perception: 0,
	defender_prot_evil: 0,
	defender_vile_ward: 0,
	defender_evilness: 0,
	defender_shadow: false,
	char_perception: 0
};

/** frmHitCalc's design-time values; the character boxes read 0 as MME's empty tags do. */
export const HIT_DEFAULTS: HitForm = {
	calcType: 'normal',
	attacker: 'character',
	defender: 'character',
	monster: null,
	text: ['100', '0', '10', '0', '0', '0'],
	shadow: false,
	seeHidden: false,
	evilIndex: 0,
	enabled: null,
	state: NO_HIT_STATE,
	character: {
		accuracy: '0',
		ac: '0',
		dodge: '0',
		protEvil: '0',
		stealth: '0',
		bsAccuracy: '0',
		agility: '0',
		shadowAc: '0',
		classNumber: null,
		bsWeapon: null,
		strike: 'normal',
		accyAdj: '0',
		strength: '0',
		level: '0',
		accyAbilities: '0',
		accyOther: '0',
		accyItems: '0',
		vileWard: '0',
		weaponNumber: '0',
		weaponAccy: '0',
		weaponBsAccy: '0'
	}
};

/** The arrow buttons' order in MME: accuracy, AC, dodge, protection, vile ward, perception. */
export const HIT_BOXES = [
	{ index: 0, label: 'Attacker Accuracy' },
	{ index: 1, label: 'vs AC' },
	{ index: 2, label: 'vs Dodge' },
	{ index: 3, label: 'vs Prot. Evil' },
	{ index: 5, label: 'vs Perception' }
] as const;

export const HIT_STRIKES: { value: HitStrike; label: string }[] = [
	{ value: 'normal', label: 'Normal' },
	{ value: 'bash', label: 'Bash' },
	{ value: 'smash', label: 'Smash' },
	{ value: 'kick', label: 'Kick' },
	{ value: 'jumpkick', label: 'Jumpkick' }
];

function wholeOrZero(s: string): number {
	return wholeNumber(s) ?? 0;
}

export function hitRequest(f: HitForm, ev: HitEvent) {
	const c = f.character;
	return {
		...ev,
		calc_type: f.calcType,
		attacker: f.attacker,
		defender: f.defender,
		...(f.monster === null ? {} : { monster: f.monster }),
		text: f.text,
		shadow: f.shadow,
		see_hidden: f.seeHidden,
		evil_index: f.evilIndex,
		...(f.enabled === null ? {} : { enabled: f.enabled }),
		state: f.state,
		character: {
			accuracy: c.accuracy,
			ac: c.ac,
			dodge: c.dodge,
			prot_evil: c.protEvil,
			stealth: c.stealth,
			bs_accuracy: c.bsAccuracy,
			agility: c.agility,
			shadow_ac: wholeOrZero(c.shadowAc),
			...(c.classNumber === null ? {} : { class: c.classNumber }),
			...(c.bsWeapon === null ? {} : { bs_weapon: c.bsWeapon }),
			strike: c.strike,
			accy_adj: wholeOrZero(c.accyAdj),
			strength: c.strength,
			level: c.level,
			accy_abilities: wholeOrZero(c.accyAbilities),
			accy_other: wholeOrZero(c.accyOther),
			accy_items: wholeOrZero(c.accyItems),
			vile_ward: wholeOrZero(c.vileWard),
			weapon_number: wholeOrZero(c.weaponNumber),
			weapon_accy: wholeOrZero(c.weaponAccy),
			weapon_bs_accy: wholeOrZero(c.weaponBsAccy)
		}
	};
}

/**
 * What an answer changes in the form. A `calc` answer never rewrites the boxes (the person may
 * have typed since the request left); the others do, as SetHitCalcVals and the arrows write them.
 */
export function applyHit(f: HitForm, r: HitResult, ev: HitEvent): HitForm {
	const out: HitForm = {
		...f,
		attacker: r.attacker,
		enabled: r.enabled,
		state: r.state
	};
	if (ev.event !== 'calc') {
		out.text = r.text;
		out.shadow = r.shadow;
		out.seeHidden = r.see_hidden;
		out.evilIndex = r.evil_index;
	}
	return out;
}

/** MME's `LoadMonsters`: named monsters in the table's order, "name (number)". */
export type MonsterChoice = WeaponChoice;
export function monsterChoices(monsters: Monster[]): MonsterChoice[] {
	return monsters
		.filter((m) => m.in_game && m.name && !m.name.startsWith('sdf'))
		.map((m) => ({
			number: m.number,
			name: m.name ?? '',
			label: `${m.name} (${m.number})`
		}));
}

/** The lines of a CRLF caption. */
export function captionLines(s: string): string[] {
	return s === '' ? [] : s.split(/\r?\n/);
}

// ---------- the current character ----------

/**
 * The numbers the calculator windows copy from the main form (`txtCharStats(i).Tag`,
 * `lblInvenCharStat(i)`, `nEquippedItem(16)`, ...), taken from the Character tab's evaluation. Text
 * boxes are strings because MME reads them with `Val`.
 */
export interface CalcCharacter {
	classNumber: number;
	/** `GetClassCombat`: 1..5, 3 when the class gives nothing else. */
	combat: number;
	classStealth: boolean;
	globalFilter: boolean;
	level: string;
	strength: string;
	agility: string;
	charm: number;
	/** `lblInvenCharStat(0)` and `(1)`. */
	encum: string;
	maxEncum: string;
	maxDamage: string;
	stealth: string;
	bsMin: string;
	bsMax: string;
	accuracy: string;
	ac: string;
	dodge: string;
	protEvil: string;
	bsAccuracy: string;
	/** The equipped weapon (`nEquippedItem(16)`); null when the slot is empty. */
	weapon: number | null;
	/** The weapon the backstab uses: the chosen one when Backstab is on, else the equipped one. */
	bsWeapon: number | null;
	strike: HitStrike;
	accyAdj: number;
	shadowAc: number;
	accyAbilities: number;
	accyOther: number;
	accyItems: number;
	vileWard: number;
	weaponAccy: number;
	weaponBsAccy: number;
	weaponNumber: number;
}

/** A number as MME's `Val` reads a caption: thousands separators dropped, junk is 0. */
export function captionNumber(s: string | undefined): number {
	const m = /^\s*(-?\d+(?:\.\d+)?)/.exec((s ?? '').replace(/,/g, ''));
	return m ? Number(m[1]) : 0;
}

const box = (n: number) => String(n);

/** Port of GetClassCombat: `CombatLVL - 2`, 1 for no class. */
export function classCombat(c: ClassRec | undefined): number {
	return c ? c.combat_lvl - 2 : 1;
}

/** Port of GetClassStealth: the class has ability 103. */
export function classHasStealth(c: ClassRec | undefined): boolean {
	return c?.abilities.some((a) => a.ability === 103) ?? false;
}

/** The attack choice as the Hit Calculator's strike list names it. */
export function strikeOf(attackType: number, martialArt: number): HitStrike {
	if (attackType === 6) return 'bash';
	if (attackType === 7) return 'smash';
	if (attackType === 4 && martialArt === 2) return 'kick';
	if (attackType === 4 && martialArt === 3) return 'jumpkick';
	return 'normal';
}

/**
 * What the calculators take from the loaded character, or undefined while no character is
 * evaluated (no class or race yet, or the server has not answered).
 */
export function calcCharacter(
	doc: CharacterDoc,
	ev: EvalState | undefined,
	classes: ClassRec[]
): CalcCharacter | undefined {
	if (!ev || doc.class === null || doc.race === null) return undefined;
	const cls = classes.find((c) => c.number === doc.class);
	const tag = (i: number) => ev.calc?.stat_tags[i] ?? 0;
	const stat = (i: number) => box(captionNumber(ev.stats[i]));
	const combat = classCombat(cls);
	const weapon = doc.equipped[WEAPON_SLOT] > 0 ? doc.equipped[WEAPON_SLOT] : null;
	const chosen = doc.attack.backstab && doc.attack.backstab_weapon > 0 ? doc.attack.backstab_weapon : null;
	return {
		classNumber: doc.class,
		combat: combat >= 1 && combat <= 5 ? combat : 3,
		classStealth: classHasStealth(cls),
		globalFilter: doc.global_filter,
		level: box(doc.level),
		strength: box(ev.stat_values[0]),
		agility: box(ev.stat_values[3]),
		charm: ev.stat_values[5],
		encum: stat(0),
		maxEncum: stat(1),
		maxDamage: stat(11),
		stealth: stat(19),
		bsMin: stat(14),
		bsMax: stat(15),
		accuracy: box(tag(10)),
		ac: box(Math.trunc(tag(2))),
		dodge: box(tag(8)),
		protEvil: box(tag(20)),
		bsAccuracy: box(tag(13)),
		weapon,
		bsWeapon: chosen ?? weapon,
		strike: strikeOf(doc.attack.attack_type, doc.attack.martial_art),
		accyAdj: ev.calc?.attack_accy_adj ?? 0,
		shadowAc: ev.calc?.shadow_ac ?? 0,
		accyAbilities: ev.calc?.accy_abilities ?? 0,
		accyOther: ev.calc?.accy_other ?? 0,
		accyItems: ev.calc?.accy_items ?? 0,
		vileWard: ev.calc?.vile_ward ?? 0,
		weaponAccy: ev.calc?.weapon_accy ?? 0,
		weaponBsAccy: ev.calc?.weapon_bs_accy ?? 0,
		weaponNumber: ev.calc?.weapon_number ?? 0
	};
}

/** A box MME only overwrites when the character's value is above 0; otherwise it keeps its text. */
const positive = (value: string, keep: string) => (captionNumber(value) > 0 ? value : keep);

/** frmSwingCalc `Form_Load`: combat from the class, the stat boxes and the equipped weapon. */
export function swingFromCharacter(c: CalcCharacter, f: SwingForm): SwingForm {
	return {
		...f,
		combat: c.combat,
		agility: positive(c.agility, f.agility),
		level: positive(c.level, f.level),
		strength: positive(c.strength, f.strength),
		encum: positive(c.encum, f.encum),
		// The strength box rewrites Max Enc. first and the character's own value follows it.
		maxEncum: positive(c.maxEncum, f.maxEncum),
		maxEncumAuto: captionNumber(c.maxEncum) > 0 ? false : f.maxEncumAuto,
		weapon: c.weapon ?? f.weapon
	};
}

/** frmBSCalc `Form_Load`. */
export function backstabFromCharacter(c: CalcCharacter, f: BackstabForm): BackstabForm {
	const nonZero = (value: string, keep: string) => (captionNumber(value) !== 0 ? value : keep);
	return {
		...f,
		strength: positive(c.strength, f.strength),
		level: positive(c.level, f.level),
		maxDmg: nonZero(c.maxDamage, f.maxDmg),
		stealth: nonZero(c.stealth, f.stealth),
		classStealth: c.classStealth,
		bsMin: nonZero(c.bsMin, f.bsMin),
		bsMax: nonZero(c.bsMax, f.bsMax),
		weapon: c.weapon ?? f.weapon
	};
}

/** The Hit Calculator's character section (SetHitCalcVals reads these from the main form). */
export function hitFromCharacter(c: CalcCharacter, f: HitCharacter): HitCharacter {
	return {
		...f,
		accuracy: c.accuracy,
		ac: c.ac,
		dodge: c.dodge,
		protEvil: c.protEvil,
		stealth: c.stealth,
		bsAccuracy: c.bsAccuracy,
		agility: c.agility,
		shadowAc: String(c.shadowAc),
		classNumber: c.classNumber,
		bsWeapon: c.bsWeapon,
		strike: c.strike,
		accyAdj: String(c.accyAdj),
		strength: c.strength,
		level: c.level,
		accyAbilities: String(c.accyAbilities),
		accyOther: String(c.accyOther),
		accyItems: String(c.accyItems),
		vileWard: String(c.vileWard),
		weaponNumber: String(c.weaponNumber),
		weaponAccy: String(c.weaponAccy),
		weaponBsAccy: String(c.weaponBsAccy)
	};
}

/** The charm number the coin window sends: only while the global filter is on, as MME checks. */
export function coinCharm(c: CalcCharacter | undefined): number | undefined {
	return c && c.globalFilter ? c.charm : undefined;
}

// ---------- Monster Attack Simulator (frmMonsterAttackSim) ----------

/** The defense text boxes; MME reads them with `Val`, so they stay text. */
export interface SimBoxes {
	ac: string;
	dr: string;
	dodge: string;
	mr: string;
	anti_magic: boolean;
	prot_evil: string;
	vile_ward: string;
	resist_cold: string;
	resist_fire: string;
	resist_stone: string;
	resist_lightning: string;
	resist_water: string;
}

export interface SimAttackStats {
	true_cast_pct: number;
	avg_per_hit: number;
	success_pct: number;
	dmg_resisted_pct: number;
	resist_dodge_pct: number;
}
export interface SimAttackRow {
	index: number;
	name: string;
	kind: number;
	stats: SimAttackStats | null;
}
export interface SimSummary {
	total_damage: number;
	average_damage: number;
	avg_damage: number;
	max_damage: number;
	max_seen: number;
	avg_phys: number;
	avg_spell: number;
	avg_caption: string;
	max_caption: string;
	breakdown_caption: string;
}
export interface SimResult {
	monster: { number: number; name: string };
	rounds: number;
	ran: boolean;
	seed: number;
	rng_after: number;
	energy_per_round: number;
	defender: SimBoxes;
	class: number;
	attacks: SimAttackRow[];
	summary: SimSummary | null;
	combat_log: string;
	error: { number: number; text: string } | null;
}
export interface SimDefenderResult {
	class: number;
	defender: SimBoxes;
}

/** `cmdResetUserDefs_Click(0)`: the boxes the window opens with (MR 50, the rest 0). */
export const SIM_BOXES: SimBoxes = {
	ac: '0',
	dr: '0',
	dodge: '0',
	mr: '50',
	anti_magic: false,
	prot_evil: '0',
	vile_ward: '0',
	resist_cold: '0',
	resist_fire: '0',
	resist_stone: '0',
	resist_lightning: '0',
	resist_water: '0'
};

/** The form's rounds limit and the server's default seed (VB6's unseeded generator). */
export const SIM_MAX_ROUNDS = 500000;
export const SIM_DEFAULT_SEED = 327680;

export interface SimForm {
	monster: number | null;
	rounds: string;
	/** Empty means the default seed. */
	seed: string;
	dynamic: boolean;
	logRounds: string;
	logMaxRoundOnly: boolean;
	hideEnergy: boolean;
	alwaysDodge: boolean;
	classNumber: number | null;
	boxes: SimBoxes;
}

export const SIM_DEFAULTS: SimForm = {
	monster: null,
	rounds: '500',
	seed: '',
	dynamic: false,
	logRounds: '100',
	logMaxRoundOnly: false,
	hideEnergy: false,
	alwaysDodge: false,
	classNumber: null,
	boxes: SIM_BOXES
};

/** A whole, non-negative number typed in a box, else undefined (the server's default applies). */
function wholeOrUndefined(s: string): number | undefined {
	const n = wholeNumber(s);
	return n === undefined || n < 0 || n > 4294967295 ? undefined : n;
}

/**
 * The run request. With `character` (the evaluate body of the Character tab) the server takes the
 * defense boxes and the class from that character; otherwise the typed boxes are sent.
 */
export function simRequest(f: SimForm, character?: object) {
	const seed = wholeOrUndefined(f.seed);
	const logRounds = wholeOrUndefined(f.logRounds);
	return {
		monster: f.monster ?? 0,
		rounds: f.rounds,
		...(seed === undefined ? {} : { seed }),
		dynamic: f.dynamic,
		...(logRounds === undefined ? {} : { log_rounds: Math.min(logRounds, 1000) }),
		log_max_round_only: f.logMaxRoundOnly,
		hide_energy: f.hideEnergy,
		always_dodge: f.alwaysDodge,
		defender: character
			? { character }
			: { ...f.boxes, ...(f.classNumber === null ? {} : { class: f.classNumber }) }
	};
}

/** The percentage cells of the attack table: MME's boxes show whole numbers or one decimal. */
export function pctText(n: number): string {
	return Number.isInteger(n) ? String(n) : n.toFixed(1);
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
	async function post<T>(name: CalcSlug | 'mega-stat' | 'monster-sim/defender', body: unknown): Promise<T> {
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
		megaStat: (text: string) => post<MegaResult>('mega-stat', { text }),
		backstab: (body: ReturnType<typeof backstabRequest>) => post<BackstabResult>('backstab', body),
		hit: (body: ReturnType<typeof hitRequest>) => post<HitResult>('hit', body),
		monsterSim: (body: ReturnType<typeof simRequest>) => post<SimResult>('monster-sim', body),
		monsterSimDefender: (character: object) =>
			post<SimDefenderResult>('monster-sim/defender', { character })
	};
}

export const calcClient = createCalcClient();

/** "Energy per swing: 58" -> "58": the part of a result caption after its label. */
export function afterLabel(caption: string): string {
	const i = caption.indexOf(': ');
	return i < 0 ? caption : caption.slice(i + 2);
}

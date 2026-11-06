// The Char tab (F10): the character as the server stores it, the quest and equipment-slot tables,
// the mapping from the form to `POST /api/character/evaluate`, and the REST client for saved
// characters. Every derived number comes from the server; nothing is computed here.
import type { Item } from './api';

type FetchFn = typeof fetch;

// ---------- wire types (snake_case, as `bin/touchstone/src/character.rs`) ----------

export interface AttackDoc {
	attack_type: number;
	backstab: boolean;
	backstab_weapon: number;
	martial_art: number;
	spell_num: number;
	spell_lvl: number;
	manual_p: number;
	manual_m: number;
	use_meditate: boolean;
	heal_type: number;
	heal_spell_num: number;
	heal_spell_lvl: number;
	heal_rounds: number;
	heal_manual: number;
}

export interface CharacterDoc {
	name: string;
	/** Class and race record numbers; null until picked. */
	race: number | null;
	class: number | null;
	level: number;
	/** 0 any, 1 good, 2 neutral, 3 evil. */
	alignment: number;
	global_filter: boolean;
	/** STR INT WIL AGL HEA CHM. */
	stats: number[];
	stat_adjustments: number[];
	/** 20 slots (see EQUIP_SLOTS). */
	equipped: number[];
	bless: number[];
	quests: boolean[];
	quest_options: number[];
	mr_text: string;
	anti_magic: boolean;
	add_weight: number;
	use_add_weight: boolean;
	attack: AttackDoc;
	learned_spells: number[];
	learned_class: number;
}

export interface EvalOptions {
	hide_class_race: boolean;
	min_item_level: number;
}

export interface CharacterSummary {
	name: string;
	race: number | null;
	class: number | null;
	level: number;
}

export interface BlessChoice {
	number: number;
	label: string;
}

export interface EvalState {
	setup: {
		quest_options: number[];
		magery_lvl: number;
		magery: number;
		has_picks: boolean;
		anti_magic: boolean;
		has_magic: boolean;
	} | null;
	quest_options: number[];
	cps: {
		stat_costs: number[];
		total_cost: number;
		used: number | null;
		avail: number | null;
		level_required: number | null;
		exp_kind: 'none' | 'a_lot' | 'value';
		exp: number | null;
		error: number | null;
		error_text: string | null;
	};
	stat_values: number[];
	stat_labels: (string | null)[];
	stat_bonus: number[];
	hp: {
		caption: string | null;
		min: number | null;
		max: number | null;
		average: number | null;
		rest_caption: string | null;
		rest_normal: number | null;
		rest_resting: number | null;
		regen: number;
		error_text: string | null;
	};
	mana: {
		max_caption: string | null;
		max_mana: number | null;
		regen_caption: string | null;
		mana_regen: number | null;
		medi_ticks: number | null;
		spellcasting_caption: string | null;
		spellcasting: number | null;
		regen: number;
		error_text: string | null;
	};
	magic_res: number | null;
	anti_magic: boolean;
	picklocks_caption: string | null;
	picklocks: number | null;
	dodge: number;
	dodge_caption: string;
	dodge_has_summary: boolean;
	encum: {
		current: string;
		max: string;
		word: string | null;
		caption: string | null;
		tag: string | null;
	};
	bless_mana: string | null;
	/** `lblInvenCharStat(0..46)` as MME shows them. */
	stats: string[];
	heal: { value: number; cost: number; rounds: number; spell_lvl: number };
	stats_error_text: string | null;
}

// ---------- tables ----------

export const STAT_NAMES = ['Strength', 'Intellect', 'Willpower', 'Agility', 'Health', 'Charm'] as const;

export const ALIGNMENTS = [
	{ value: 0, label: 'Any' },
	{ value: 1, label: 'Good' },
	{ value: 2, label: 'Neutral' },
	{ value: 3, label: 'Evil' }
] as const;

/** `docs/inventory.md` 5b: the names of `lblInvenCharStat(0..46)`; 43 and up have none in MME. */
export const DERIVED_STAT_NAMES: string[] = [
	'Current Encumbrance',
	'Max Encumbrance',
	'Armour Class',
	'Damage Resistance',
	'Encumbrance Bonus %',
	'HitPoints',
	'Mana',
	'Crits',
	'Dodge',
	'SpellCast',
	'Accuracy',
	'Max Damage',
	'Hit Magic',
	'BS Accuracy',
	'BS Min Damage',
	'BS Max Damage',
	'HitPoint Regen',
	'Mana Regen',
	'Perception',
	'Stealth',
	'Prot. Evil',
	'Traps',
	'Picklocks',
	'Illuminate',
	'Magic Resist',
	'Stone Resist',
	'Water Resist',
	'Fire Resist',
	'Cold Resist',
	'Lightning Res',
	'Min Damage',
	'Quickness',
	'Prot. Good',
	'Spell Damage',
	'Punch DMG',
	'Kick DMG',
	'Jumpkick DMG',
	'Punch Skill',
	'Kick Skill',
	'Jumpkick Skill',
	'Punch Accy',
	'Kick Accy',
	'Jumpkick Accy'
];

export function derivedStatName(i: number): string {
	return DERIVED_STAT_NAMES[i] ?? `Stat ${i}`;
}

export interface QuestDef {
	/** `chkCharQuests` index. */
	index: number;
	label: string;
	/** Index into `quest_options` when the quest has a reward choice. */
	option?: number;
	/** The stock engine applies it; the others only change GreaterMUD characters. */
	stock: boolean;
}

export const QUESTS: readonly QuestDef[] = [
	{ index: 0, label: 'Ice Sorceress (+1 AC)', stock: true },
	{ index: 1, label: 'High Druid (+1 SC)', stock: true },
	{ index: 2, label: 'ARD (+1 Crit, +2 SC)', stock: true },
	{ index: 3, label: 'Bishop (+3 ACC)', stock: true },
	{ index: 4, label: 'Apparatus (+1 Dodge)', stock: true },
	{ index: 5, label: '2nd alignment quest', option: 0, stock: true },
	{ index: 6, label: 'Opaline (+100 HP)', stock: false },
	{ index: 7, label: 'Cartographer (+3% Enc)', stock: false },
	{ index: 8, label: 'Loremaster (+1 AC)', stock: false },
	{ index: 9, label: '6th alignment quest', option: 1, stock: false },
	{ index: 10, label: 'Dread Wraith', option: 2, stock: false },
	{ index: 11, label: 'Renfry (War/Pal/Cler)', option: 3, stock: false }
];

/** The reward choices of the stock 2nd alignment quest, by `quest_options[0]`. */
export const QUEST_OPTIONS: Record<number, readonly string[]> = {
	0: [
		'Pick a reward',
		'+1 max damage',
		'+1 AC, +6 mana',
		'+1 SC, +10 mana',
		'+4 mana, +6 BS min/max, +1 stealth',
		'+10 BS min/max, +2 stealth'
	]
};

export function questOptionLabels(option: number): readonly string[] {
	return QUEST_OPTIONS[option] ?? ['Pick a reward'];
}

export interface SlotDef {
	/** Index into `equipped`. */
	index: number;
	label: string;
	/** Item `worn` values that fit the slot; empty means any wearable. */
	worn: readonly number[];
	/** Weapons fit too (the weapon slot and the off-hand). */
	weapons?: boolean;
	/** False for the weapon slot, which takes no armour. */
	armour?: boolean;
}

/** `nEquippedItem(0..19)`; 16 is the weapon. */
export const EQUIP_SLOTS: readonly SlotDef[] = [
	{ index: 0, label: 'Head', worn: [2] },
	{ index: 1, label: 'Ears', worn: [15] },
	{ index: 2, label: 'Neck', worn: [8] },
	{ index: 3, label: 'Back', worn: [7] },
	{ index: 4, label: 'Torso', worn: [11] },
	{ index: 5, label: 'Arms', worn: [6] },
	{ index: 6, label: 'Wrist', worn: [14] },
	{ index: 7, label: 'Wrist 2', worn: [14] },
	{ index: 8, label: 'Hands', worn: [3] },
	{ index: 9, label: 'Finger 1', worn: [4, 13] },
	{ index: 10, label: 'Finger 2', worn: [4, 13] },
	{ index: 11, label: 'Waist', worn: [10] },
	{ index: 12, label: 'Legs', worn: [9] },
	{ index: 13, label: 'Feet', worn: [5] },
	{ index: 14, label: 'Worn', worn: [] },
	{ index: 15, label: 'Off-Hand', worn: [12], weapons: true },
	{ index: 16, label: 'Weapon', worn: [], weapons: true, armour: false },
	{ index: 17, label: 'Eyes', worn: [15] },
	{ index: 18, label: 'Face', worn: [15] },
	{ index: 19, label: 'Everywhere', worn: [] }
];
export const WEAPON_SLOT = 16;
export const BLESS_SLOT_COUNT = 10;

// ---------- the character ----------

export function defaultAttack(): AttackDoc {
	return {
		attack_type: 0,
		backstab: false,
		backstab_weapon: 0,
		martial_art: 0,
		spell_num: 0,
		spell_lvl: 0,
		manual_p: 0,
		manual_m: 0,
		use_meditate: false,
		heal_type: 0,
		heal_spell_num: 0,
		heal_spell_lvl: 5,
		heal_rounds: 30,
		heal_manual: 0
	};
}

export function newCharacter(name = ''): CharacterDoc {
	return {
		name,
		race: null,
		class: null,
		level: 1,
		alignment: 0,
		global_filter: false,
		stats: [0, 0, 0, 0, 0, 0],
		stat_adjustments: [],
		equipped: Array<number>(20).fill(0),
		bless: Array<number>(BLESS_SLOT_COUNT).fill(0),
		quests: Array<boolean>(12).fill(false),
		quest_options: [0, 0, 0, 0],
		mr_text: '',
		anti_magic: false,
		add_weight: 0,
		use_add_weight: false,
		attack: defaultAttack(),
		learned_spells: [],
		learned_class: 0
	};
}

/** A stored character with every list at full length, so the form can index any slot. */
export function fromServer(doc: Partial<CharacterDoc>): CharacterDoc {
	const base = newCharacter();
	const pad = <T>(have: T[] | undefined, want: T[]): T[] => {
		const out = want.slice();
		(have ?? []).slice(0, want.length).forEach((v, i) => (out[i] = v));
		return out;
	};
	return {
		...base,
		...doc,
		stats: pad(doc.stats, base.stats),
		equipped: pad(doc.equipped, base.equipped),
		bless: pad(doc.bless, base.bless),
		quests: pad(doc.quests, base.quests),
		quest_options: pad(doc.quest_options, base.quest_options),
		stat_adjustments: doc.stat_adjustments ?? [],
		learned_spells: doc.learned_spells ?? [],
		attack: { ...base.attack, ...doc.attack }
	};
}

/** The same rule as the server's `validate_name`; the server stays the authority. */
export function nameProblem(name: string): string | undefined {
	const n = [...name].length;
	if (n === 0) return 'a character needs a name';
	if (n > 64) return 'a name is at most 64 characters';
	// eslint-disable-next-line no-control-regex
	if (/[\u0000-\u001f\u007f-\u009f/\\:*?"<>|]/.test(name)) return 'a name cannot contain / \\ : * ? " < > | or control characters';
	if (name.startsWith('.')) return 'a name cannot start with a dot';
	if (name.startsWith(' ') || name.endsWith(' ') || name.endsWith('.'))
		return 'a name cannot start with a space or end with a space or dot';
	return undefined;
}

/** A whole number from a box, clamped to MME's three digits; empty and junk are 0 like `Val`. */
export function boxNumber(text: string): number {
	const m = /^\s*(-?\d+)/.exec(text);
	if (!m) return 0;
	return Math.min(999, Math.max(0, Number(m[1])));
}

/**
 * The evaluate request, or undefined while class or race is not picked (the server would answer
 * 422 "pick a class and race").
 */
export function evalRequest(doc: CharacterDoc, options: EvalOptions): (CharacterDoc & { options: EvalOptions }) | undefined {
	if (doc.class === null || doc.race === null) return undefined;
	return { ...doc, options };
}

/** What gets saved: the character without view options. */
export function saveBody(doc: CharacterDoc): CharacterDoc {
	return doc;
}

// ---------- pickers ----------

export interface ItemChoice {
	number: number;
	name: string;
	label: string;
}

/** In-game items that fit the slot, by name as MME's combos sort them. */
export function slotChoices(items: Item[], slot: SlotDef): ItemChoice[] {
	return items
		.filter((i) => {
			if (!i.in_game) return false;
			if (slot.weapons && i.item_type === 1) return true;
			if (slot.armour === false || i.item_type !== 0) return false;
			if (slot.worn.length === 0) return i.worn !== 0;
			return slot.worn.includes(i.worn);
		})
		.map((i) => ({ number: i.number, name: i.name, label: `${i.name} (${i.number})` }))
		.sort((a, b) => a.name.localeCompare(b.name) || a.number - b.number);
}

/** Every word of the query must appear in the label, in any order. */
export function filterChoices<T extends { label: string }>(list: T[], query: string): T[] {
	const words = query.toLowerCase().split(/\s+/).filter(Boolean);
	if (words.length === 0) return list;
	return list.filter((c) => {
		const l = c.label.toLowerCase();
		return words.every((w) => l.includes(w));
	});
}

// ---------- REST client ----------

export class CharacterError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'CharacterError';
	}
}

export function createCharacterClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function call<T>(path: string, init?: RequestInit): Promise<T> {
		const res = await fetchFn(path, init);
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new CharacterError(res.status, reason || `HTTP ${res.status}`);
		}
		if (res.status === 204) return undefined as T;
		return (await res.json()) as T;
	}
	const json = (method: string, body: unknown): RequestInit => ({
		method,
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	const enc = (name: string) => `/api/characters/${encodeURIComponent(name)}`;
	return {
		evaluate: (body: object) => call<EvalState>('/api/character/evaluate', json('POST', body)),
		blessSpells(q: { class: number; level: number; alignment: number; all?: boolean; include?: number[] }) {
			const p = new URLSearchParams({
				class: String(q.class),
				level: String(q.level),
				alignment: String(q.alignment)
			});
			if (q.all) p.set('all', 'true');
			const inc = (q.include ?? []).filter((n) => n > 0);
			if (inc.length) p.set('include', inc.join(','));
			return call<BlessChoice[]>(`/api/character/bless-spells?${p}`);
		},
		list: () => call<CharacterSummary[]>('/api/characters'),
		async get(name: string) {
			return fromServer(await call<Partial<CharacterDoc>>(enc(name)));
		},
		async put(name: string, doc: CharacterDoc) {
			return fromServer(await call<Partial<CharacterDoc>>(enc(name), json('PUT', saveBody(doc))));
		},
		remove: (name: string) => call<void>(enc(name), { method: 'DELETE' })
	};
}

export type CharacterClient = ReturnType<typeof createCharacterClient>;
export const characterClient = createCharacterClient();

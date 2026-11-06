// What each MME explorer tab lists: which records belong, the raw columns in MME's order with
// MME's headers, the detail pane, and the computed columns that are still to come (`pending`).
// Column order and headers follow modMain's Add*2LV row builders; record filters follow the
// Filter* procedures in frmMain with MME's default "only in-game" setting.
import type { ClassRec, Item, Monster, Race, Room, Shop, Spell, TableName, TableTypes } from './api';
import type { Abilities, AbilitySlot } from './api';
import * as E from './enums';
import type { EvalTab } from './evaluate';
import type { SectionSlug } from './sections';

export type Cell = string | number;

export interface ColumnSpec<T> {
	id: string;
	header: string;
	width?: number;
	/** Right-aligned, numeric filter. */
	numeric?: boolean;
	/** Cell value; numbers sort and filter numerically. */
	get(rec: T): Cell;
	/** Display text when it differs from `String(get())`. */
	format?(v: Cell, rec: T): string;
	/** Numeric sort key for composite text such as "12/3". */
	sortKey?(rec: T): number;
}

export interface DetailRow {
	label: string;
	value: string;
	/** Free text that should wrap instead of staying on one line. */
	long?: boolean;
}
export interface DetailGroup {
	title: string;
	rows: DetailRow[];
}

/** Server-computed values the detail pane waits for; each is absent until it arrives. */
export interface DetailExtra {
	/** MME's ability text for the record. */
	abilities?: Abilities;
	/** The text could not be fetched; the raw slots are shown instead. */
	abilitiesFailed?: boolean;
	/** `GetMaxDamage` for a monster. */
	maxDamage?: number;
}

/** Name lookups for cross references in the detail pane; any table may still be loading. */
export interface Lookups {
	items?: Map<number, string>;
	spells?: Map<number, string>;
	monsters?: Map<number, string>;
	classes?: Map<number, string>;
	races?: Map<number, string>;
	/** MME's NMR version as a number (v1.8.3 is 1.83); gates a few record filters. */
	nmr: number;
}

export interface Explorer<K extends TableName = TableName> {
	slug: SectionSlug;
	table: K;
	/** Other tables the detail pane resolves names from. */
	needs: TableName[];
	include(rec: TableTypes[K], lk: Lookups): boolean;
	columns: ColumnSpec<TableTypes[K]>[];
	rowId(rec: TableTypes[K]): string | number;
	title(rec: TableTypes[K]): string;
	defaultSort?: { col: string; dir: 'asc' | 'desc' };
	/** Set when the server builds the list (`/api/evaluate`); `columns` is then unused. */
	computed?: EvalTab;
	/** Columns MME shows that need the rules engine or a character; not faked here. */
	pending: string[];
	detail(rec: TableTypes[K], lk: Lookups, extra?: DetailExtra): DetailGroup[];
}

// ---------- helpers ----------

export function putCommas(n: number): string {
	const [int, frac] = String(n).split('.');
	const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
	return frac ? `${grouped}.${frac}` : grouped;
}
/** VB Format(n, "#,#") for a value that is shown as 0 when not positive. */
const commaOrZero = (v: Cell) => (typeof v === 'number' && v > 0 ? putCommas(Math.round(v)) : '0');

/** Value of the last slot holding one of `ids`, as MME's ability loops leave it. */
export function abilityValue(slots: AbilitySlot[], ...ids: number[]): number | undefined {
	let found: number | undefined;
	for (const s of slots) if (s.ability !== 0 && ids.includes(s.ability)) found = s.value;
	return found;
}

/** "Name (#n)" when the table is loaded, "#n" otherwise. */
function ref(map: Map<number, string> | undefined, n: number): string {
	const name = map?.get(n);
	return name ? `${name} (#${n})` : `#${n}`;
}

const text = (s: string | null | undefined) => s ?? '';
const yes = (b: boolean) => (b ? 'Yes' : 'No');
const row = (label: string, value: string | number, long = false): DetailRow => ({
	label,
	value: String(value),
	long
});
const group = (title: string, rows: DetailRow[]): DetailGroup => ({ title, rows });

function abilityRows(slots: AbilitySlot[]): DetailRow[] {
	return slots
		.filter((s) => s.ability !== 0)
		.map((s) => row(`Ability ${s.ability}`, s.value));
}
/**
 * The Abilities group: MME's text ("Accuracy" / "+5") once the server has answered, the raw slots
 * only if it could not. While the text is on its way the group is left out rather than flashing
 * numbers.
 */
function abilityGroup(slots: AbilitySlot[], extra: DetailExtra | undefined): DetailGroup[] {
	const ab = extra?.abilities;
	if (!ab) {
		const raw = extra?.abilitiesFailed ? abilityRows(slots) : [];
		return raw.length ? [group('Abilities', raw)] : [];
	}
	const rows = ab.lines.map((l) => row(l.name, l.detail || 'Yes', l.detail.length > 36));
	if (ab.class_ok.length) rows.push(row('Class OK', ab.class_ok.join(', '), true));
	return rows.length ? [group('Abilities', rows)] : [];
}
function listRefs(label: string, nums: number[], map: Map<number, string> | undefined): DetailRow[] {
	const set = nums.filter((n) => n !== 0);
	return set.length ? [row(label, set.map((n) => ref(map, n)).join(', '), true)] : [];
}

const num = <T>(id: string, header: string, get: (r: T) => number, width = 80): ColumnSpec<T> => ({
	id,
	header,
	width,
	numeric: true,
	get
});
const str = <T>(id: string, header: string, get: (r: T) => string, width?: number): ColumnSpec<T> => ({
	id,
	header,
	width,
	get
});
const numberCol = <T extends { number: number }>(): ColumnSpec<T> =>
	num<T>('num', '#', (r) => r.number, 70);

// ---------- items: Weapons, Armour, Sundry ----------


const acText = (r: Item, round: boolean) =>
	`${round ? Math.ceil(r.armour_class / 10) : r.armour_class / 10}/${r.damage_resist / 10}`;

function itemDetail(r: Item, lk: Lookups, extra?: DetailExtra): DetailGroup[] {
	const isWeapon = r.item_type === 1;
	const isArmour = r.item_type === 0 && r.worn !== 0;
	const stats = [
		row('Number', r.number),
		row('Type', E.itemType(r.item_type)),
		row('Encumbrance', r.encum),
		row('Price', `${putCommas(r.price)} ${E.costType(r.currency).toLowerCase()}`),
		row('Limit', r.limit),
		row('Uses', r.use_count)
	];
	if (isWeapon)
		stats.push(
			row('Weapon type', E.weaponType(r.weapon_type)),
			row('Damage', `${r.min} - ${r.max}`),
			row('Speed', r.speed),
			row('Strength required', r.str_req)
		);
	if (isArmour || r.armour_class || r.damage_resist)
		stats.push(
			row('Worn', E.wornType(r.worn)),
			row('Armour type', E.armourType(r.armour_type)),
			row('AC / DR', acText(r, false))
		);
	if (r.accy) stats.push(row('Accuracy', r.accy));
	const out = [
		group(r.name, stats),
		group('Flags', [
			row('In game', yes(r.in_game)),
			row('Gettable', yes(r.gettable)),
			row('Not droppable', yes(r.not_droppable)),
			row('Destroyed on death', yes(r.destroy_on_death)),
			row('Retained after uses', yes(r.retain_after_uses))
		])
	];
	const restrictions = [
		...listRefs('Classes barred', r.class_rest, lk.classes),
		...listRefs('Races barred', r.race_rest, lk.races),
		...listRefs('Negates spells', r.negate_spell, lk.spells)
	];
	if (restrictions.length) out.push(group('Restrictions', restrictions));
	out.push(...abilityGroup(r.abilities, extra));
	const where = [
		...(r.obtained_from ? [row('Obtained from', r.obtained_from, true)] : []),
		...(r.references ? [row('References', r.references, true)] : [])
	];
	if (where.length) out.push(group('Locations', where));
	return out;
}

const itemBase = {
	needs: ['classes', 'races', 'spells'] as TableName[],
	rowId: (r: Item) => r.number,
	title: (r: Item) => r.name,
	defaultSort: { col: 'name', dir: 'asc' as const },
	detail: itemDetail
};
const namedOk = (r: Item) => r.name !== '';

const weapons: Explorer<'items'> = {
	...itemBase,
	slug: 'weapons',
	table: 'items',
	include: (r) => r.item_type === 1 && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/weapons): MME's columns and cells.
	computed: 'weapons',
	columns: [],
	pending: []
};

const armour: Explorer<'items'> = {
	...itemBase,
	slug: 'armour',
	table: 'items',
	include: (r) => r.item_type === 0 && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/armour): MME's columns and cells.
	computed: 'armour',
	columns: [],
	pending: []
};

const sundry: Explorer<'items'> = {
	...itemBase,
	slug: 'items',
	table: 'items',
	include: (r) => ((r.item_type === 0 && r.worn === 0) || r.item_type > 1) && r.in_game && namedOk(r),
	// The list comes from the server (POST /api/evaluate/items): MME's columns and cells.
	computed: 'items',
	columns: [],
	pending: []
};

// ---------- spells ----------

/** Port of modMMudFunc SpellIsInGame; `disableKai` comes from the Info row (false when unknown). */
export function spellInGame(s: Spell, nmr: number, disableKai = false): boolean {
	const len = (x: string | null) => (x ?? '').length;
	const unusedKai = s.magery !== 5 || s.req_level < 1 || disableKai;
	if (!s.learnable && len(s.learned_from) <= 1 && len(s.casted_by) <= 1 && unusedKai) {
		return nmr >= 1.8 && len(s.classes) > 1;
	}
	return true;
}

const spellName = (s: Spell) => text(s.name);

const spells: Explorer<'spells'> = {
	slug: 'spells',
	table: 'spells',
	needs: [],
	include: (s, lk) => {
		const n = spellName(s);
		return n !== '' && !n.startsWith('1') && !n.toLowerCase().startsWith('sdf') && spellInGame(s, lk.nmr);
	},
	// The list comes from the server (POST /api/evaluate/spells): MME's columns and cells.
	computed: 'spells',
	columns: [],
	rowId: (s) => s.number,
	title: spellName,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: [],
	detail: (s, _lk, extra) => {
		const out = [
			group(spellName(s), [
				row('Number', s.number),
				row('Short', text(s.short)),
				row('Magery', E.mageryName(s.magery, s.magery_lvl)),
				row('Required level', s.req_level),
				row('Mana cost', s.mana_cost),
				row('Energy cost', s.energy_cost),
				row('Difficulty', s.diff),
				row('Targets', E.spellTargets(s.targets)),
				row('Attack type', E.spellAttackType(s.att_type)),
				row('Resist type', s.type_of_resists),
				row('Learnable', yes(s.learnable))
			]),
			group('Effect', [
				row('Base', `${s.min_base} - ${s.max_base}`),
				row('Duration', s.dur),
				row('Cap level', s.cap),
				row('Max grows', `+${s.max_inc} per ${s.max_inc_lvls} levels`),
				row('Min grows', `+${s.min_inc} per ${s.min_inc_lvls} levels`),
				row('Duration grows', `+${s.dur_inc} per ${s.dur_inc_lvls} levels`)
			])
		];
		out.push(...abilityGroup(s.abilities, extra));
		const where = [
			...(s.learned_from ? [row('Learned from', s.learned_from, true)] : []),
			...(s.casted_by ? [row('Cast by', s.casted_by, true)] : []),
			...(s.classes ? [row('Classes', s.classes, true)] : [])
		];
		if (where.length) out.push(group('Locations', where));
		return out;
	}
};

// ---------- monsters ----------

const DODGE = 34;
const MAGICAL = 28;
const monName = (m: Monster) => text(m.name);

const monsters: Explorer<'monsters'> = {
	slug: 'monsters',
	table: 'monsters',
	needs: ['items', 'spells'],
	include: (m) => m.in_game && monName(m) !== '' && !monName(m).startsWith('sdf'),
	// The column set of NMR v1.83 and later (Script Value is only shown by older data).
	columns: [
		numberCol<Monster>(),
		str('name', 'Name', monName, 240),
		num('rgn', 'RGN', (m) => m.regen_time, 80),
		{
			...num<Monster>('exp', 'Exp', (m) => m.exp * m.exp_multi, 120),
			format: commaOrZero
		},
		{ ...num<Monster>('hp', 'HP', (m) => m.hp, 80), format: commaOrZero },
		{
			...str<Monster>('acdr', 'AC/DR', (m) => `${m.armour_class}/${m.damage_resist}`, 90),
			sortKey: (m) => m.armour_class + m.damage_resist
		},
		num('dodge', 'Dodge', (m) => abilityValue(m.abilities, DODGE) ?? 0, 75),
		num('mr', 'MR', (m) => m.magic_res, 65),
		{
			...num<Monster>('lairexp', 'Lair Exp', (m) => m.avg_lair_exp, 95),
			format: (v) => putCommas(Number(v))
		},
		{
			...num<Monster>('mag', 'Mag.', (m) => abilityValue(m.abilities, MAGICAL) ?? 0, 70),
			format: (v) => (v ? String(v) : '')
		},
		{
			...num<Monster>('bsdef', 'BS Defense', (m) => m.bs_defense, 95),
			format: (v) => (v ? String(v) : '')
		},
		str('undead', 'Undead', (m) => (m.undead > 0 ? 'X' : ''), 80)
	],
	rowId: (m) => m.number,
	title: monName,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: ['Acc (Maj/Mx)', 'Damage', 'Exp/(Dmg+HP)', 'Lairs', 'Mobs/Spwn', 'Spell Atk.'],
	detail: (m, lk, extra) => {
		const out = [
			group(monName(m), [
				row('Number', m.number),
				row('Type', E.monsterType(m.type)),
				row('Alignment', E.monsterAlignment(m.align)),
				row('HP', putCommas(m.hp)),
				row('Exp', putCommas(m.exp)),
				row('Exp multiplier', m.exp_multi),
				row('AC / DR', `${m.armour_class} / ${m.damage_resist}`),
				row('Magic resist', m.magic_res),
				row('BS defense', m.bs_defense),
				row('Undead', yes(m.undead > 0)),
				row('Energy', m.energy),
				row('HP regen', m.hp_regen),
				row('Regen time', m.regen_time),
				row('Charm level', m.charm_lvl),
				row('Follow %', m.follow_pct),
				row('Game limit', m.game_limit),
				...(extra?.maxDamage === undefined
					? []
					: [row('Max damage / round', putCommas(extra.maxDamage))]),
				row('Stored avg damage', m.avg_dmg),
				row('Stored lair exp', putCommas(m.avg_lair_exp)),
				row('Weapon', ref(lk.items, m.weapon)),
				row('In game', yes(m.in_game))
			])
		];
		const attacks = m.attacks
			.filter((a) => a.att_type !== 0 || a.pct > 0)
			.map((a, i) =>
				row(
					`Attack ${i + 1}`,
					`${a.name}: ${E.monsterAttackType(a.att_type)}, acc ${a.acc}, ${a.pct}%, ${a.min}-${a.max}` +
						(a.hit_spell ? `, hit spell ${ref(lk.spells, a.hit_spell)}` : ''),
					true
				)
			);
		if (attacks.length) out.push(group('Attacks', attacks));
		const mid = m.mid_spells
			.filter((s) => s.spell !== 0)
			.map((s) => row(ref(lk.spells, s.spell), `${s.pct}% at level ${s.lvl}`));
		if (m.death_spell) mid.push(row('On death', ref(lk.spells, m.death_spell)));
		if (m.create_spell) mid.push(row('On creation', ref(lk.spells, m.create_spell)));
		if (mid.length) out.push(group('Spells', mid));
		const drops = m.drops.filter((d) => d.item !== 0).map((d) => row(ref(lk.items, d.item), `${d.pct}%`));
		if (drops.length) out.push(group('Drops', drops));
		// Guarded-by (146) is a row of its own in MME; the server leaves it out of the text.
		out.push(...abilityGroup(m.abilities, extra));
		if (m.summoned_by) out.push(group('Locations', [row('Summoned by', m.summoned_by, true)]));
		return out;
	}
};

// ---------- shops ----------

const shopOk = (s: Shop) => {
	const n = s.name;
	return n !== '' && !n.toLowerCase().startsWith('sdf') && n !== 'Leave this blank';
};

const shops: Explorer<'shops'> = {
	slug: 'shops',
	table: 'shops',
	needs: ['classes'],
	include: (s) => s.in_game && shopOk(s),
	// The list comes from the server (POST /api/evaluate/shops): MME's columns and cells.
	computed: 'shops',
	columns: [],
	rowId: (s) => s.number,
	title: (s) => s.name,
	defaultSort: { col: 'name', dir: 'asc' },
	pending: [],
	detail: (s, lk) => {
		const out = [
			group(s.name, [
				row('Number', s.number),
				row('Type', E.shopType(s.shop_type)),
				row('Level range', `${s.min_lvl} - ${s.max_lvl}`),
				row('Markup', `${s.markup_pct}%`),
				row('Class restriction', s.class_rest ? ref(lk.classes, s.class_rest) : 'None'),
				row('In game', yes(s.in_game))
			])
		];
		// The stock list with charm-adjusted prices comes from the server (ShopPrices).
		if (s.assigned_to) out.push(group('Locations', [row('Assigned to', s.assigned_to, true)]));
		return out;
	}
};

// ---------- races and classes ----------

const range = (a: number, b: number) => `${a}-${b}`;

const races: Explorer<'races'> = {
	slug: 'races',
	table: 'races',
	needs: [],
	include: (r) => r.name !== '',
	// The list comes from the server (POST /api/evaluate/races): MME's columns and cells.
	computed: 'races',
	columns: [],
	rowId: (r) => r.number,
	title: (r) => r.name,
	defaultSort: { col: 'num', dir: 'asc' },
	pending: [],
	detail: (r, _lk, extra) => {
		const out = [
			group(r.name, [
				row('Number', r.number),
				row('Exp', `${r.exp_table}%`),
				row('HP per level', r.hp_per_lvl),
				row('Base CP', r.base_cp)
			]),
			group('Stat ranges', [
				row('Strength', range(r.m_str, r.x_str)),
				row('Intellect', range(r.m_int, r.x_int)),
				row('Willpower', range(r.m_wil, r.x_wil)),
				row('Agility', range(r.m_agl, r.x_agl)),
				row('Health', range(r.m_hea, r.x_hea)),
				row('Charm', range(r.m_chm, r.x_chm))
			])
		];
		out.push(...abilityGroup(r.abilities, extra));
		return out;
	}
};

const classes: Explorer<'classes'> = {
	slug: 'classes',
	table: 'classes',
	needs: [],
	include: (c) => c.name !== '',
	// The list comes from the server (POST /api/evaluate/classes): MME's columns and cells.
	computed: 'classes',
	columns: [],
	rowId: (c) => c.number,
	title: (c) => c.name,
	defaultSort: { col: 'num', dir: 'asc' },
	pending: [],
	detail: (c, _lk, extra) => {
		const out = [
			group(c.name, [
				row('Number', c.number),
				row('Exp', `${c.exp_table + 100}%`),
				row('Weapons', E.classWeaponType(c.weapon_type)),
				row('Armour', E.armourType(c.armour_type)),
				row('Magic', E.mageryName(c.magery_type, c.magery_lvl)),
				row('Combat level', c.combat_lvl - 2),
				row('HP per level', range(c.min_hits, c.min_hits + c.max_hits))
			])
		];
		// Ability 59 only marks the class as usable, which MME leaves out of its Abilities text.
		out.push(...abilityGroup(c.abilities.filter((a) => a.ability !== 59), extra));
		return out;
	}
};

// ---------- rooms ----------

const EXITS = [
	['n', 'N'],
	['s', 'S'],
	['e', 'E'],
	['w', 'W'],
	['ne', 'NE'],
	['nw', 'NW'],
	['se', 'SE'],
	['sw', 'SW'],
	['u', 'U'],
	['d', 'D']
] as const;

const rooms: Explorer<'rooms'> = {
	slug: 'rooms',
	table: 'rooms',
	needs: [],
	// MME has no room list (rooms are reached by map and room number), so every room is listed.
	include: () => true,
	columns: [
		num<Room>('map', 'Map', (r) => r.map_number, 70),
		num<Room>('room', 'Room', (r) => r.room_number, 75),
		str<Room>('name', 'Name', (r) => text(r.name), 260),
		num<Room>('light', 'Light', (r) => r.light, 70),
		num<Room>('shop', 'Shop', (r) => r.shop, 70),
		num<Room>('npc', 'NPC', (r) => r.npc, 70),
		num<Room>('cmd', 'CMD', (r) => r.cmd, 70),
		num<Room>('spell', 'Spell', (r) => r.spell, 70),
		str<Room>('lair', 'Lair', (r) => text(r.lair), 160),
		num<Room>('delay', 'Delay', (r) => r.delay, 70),
		str<Room>('placed', 'Placed', (r) => text(r.placed), 160),
		...EXITS.map(([k, label]) => str<Room>(k, label, (r) => (r[k] === '0' ? '' : r[k]), 90))
	],
	rowId: (r) => `${r.map_number}/${r.room_number}`,
	title: (r) => `${r.map_number}/${r.room_number} ${text(r.name)}`,
	pending: [],
	detail: (r) => {
		const exits = EXITS.filter(([k]) => r[k] !== '0').map(([k, label]) => row(label, r[k]));
		const out = [
			group(text(r.name) || `Room ${r.room_number}`, [
				row('Map', r.map_number),
				row('Room', r.room_number),
				row('Light', r.light),
				row('Shop', r.shop),
				row('NPC', r.npc),
				row('CMD', r.cmd),
				row('Spell', r.spell),
				row('Delay', r.delay)
			])
		];
		if (exits.length) out.push(group('Exits', exits));
		const where = [
			...(r.lair ? [row('Lair', r.lair, true)] : []),
			...(r.placed ? [row('Placed', r.placed, true)] : [])
		];
		if (where.length) out.push(group('Spawns', where));
		return out;
	}
};

// ---------- registry ----------

export type AnyExplorer = Explorer<TableName>;

export const EXPLORERS: Partial<Record<SectionSlug, AnyExplorer>> = {
	weapons,
	armour,
	spells,
	monsters,
	shops,
	items: sundry,
	races,
	classes,
	rooms
} as unknown as Partial<Record<SectionSlug, AnyExplorer>>;

export function explorerFor(slug: string): AnyExplorer | undefined {
	return (EXPLORERS as Record<string, AnyExplorer | undefined>)[slug];
}

/** MME's NMR version as a number: "v1.8.3" is 1.83. */
export function parseNmr(v: string): number {
	const m = /(\d+)\.(\d+)(?:\.(\d+))?/.exec(v);
	return m ? Number(`${m[1]}.${m[2]}${m[3] ?? ''}`) : 0;
}

/** The rows that belong on the tab, in table order. */
export function listRows<K extends TableName>(ex: Explorer<K>, table: TableTypes[K][], lk: Lookups) {
	return table.filter((r) => ex.include(r, lk));
}

export function buildLookups(
	tables: Partial<{ [K in TableName]: TableTypes[K][] }>,
	nmr: number
): Lookups {
	const map = <T extends { number: number }>(rows: T[] | undefined, name: (r: T) => string | null) =>
		rows ? new Map(rows.map((r) => [r.number, name(r) ?? ''] as [number, string])) : undefined;
	return {
		items: map(tables.items, (r) => r.name),
		spells: map(tables.spells, (r) => r.name),
		monsters: map(tables.monsters, (r) => r.name),
		classes: map(tables.classes, (r) => r.name),
		races: map(tables.races, (r) => r.name),
		nmr
	};
}

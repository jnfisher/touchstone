import type { RefRow } from './detail';
/** Types matching the server's JSON (snake_case field names of the `touchstone-model` structs). */

export interface AbilitySlot {
	ability: number;
	value: number;
}

export interface Item {
	number: number;
	name: string;
	limit: number;
	encum: number;
	item_type: number;
	use_count: number;
	price: number;
	currency: number;
	min: number;
	max: number;
	armour_class: number;
	damage_resist: number;
	weapon_type: number;
	armour_type: number;
	worn: number;
	accy: number;
	gettable: boolean;
	str_req: number;
	speed: number;
	not_droppable: boolean;
	destroy_on_death: boolean;
	retain_after_uses: boolean;
	class_rest: number[];
	race_rest: number[];
	negate_spell: number[];
	abilities: AbilitySlot[];
	in_game: boolean;
	obtained_from: string | null;
	references: string | null;
}

export interface MonsterAttack {
	name: string;
	att_type: number;
	acc: number;
	pct: number;
	true_pct: number;
	min: number;
	max: number;
	energy: number;
	hit_spell: number;
}

export interface Monster {
	number: number;
	name: string | null;
	weapon: number;
	armour_class: number;
	damage_resist: number;
	follow_pct: number;
	magic_res: number;
	bs_defense: number;
	exp: number;
	exp_multi: number;
	hp: number;
	energy: number;
	avg_dmg: number;
	avg_dmg_phys: number;
	avg_dmg_spell: number;
	greet_txt: number;
	hp_regen: number;
	charm_lvl: number;
	type: number;
	undead: number;
	align: number;
	regen_time: number;
	game_limit: number;
	avg_lair_exp: number;
	script_value: number;
	r: number;
	p: number;
	g: number;
	s: number;
	c: number;
	death_spell: number;
	create_spell: number;
	attacks: MonsterAttack[];
	mid_spells: { spell: number; pct: number; lvl: number }[];
	drops: { item: number; pct: number }[];
	abilities: AbilitySlot[];
	in_game: boolean;
	summoned_by: string | null;
}

export interface Spell {
	number: number;
	name: string | null;
	short: string | null;
	req_level: number;
	energy_cost: number;
	mana_cost: number;
	min_base: number;
	max_base: number;
	diff: number;
	type_of_resists: number;
	targets: number;
	dur: number;
	att_type: number;
	magery: number;
	magery_lvl: number;
	cap: number;
	max_inc_lvls: number;
	max_inc: number;
	min_inc_lvls: number;
	min_inc: number;
	dur_inc_lvls: number;
	dur_inc: number;
	abilities: AbilitySlot[];
	learnable: boolean;
	learned_from: string | null;
	casted_by: string | null;
	classes: string | null;
}

export interface Room {
	map_number: number;
	room_number: number;
	name: string | null;
	light: number;
	shop: number;
	npc: number;
	cmd: number;
	spell: number;
	lair: string | null;
	delay: number;
	placed: string | null;
	n: string;
	s: string;
	e: string;
	w: string;
	ne: string;
	nw: string;
	se: string;
	sw: string;
	u: string;
	d: string;
}

export interface Shop {
	number: number;
	name: string;
	shop_type: number;
	min_lvl: number;
	max_lvl: number;
	markup_pct: number;
	class_rest: number;
	stock: { item: number; max: number; time: number; amount: number; pct: number }[];
	in_game: boolean;
	assigned_to: string | null;
}

export interface ClassRec {
	number: number;
	name: string;
	min_hits: number;
	max_hits: number;
	exp_table: number;
	magery_type: number;
	magery_lvl: number;
	weapon_type: number;
	armour_type: number;
	combat_lvl: number;
	abilities: AbilitySlot[];
}

export interface Race {
	number: number;
	name: string;
	m_int: number;
	m_wil: number;
	m_str: number;
	m_hea: number;
	m_agl: number;
	m_chm: number;
	x_int: number;
	x_wil: number;
	x_str: number;
	x_hea: number;
	x_agl: number;
	x_chm: number;
	hp_per_lvl: number;
	exp_table: number;
	base_cp: number;
	abilities: AbilitySlot[];
}

export interface Lair {
	group_index: string;
	mob_list: string;
	mobs: number;
	total_lairs: number;
	avg_delay: number;
	avg_walk: number;
	avg_exp: number;
	avg_dmg: number;
	avg_dmg_phys: number;
	avg_dmg_spell: number;
	avg_hp: number;
	avg_ac: number;
	avg_dr: number;
	avg_mr: number;
	avg_dodge: number;
}

export interface Info {
	counts: Record<string, number>;
	data_version: number | null;
	load_count: number;
	loaded_at: string;
	source: { kind: string; path: string };
	version: {
		custom: string;
		dat_file_version: string;
		date: string;
		nmr_version: string;
		time: string;
	};
}

export interface TableTypes {
	items: Item;
	monsters: Monster;
	spells: Spell;
	rooms: Room;
	shops: Shop;
	classes: ClassRec;
	races: Race;
	lairs: Lair;
}
export type TableName = keyof TableTypes;
export const TABLE_NAMES: readonly TableName[] = [
	'items',
	'monsters',
	'spells',
	'rooms',
	'shops',
	'classes',
	'races',
	'lairs'
];

/** `GET /api/shops/{n}/prices`: each stocked item's buy and sell price, or a trainer's levels. */
export interface ShopPriceRow {
	slot: number;
	item: number;
	name: string;
	max: number;
	regen: string;
	copper_buy: number;
	buy: string;
	buy_short: string;
	copper_sell: number;
	sell: string;
	sell_short: string;
}
export interface TrainLevel {
	level: number;
	copper: number;
	cost: string;
}
export interface ShopPrices {
	shop: number;
	name: string;
	shop_type: number;
	min_lvl: number;
	max_lvl: number;
	markup: number;
	class_rest: number;
	charm: number;
	/** MME's charm label for buying; empty at charm 0. */
	buy_note: string;
	sell_note: string;
	training: boolean;
	items: ShopPriceRow[];
	levels: TrainLevel[];
}

export interface ReachabilityStatus {
	computed: boolean;
	rooms: number;
	edges: number;
	random_groups: number;
	level_bands: number;
	monsters: number;
	located: number;
}

/** `GET /api/exp`: exp needed to reach each level and the cost to train to it. */
export interface ExpRow {
	level: number;
	exp: number;
	exp_text: string;
	train_copper: number;
	train_cost: string;
}
export interface ExpTable {
	race: number;
	class: number;
	markup: number;
	rows: ExpRow[];
}
export interface ExpQuery {
	race: number;
	class: number;
	from?: number;
	to?: number;
	markup?: number;
}

/** The query string of an exp request; omitted fields take the server's defaults. */
export function expPath(q: ExpQuery): string {
	const p = new URLSearchParams({ race: String(q.race), class: String(q.class) });
	if (q.from !== undefined) p.set('from', String(q.from));
	if (q.to !== undefined) p.set('to', String(q.to));
	if (q.markup !== undefined) p.set('markup', String(q.markup));
	return `/api/exp?${p}`;
}

/** `GET /api/class-race-builder`: the Build view's text for a class and a race (0 for none). */
export interface ClassRaceBuild {
	/** The class and race MME used: a number with no row counts as none (0). */
	class: number;
	race: number;
	/** The text box, CRLF between lines; null when MME's handler ran and the box kept its old text. */
	text: string | null;
	lines: string[];
	levels: number[];
	error: string | null;
}

export function builderPath(cls: number, race: number): string {
	return `/api/class-race-builder?class=${cls}&race=${race}`;
}

/** The Equip button: the EQ tab's pickers take these when set. */
export interface BuilderEquip {
	class: number | null;
	race: number | null;
}

/** The Spellbook button: `no_magic` is MME's "No Magic" box, `open` opens the book at level 999. */
export interface BuilderSpellbook {
	action: 'nothing' | 'no_magic' | 'open';
	class: number | null;
	level: number | null;
	magery: number | null;
	magery_level: number | null;
}

/** `POST /api/spells/counters`: the spells and items that negate, dispel or kill a spell. */
export interface CounterRow {
	table: 'items' | 'spells';
	number: number;
	text: string;
}
export interface Counters {
	/** MME asks "Cast spell at a certain level?" first: ask, then send `level_answer`. */
	ask_level: boolean;
	level: number;
	rows: CounterRow[];
}
export interface CountersRequest {
	spell: number;
	/** The monster pane row's tag (`number` or `number@level`). */
	tag?: string;
	level_answer?: number;
}

/** `GET /api/spells/{n}/casts`: what casts a spell. Rows link to the records. */
/** One item of the ability combos (`GetAbilityList`). */
export interface AbilityName {
	number: number;
	name: string;
}

export interface SpellCasts {
	status: 'found' | 'nothing' | 'missing';
	caption: string;
	rows: RefRow[];
}

/** `GET /api/items/{n}/chest`: a container's contents, most likely first. */
export interface ChestRow {
	text: string;
	percent: number;
	item_text: string;
	item: number;
}
export interface Chest {
	status: 'rows' | 'no_item' | 'not_container' | 'no_data';
	message: string | null;
	rows: ChestRow[];
}

/** The Spell Book window's controls, as `POST /api/spell-book/list` takes them. */
export interface SpellBookRequest {
	class: number;
	level: string;
	alignment: number;
	magery: number;
	magery_level: number;
	filter_on: boolean;
	global_level: string;
	global_class: number;
	char_magery: number;
	char_magery_level: number;
}
export interface SpellBookList {
	spells: number[];
	uses_character: boolean;
	aborted: boolean;
	sort_ascending: boolean;
}
export interface SpellBookClass {
	magery_level: number | null;
	magery: number | null;
}
export interface SpellBookCopy {
	alignment: number | null;
	level: string | null;
	class: number | null;
}

/** `GET /api/abilities/{table}/{n}`: MME's ability text for a record, one line per printed slot. */
export type AbilityTable = 'items' | 'spells' | 'races' | 'classes' | 'monsters';
export interface AbilityLine {
	ability: number;
	value: number;
	name: string;
	/** `GetAbilityStats`: the name followed by the value. */
	text: string;
	/** `text` without the leading name. */
	detail: string;
}
export interface Abilities {
	table: AbilityTable;
	number: number;
	lines: AbilityLine[];
	/** Item ability 59: class names, listed apart from the abilities. */
	class_ok: string[];
}

/** `GET /api/spells/{n}/effect?level=L`: numbers and effect text at a caster level. */
export interface SpellRange {
	min: number;
	max: number;
}
export interface SpellEffect {
	number: number;
	level: number;
	used_level: number;
	req_level: number;
	cap: number;
	damage: SpellRange | null;
	heal: SpellRange | null;
	duration: number;
	effect: string;
}

/** `GET /api/monsters/{n}/max-damage`. */
export interface MonsterMaxDamage {
	number: number;
	max_damage: number;
	energy_per_round: number;
}

export const abilitiesPath = (table: AbilityTable, n: number) => `/api/abilities/${table}/${n}`;
export const spellEffectPath = (n: number, level?: number) =>
	`/api/spells/${n}/effect${level === undefined ? '' : `?level=${level}`}`;
export const maxDamagePath = (n: number) => `/api/monsters/${n}/max-damage`;

type FetchFn = typeof fetch;
interface Entry {
	etag: string | null;
	data: unknown;
}

export class ApiError extends Error {
	constructor(
		readonly status: number,
		path: string
	) {
		super(`GET ${path} failed: HTTP ${status}`);
	}
}

/**
 * Fetches whole tables once and keeps them. `table()` never refetches; `revalidate()` sends the
 * stored ETag and keeps the cached data on a 304. Concurrent callers share one request.
 */
export function createApi(fetchFn: FetchFn = (...a) => fetch(...a)) {
	const cache = new Map<string, Entry>();
	const inflight = new Map<string, Promise<unknown>>();

	async function load(path: string, revalidate: boolean): Promise<unknown> {
		const prev = cache.get(path);
		const headers: Record<string, string> = {};
		if (revalidate && prev?.etag) headers['If-None-Match'] = prev.etag;
		const res = await fetchFn(path, { headers });
		if (res.status === 304 && prev) return prev.data;
		if (!res.ok) throw new ApiError(res.status, path);
		const data: unknown = await res.json();
		cache.set(path, { etag: res.headers.get('etag'), data });
		return data;
	}

	function shared(path: string, revalidate: boolean): Promise<unknown> {
		const running = inflight.get(path);
		if (running) return running;
		const p = load(path, revalidate).finally(() => inflight.delete(path));
		inflight.set(path, p);
		return p;
	}

	async function getJson<T>(path: string): Promise<T> {
		const res = await fetchFn(path);
		if (!res.ok) throw new ApiError(res.status, path);
		return (await res.json()) as T;
	}

	async function postJson<T>(path: string, body: unknown): Promise<T> {
		const res = await fetchFn(path, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(body)
		});
		if (!res.ok) throw new ApiError(res.status, path);
		return (await res.json()) as T;
	}

	return {
		async table<T extends TableName>(name: T): Promise<TableTypes[T][]> {
			const path = `/api/${name}`;
			const hit = cache.get(path);
			if (hit) return hit.data as TableTypes[T][];
			return (await shared(path, false)) as TableTypes[T][];
		},
		async revalidate<T extends TableName>(name: T): Promise<TableTypes[T][]> {
			return (await shared(`/api/${name}`, true)) as TableTypes[T][];
		},
		/** Computed values depend on the inputs, so these are fetched every time and not cached. */
		async shopPrices(shop: number, charm: number): Promise<ShopPrices> {
			const path = `/api/shops/${shop}/prices?charm=${charm}`;
			const res = await fetchFn(path);
			if (!res.ok) throw new ApiError(res.status, path);
			return (await res.json()) as ShopPrices;
		},
		async expTable(q: ExpQuery): Promise<ExpTable> {
			const path = expPath(q);
			const res = await fetchFn(path);
			if (!res.ok) throw new ApiError(res.status, path);
			return (await res.json()) as ExpTable;
		},
		async classRaceBuild(cls: number, race: number): Promise<ClassRaceBuild> {
			return getJson(builderPath(cls, race));
		},
		async builderEquip(cls: number, race: number): Promise<BuilderEquip> {
			return postJson('/api/class-race-builder/equip', { class: cls, race });
		},
		async builderSpellbook(cls: number): Promise<BuilderSpellbook> {
			return postJson('/api/class-race-builder/spellbook', { class: cls });
		},
		async spellCounters(req: CountersRequest): Promise<Counters> {
			return postJson('/api/spells/counters', req);
		},
		// the hide_record_numbers setting decides whether names carry their numbers
		async spellCasts(n: number): Promise<SpellCasts> {
			return getJson(`/api/spells/${n}/casts`);
		},
		async itemChest(n: number): Promise<Chest> {
			return getJson(`/api/items/${n}/chest`);
		},
		/** `GetAbilityList`: the items of the ability combos, in number order. */
		async abilityList(): Promise<AbilityName[]> {
			return (await getJson<{ abilities: AbilityName[] }>('/api/abilities')).abilities;
		},
		async spellBookList(req: SpellBookRequest): Promise<SpellBookList> {
			return postJson('/api/spell-book/list', req);
		},
		async spellBookClass(cls: number): Promise<SpellBookClass> {
			return postJson('/api/spell-book/class', { class: cls });
		},
		async spellBookCopy(req: {
			filter_on: boolean;
			alignment: number;
			level_text: string;
			class_index: number;
			class: number;
		}): Promise<SpellBookCopy> {
			return postJson('/api/spell-book/copy-global', req);
		},
		async abilities(table: AbilityTable, n: number): Promise<Abilities> {
			return getJson(abilitiesPath(table, n));
		},
		async spellEffect(n: number, level?: number): Promise<SpellEffect> {
			return getJson(spellEffectPath(n, level));
		},
		async monsterMaxDamage(n: number): Promise<MonsterMaxDamage> {
			return getJson(maxDamagePath(n));
		},
		/** `GET /api/reachability/status`: whether `Options > Calculate Reachability` has been run for this load. */
		async reachabilityStatus(): Promise<ReachabilityStatus> {
			return getJson('/api/reachability/status');
		},
		/** `POST /api/reachability/compute`: `ReachRecompute`. */
		async computeReachability(): Promise<ReachabilityStatus> {
			return postJson('/api/reachability/compute', {});
		},
		async info(): Promise<Info> {
			return (await shared('/api/info', false)) as Info;
		},
		/** Test hook: whether the table is already held. */
		has(name: TableName): boolean {
			return cache.has(`/api/${name}`);
		}
	};
}

export const api = createApi();

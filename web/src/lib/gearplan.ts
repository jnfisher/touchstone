// The Gear Planner (touchstone's own; MME's nearest tool is Find Best): the request the form makes, the
// response's shape, and the lists the page builds from it, among them the kill list for the suggested gear.
import { targetOf, type TargetField } from './weaponsim';

export const GOALS = [
	{ value: 'melee', label: 'Melee damage', hint: 'Damage a round of a normal attack with the weapon in hand', swings: true },
	{ value: 'backstab', label: 'Backstab', hint: 'Damage of a backstab with the weapon in hand', swings: true },
	{ value: 'armour', label: 'Armour class', hint: 'Armour class, then damage resistance', swings: false },
	{ value: 'dodge', label: 'Dodge', hint: 'Dodge, then armour class', swings: false },
	{ value: 'caster', label: 'Spellcasting', hint: 'Spellcasting, then spell damage and mana', swings: false },
	{ value: 'mana_regen', label: 'Mana regen', hint: 'Mana regeneration, then mana', swings: false },
	{ value: 'hit_points', label: 'Hit points', hint: 'Hit points, then their regeneration', swings: false },
	{ value: 'stealth', label: 'Stealth', hint: 'Stealth, then dodge', swings: false }
] as const;
export type GoalValue = (typeof GOALS)[number]['value'];

/** Whether a goal swings at the target (the others do not read it). */
export function goalSwings(goal: GoalValue): boolean {
	return GOALS.find((g) => g.value === goal)?.swings ?? false;
}

/** The target boxes the damage goals read (no hit points: the planner does not fight). */
export const PLAN_TARGET_FIELDS: TargetField[] = ['ac', 'dr', 'dodge', 'bs_defense'];

export interface GearPlanForm {
	goal: GoalValue;
	ignoreLevel: boolean;
	ignoreAlignment: boolean;
	includeLimited: boolean;
	onlyObtainable: boolean;
	secondWrist: boolean;
	/** Limits as typed; empty for none. */
	minAc: string;
	maxEncum: string;
	target: Record<TargetField, string>;
	monster: number | null;
	/** Slots to leave as they are. */
	locked: number[];
	/** Items the plan must do without, in the order they were skipped ("next best"). */
	skipped: number[];
}

/** The most items a plan may skip (the server's limit). */
export const MAX_SKIPPED = 200;

/** "Next best": the list with `item` skipped too; unchanged if it already is or the list is full. */
export function skipItem(skipped: number[], item: number): number[] {
	return skipped.includes(item) || skipped.length >= MAX_SKIPPED ? skipped : [...skipped, item];
}

export const GEAR_PLAN_DEFAULTS: GearPlanForm = {
	goal: 'melee',
	ignoreLevel: false,
	ignoreAlignment: false,
	includeLimited: true,
	onlyObtainable: true,
	secondWrist: false,
	minAc: '',
	maxEncum: '',
	target: { ac: '0', dr: '0', dodge: '0', bs_defense: '0', hp: '0' },
	monster: null,
	locked: [],
	skipped: []
};

/** A limit box: a number of 0 or more, or nothing. */
function limit(text: string): number | undefined {
	const t = text.trim();
	if (t === '') return undefined;
	const n = Number(t);
	return Number.isFinite(n) && n >= 0 ? Math.min(n, 100_000) : undefined;
}

/** `POST /api/calc/gear-plan`; `character` is the evaluate body of the character in use. */
export function gearPlanRequest(form: GearPlanForm, character: object) {
	const { ac, dr, dodge, bs_defense } = targetOf(form);
	const minAc = limit(form.minAc);
	const maxEncum = limit(form.maxEncum);
	return {
		character,
		goal: form.goal,
		ignore_level: form.ignoreLevel,
		ignore_alignment: form.ignoreAlignment,
		include_limited: form.includeLimited,
		only_obtainable: form.onlyObtainable,
		second_wrist: form.secondWrist,
		// only the damage goals swing at something
		target: goalSwings(form.goal) ? { ac, dr, dodge, bs_defense } : { ac: 0, dr: 0, dodge: 0, bs_defense: 0 },
		...(minAc === undefined ? {} : { min_ac: minAc }),
		...(maxEncum === undefined ? {} : { max_encum: maxEncum }),
		locked: [...form.locked].sort((a, b) => a - b),
		...(form.skipped.length ? { skipped: form.skipped.slice(0, MAX_SKIPPED) } : {})
	};
}

// ---------- the response ----------

export interface PlanItem {
	number: number;
	name: string;
	level: number;
	armour_class: number;
	damage_resist: number;
	encum: number;
	limited: boolean;
	needs_level: boolean;
	wrong_alignment: boolean;
	unobtainable: boolean;
}

export type SourceKind = 'monster' | 'chest' | 'shop' | 'room' | 'npc' | 'script';

export interface Source {
	kind: SourceKind;
	number: number;
	map: number;
	name: string;
	pct: number | null;
	kills: number | null;
	chest: number | null;
	chest_name: string | null;
	chest_pct: number | null;
	one_time: boolean;
}

export interface Acquisition {
	item: number;
	name: string;
	sources: Source[];
	more: number;
	orphan_chests: number;
}

export type ClosedReason = 'locked' | 'weapon' | 'two_handed' | 'second_wrist';

export interface PlanSlot {
	slot: number;
	name: string;
	closed: ClosedReason | null;
	current: PlanItem | null;
	suggested: PlanItem | null;
	changed: boolean;
	gain: number;
	alternatives: { item: PlanItem; delta: number }[];
	acquisition: Acquisition | null;
}

export interface GearPlanResult {
	goal: GoalValue;
	metric: string;
	before: number;
	after: number;
	slots: PlanSlot[];
	stats: { label: string; before: number; after: number }[];
	candidates: number;
	evaluations: number;
	passes: number;
	level: number;
	skipped: { number: number; name: string }[];
	unmet: string[];
	elapsed_ms: number;
}

// ---------- reading a plan ----------

const CLOSED_TEXT: Record<ClosedReason, string> = {
	locked: 'locked',
	weapon: 'not planned (see the Weapon Simulator)',
	two_handed: 'the weapon takes both hands',
	second_wrist: 'second wrist is off'
};

export function closedText(r: ClosedReason | null): string {
	return r ? CLOSED_TEXT[r] : '';
}

/** A number as the page shows it: whole when it is, else one decimal. */
export function num(n: number): string {
	return Number.isInteger(n) ? String(n) : n.toFixed(1);
}

export function signed(n: number): string {
	return n > 0 ? `+${num(n)}` : num(n);
}

/** "+53%" from before to after; empty when there is nothing to compare with. */
export function changePct(before: number, after: number): string {
	if (before <= 0 || after === before) return '';
	const p = Math.round(((after - before) / before) * 100);
	return `${p > 0 ? '+' : ''}${p}%`;
}

/** What stops the character wearing the item today, and what kind of item it is. */
export function itemNote(i: PlanItem | null, level: number): string {
	if (!i) return '';
	const notes: string[] = [];
	if (i.needs_level) notes.push(`level ${i.level}${i.level > level ? ` (+${i.level - level})` : ''}`);
	if (i.wrong_alignment) notes.push('alignment');
	if (i.limited) notes.push('limited');
	if (i.unobtainable) notes.push('no known source');
	return notes.join(', ');
}

export type SlotOrder = 'slot' | 'gain';

/** The slots as listed: in MME's order, or the swaps worth most on their own first. */
export function orderedSlots(plan: GearPlanResult, order: SlotOrder): PlanSlot[] {
	if (order === 'slot') return plan.slots;
	return [...plan.slots].sort(
		(a, b) => Number(b.changed) - Number(a.changed) || b.gain - a.gain || a.slot - b.slot
	);
}

export function changedSlots(plan: GearPlanResult): PlanSlot[] {
	return plan.slots.filter((s) => s.changed);
}

/** "5% (about 20 kills)", "0.3% (about 364 kills)". */
export function chanceText(s: Pick<Source, 'pct' | 'kills'>): string {
	if (s.pct === null) return '';
	const pct = `${s.pct >= 10 ? Math.round(s.pct) : Number(s.pct.toFixed(s.pct < 1 ? 2 : 1))}%`;
	if (s.kills === null || s.pct >= 100) return pct;
	return `${pct} (about ${Math.max(1, Math.round(s.kills)).toLocaleString('en-US')} kills)`;
}

/** One wanted item a monster can yield. */
export interface Drop {
	item: number;
	itemName: string;
	slot: string;
	pct: number;
	kills: number | null;
	/** Set when the item comes out of a chest the monster drops. */
	chestName: string | null;
	chestPct: number | null;
}

export interface KillTarget {
	monster: number;
	name: string;
	drops: Drop[];
}

/**
 * The kill list: every monster that drops a suggested item (or a chest holding one), with what it gives. A
 * monster that yields more of the wanted items comes first, then the one with the surest drop.
 */
export function killList(plan: GearPlanResult): KillTarget[] {
	const by = new Map<number, KillTarget>();
	for (const s of changedSlots(plan)) {
		for (const src of s.acquisition?.sources ?? []) {
			if ((src.kind !== 'monster' && src.kind !== 'chest') || src.pct === null) continue;
			let t = by.get(src.number);
			if (!t) by.set(src.number, (t = { monster: src.number, name: src.name, drops: [] }));
			// one line per item: the surer of a straight drop and a chest
			const had = t.drops.find((d) => d.item === s.acquisition!.item);
			const drop: Drop = {
				item: s.acquisition!.item,
				itemName: s.acquisition!.name,
				slot: s.name,
				pct: src.pct,
				kills: src.kills,
				chestName: src.chest_name,
				chestPct: src.chest_pct
			};
			if (!had) t.drops.push(drop);
			else if (drop.pct > had.pct) Object.assign(had, drop);
		}
	}
	const best = (t: KillTarget) => Math.max(...t.drops.map((d) => d.pct));
	return [...by.values()]
		.map((t) => ({ ...t, drops: [...t.drops].sort((a, b) => b.pct - a.pct) }))
		.sort((a, b) => b.drops.length - a.drops.length || best(b) - best(a) || a.monster - b.monster);
}

export interface ShopStop {
	shop: number;
	name: string;
	items: { item: number; itemName: string; slot: string; oneTime: boolean }[];
}

/** The shops that sell suggested items, the one selling most of them first. */
export function shopList(plan: GearPlanResult): ShopStop[] {
	const by = new Map<number, ShopStop>();
	for (const s of changedSlots(plan)) {
		for (const src of s.acquisition?.sources ?? []) {
			if (src.kind !== 'shop') continue;
			let stop = by.get(src.number);
			if (!stop) by.set(src.number, (stop = { shop: src.number, name: src.name, items: [] }));
			if (!stop.items.some((i) => i.item === s.acquisition!.item)) {
				stop.items.push({ item: s.acquisition!.item, itemName: s.acquisition!.name, slot: s.name, oneTime: src.one_time });
			}
		}
	}
	return [...by.values()].sort((a, b) => b.items.length - a.items.length || a.shop - b.shop);
}

/** A source that is neither a kill nor a shop, worded. */
export function otherText(s: Source): string {
	switch (s.kind) {
		case 'room':
			return `Room ${s.map}/${s.number}${s.name ? `: ${s.name}` : ''}`;
		case 'npc':
			return `Given by ${s.name}`;
		case 'script':
			return `A script gives it${s.pct === null ? '' : ` (${chanceText({ pct: s.pct, kills: null })})`}`;
		default:
			return s.name;
	}
}

export interface OtherWay {
	item: number;
	itemName: string;
	slot: string;
	sources: Source[];
	/** Nothing at all is listed for the item, by kill, shop or otherwise. */
	nothing: boolean;
}

/** Per suggested item, the rooms, people and scripts that yield it; and the items with no source at all. */
export function otherWays(plan: GearPlanResult): OtherWay[] {
	return changedSlots(plan)
		.filter((s) => s.acquisition)
		.map((s) => {
			const a = s.acquisition!;
			const sources = a.sources.filter((x) => x.kind === 'room' || x.kind === 'npc' || x.kind === 'script');
			return { item: a.item, itemName: a.name, slot: s.name, sources, nothing: a.sources.length === 0 };
		})
		.filter((w) => w.sources.length > 0 || w.nothing);
}

// ---------- transport ----------

type FetchFn = typeof fetch;

export function createGearPlanClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	return {
		async run(body: ReturnType<typeof gearPlanRequest>): Promise<GearPlanResult> {
			const res = await fetchFn('/api/calc/gear-plan', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body)
			});
			if (!res.ok) {
				const reason = (await res.text().catch(() => '')).trim();
				throw new Error(reason || `HTTP ${res.status}`);
			}
			return (await res.json()) as GearPlanResult;
		}
	};
}

export const gearPlanClient = createGearPlanClient();

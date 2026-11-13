// The item popover (touchstone's own): what hovering an item's name shows. The facts come from the items table
// the page already holds; the abilities, the damage line and the comparison with what is worn come from the
// item's detail pane (`/api/detail/items/{n}`), which is MME's text for the active character.
import type { Item } from './api';
import type { Pane } from './detail';
import { armourType, itemType, weaponType, wornType } from './enums';

export interface HoverLine {
	label: string;
	value: string;
}

/** AC and DR are stored in tenths. */
const tenths = (n: number) => String(n / 10);

/** The item's own numbers, the ones that matter for its kind; zeroes that say nothing are left out. */
export function itemFacts(i: Item): HoverLine[] {
	const lines: HoverLine[] = [];
	const add = (label: string, value: string | number) => lines.push({ label, value: String(value) });
	if (i.item_type === 1) {
		add('Weapon', weaponType(i.weapon_type));
		add('Base damage', `${i.min} to ${i.max}`);
		add('Speed', i.speed);
		if (i.accy) add('Accuracy', i.accy);
		if (i.str_req) add('Strength', i.str_req);
		if (i.armour_class || i.damage_resist) add('AC / DR', `${tenths(i.armour_class)} / ${tenths(i.damage_resist)}`);
	} else if (i.item_type === 0) {
		add('Worn', wornType(i.worn));
		add('Armour', armourType(i.armour_type));
		add('AC / DR', `${tenths(i.armour_class)} / ${tenths(i.damage_resist)}`);
		if (i.accy) add('Accuracy', i.accy);
	} else {
		add('Type', itemType(i.item_type));
		if (i.use_count > 0) add('Uses', i.use_count);
	}
	add('Weight', i.encum);
	if (i.limit) add('Limit', i.limit);
	return lines;
}

/** The pane's labelled rows (damage, abilities, casts, class and race limits) and its comparisons, in order. */
export function paneLines(p: Pane): HoverLine[] {
	return [
		...p.rows.filter((r) => r.label && r.value).map((r) => ({ label: r.label, value: r.value })),
		...p.compare.map((c) => ({ label: c.label, value: c.text }))
	];
}

export interface Box {
	left: number;
	top: number;
	width: number;
	height: number;
}

/**
 * Where the card goes: under the name, left edges aligned; above it when there is no room below; always
 * inside the window with a small margin. For a row of an open list (`list` is the list's box) it goes beside
 * the list instead, level with the row, so it never covers the rows being chosen from.
 */
export function placeCard(
	anchor: Box,
	card: { width: number; height: number },
	view: { width: number; height: number },
	list?: Box
): { left: number; top: number } {
	const gap = 6;
	const margin = 8;
	if (list) {
		const right = list.left + list.width + gap;
		const left = right + card.width <= view.width - margin ? right : Math.max(margin, list.left - gap - card.width);
		const top = Math.max(margin, Math.min(anchor.top, view.height - margin - card.height));
		return { left, top };
	}
	const below = anchor.top + anchor.height + gap;
	const top = below + card.height <= view.height - margin ? below : Math.max(margin, anchor.top - gap - card.height);
	const left = Math.max(margin, Math.min(anchor.left, view.width - margin - card.width));
	return { left, top };
}

/** The item a hovered element names: its own `data-item`, or the row of a grid of items when over the name cell. */
export function hoveredItem(el: Element | null): { item: number; anchor: Element } | undefined {
	if (!el) return undefined;
	const tagged = el.closest('[data-item]');
	if (tagged) {
		const n = Number(tagged.getAttribute('data-item'));
		return Number.isInteger(n) && n > 0 ? { item: n, anchor: tagged } : undefined;
	}
	const cell = el.closest('[data-item-rows] .ag-cell[col-id="name"]');
	const row = cell?.closest('.ag-row');
	const n = Number(row?.getAttribute('row-id'));
	return cell && Number.isInteger(n) && n > 0 ? { item: n, anchor: cell } : undefined;
}

// What each context-menu item does (frmMain `mnuItemsPopUpItem_Click`, `mnuSpellsPopUpItem_Click`,
// `mnuAuxPopUpItem_Click`), wired to the page's stores. Every rule runs on the server; this file
// only routes: the compare lists, the character, the Item Manager, the clipboard texts, and the
// pages the calculators and combat setup open on.
import { goto } from '$app/navigation';
import { characterClient, BLESS_SLOT_COUNT } from './character';
import { getCharacterStore, type CharacterStore } from './character.svelte';
import { clipboardClient, deliver, type ClipColumn, type ClipRow, type ClipResult } from './clipboard';
import { compareClient, type CompareKind } from './compare';
import { getCompareStore, type CompareStore } from './compare.svelte';
import { LV_NAME, menuKind, type ActionId, type MenuList } from './contextmenu';
import { calcPrefill, combatPrefill, vsMob } from './handoff.svelte';
import { dialogs } from './dialogs.svelte';
import { selection } from './explorer-state.svelte';
import { createItemManager } from './itemmanager.svelte';
import { getSettingsStore } from './settings.svelte';

/** The record a menu was opened on, and the cells of its row. */
export interface MenuTarget {
	list: MenuList;
	/** The record number (item, spell, monster, shop, class or race); the Item Manager's row item number. */
	number?: number;
	/** The row's headers and cell texts, the `#` column first. */
	columns?: string[];
	cells?: string[];
	/** The detail box text and reference list of the row (the pane the page shows for it). */
	detail?: string;
	references?: ClipRow[];
	/** The monster pane's lines, for copying a monster. */
	lines?: ClipRow[];
	/** A pane line the menu was opened on: what it stands for. */
	line?: { kind: string; number: number };
	/** The shop boxes (charm, "what the shop pays"). */
	shop?: { charm: number; showSell: boolean };
	/** Monsters: the row is a lair-only group, and the numbers it shows. */
	lairStats?: boolean;
	/** A multi-selection (the Item Manager): every selected row's item number, and its cells. */
	numbers?: number[];
	rows?: string[][];
}

export interface Deps {
	chars?: CharacterStore;
	cmp?: CompareStore;
	go?: (to: string) => unknown;
	/** Options > Jump to Equip/Lists on Add (`mnuJumpToCompare.Checked`). */
	jump?: () => boolean;
}

const COMPARE_OF_LIST: Partial<Record<MenuList, CompareKind>> = {
	weapons: 'weapons',
	weaponCompare: 'weapons',
	armour: 'armour',
	armourCompare: 'armour',
	spells: 'spells',
	spellBook: 'spells',
	spellCompare: 'spells',
	monsters: 'monsters',
	monsterCompare: 'monsters'
};

/** MME's list columns: the width a column has when it shows (0 hides it). */
function columnsOf(t: MenuTarget): ClipColumn[] {
	return (t.columns ?? []).map((text) => ({ text, width: 1000 }));
}

function rowOf(t: MenuTarget): ClipRow {
	const cells = t.cells ?? [];
	return {
		text: cells[0] ?? '',
		subs: cells.slice(1),
		selected: true,
		...(t.detail !== undefined ? { detail: t.detail } : {}),
		...(t.references ? { refs: t.references } : {})
	};
}

export function createMenuActions(deps: Deps = {}) {
	const chars = deps.chars ?? getCharacterStore();
	const cmp = deps.cmp ?? getCompareStore();
	const go = deps.go ?? ((to: string) => goto(to));
	const jump = deps.jump ?? (() => getSettingsStore().current.jump_to_compare);

	/**
	 * `CompareAddItem` and its siblings: on the list means off it, otherwise on the end; an add (not a removal)
	 * opens the Compare tab on that list when the Options item is checked.
	 */
	async function addToCompare(kind: CompareKind, n: number) {
		const had = cmp.lists[kind].includes(n);
		cmp.toggle(kind, n);
		if (had || !jump()) return;
		cmp.setActive?.(kind);
		await go('/compare');
	}

	async function copy(res: ClipResult): Promise<string> {
		const out = await deliver(res);
		if (out.fallback !== undefined) {
			dialogs.showText(out.fallback, "Your browser would not copy for you; select the text and copy it.");
			return out.error ?? 'Copy blocked by the browser';
		}
		if (out.error) return out.error;
		return out.empty ? 'Nothing to copy' : 'Copied to the clipboard';
	}

	/** `CopyLVLinetoClipboard` for the row, with MME's per-list arguments. */
	function linesFor(t: MenuTarget, nameOnly: boolean): Promise<ClipResult> {
		const nameCol = LV_NAME[t.list];
		let exclude: number | undefined;
		let useDetail = !nameOnly;
		// the exclude argument of each menu's calls: the spell lists skip column 7, reference lists column 0
		if (menuKind(t.list) === 'spells') exclude = 7;
		else if (/Loc$/.test(nameCol) && !['lvMapLoc', 'lvSpellLoc', 'lvShopLoc', 'lvSpellCompareLoc'].includes(nameCol)) exclude = 0;
		if (menuKind(t.list) === 'aux' && !/^lv(OtherItems)$/.test(nameCol)) useDetail = false;
		return clipboardClient.lines({
			list: nameCol,
			columns: columnsOf(t),
			rows: t.rows ? t.rows.map((c) => ({ text: c[0] ?? '', subs: c.slice(1), selected: true })) : [rowOf(t)],
			...(useDetail && t.detail !== undefined ? { detail: t.detail } : {}),
			...(useDetail && t.references ? { locations: t.references } : {}),
			...(exclude !== undefined ? { exclude_column: exclude } : {}),
			name_only: nameOnly
		});
	}

	async function toggleList(kind: CompareKind, n: number, remove: boolean) {
		if (remove) {
			const at = cmp.lists[kind].indexOf(n);
			if (at >= 0) cmp.removeAt(kind, at);
		} else {
			await addToCompare(kind, n);
		}
	}

	async function addItemToCompare(n: number) {
		const [c] = await compareClient.classify([n]);
		if (c?.kind === 'weapon') await addToCompare('weapons', n);
		else if (c?.kind === 'armour') await addToCompare('armour', n);
	}

	async function addToItemManager(numbers: number[]): Promise<string> {
		const im = createItemManager(chars);
		for (const n of numbers) await im.add(n);
		return im.problem ?? im.note ?? 'Added to the Item Manager';
	}

	function setBless(n: number): string {
		const slots = [...chars.doc.bless];
		if (slots.includes(n)) return 'Already a bless spell';
		const free = slots.findIndex((s) => s === 0);
		if (free < 0 || free >= BLESS_SLOT_COUNT) return 'No open bless slot';
		chars.edit((d) => {
			d.bless = d.bless.map((s, i) => (i === free ? n : s));
		});
		return 'Set as a bless spell';
	}

	function learn(n: number): string {
		const has = chars.doc.learned_spells.includes(n);
		chars.edit((d) => {
			d.learned_spells = has ? d.learned_spells.filter((s) => s !== n) : [...d.learned_spells, n];
		});
		return has ? 'Marked as not learned' : 'Marked as learned';
	}

	async function equip(n: number): Promise<string> {
		const out = await characterClient.equip({
			character: { ...chars.doc, options: chars.options },
			item: n,
			unequip: true
		});
		chars.edit((d) => (d.equipped = out.character.equipped));
		return out.messages.join(' ') || (out.changed ? 'Equipped' : 'No change');
	}

	/** The monster's numbers for "Set ... Combat Calc. to Mob": row cells 5 (AC/DR), 6 (dodge), 7 (MR). */
	function monsterNumbers(t: MenuTarget, weapons: boolean) {
		const c = t.cells ?? [];
		let [ac, dr] = (c[5] ?? '').split('/');
		let dodge = c[6] ?? '';
		let mr = c[7] ?? '';
		if (t.lairStats) {
			// a lair-only group: the averages on the monster pane
			const line = (label: string) => t.lines?.find((l) => l.text === label)?.subs[0] ?? '';
			if (weapons) {
				const [a, d] = line('AVG AC/DR').split('/');
				ac = a ?? '';
				dr = d ?? '';
				dodge = line('AVG Dodge');
			} else {
				mr = line('AVG MR');
			}
		}
		return { name: c[1] ?? '', ac: ac ?? '', dr: dr ?? '', dodge, mr };
	}

	return {
		addToCompare,
		/** Runs one menu item; resolves to a short message for the page to show. */
		async run(id: ActionId, t: MenuTarget): Promise<string | undefined> {
			const n = t.number ?? 0;
			const kind = COMPARE_OF_LIST[t.list];
			const removeOnly = t.list.endsWith('Compare');
			switch (id) {
				case 'equip': {
					const msgs: string[] = [];
					for (const k of t.numbers?.length ? t.numbers : [n]) msgs.push(await equip(k));
					return msgs.join(' ');
				}
				case 'saveList':
					if (t.list === 'itemManager') for (const k of t.numbers?.length ? t.numbers : [n]) await addItemToCompare(k);
					else if (kind) await toggleList(kind, n, removeOnly);
					return undefined;
				case 'monsterList':
					if (t.list === 'monsterDetail' || t.list === 'monsterCompareLoc') {
						const l = t.line;
						if (!l) return undefined;
						if (l.kind === 'monster') await addToCompare('monsters', l.number);
						else if (l.kind === 'item') await addItemToCompare(l.number);
						else if (l.kind === 'spell') await addToCompare('spells', l.number);
					} else {
						await toggleList('monsters', n, t.list === 'monsterCompare');
					}
					return undefined;
				case 'copyNames':
					return copy(await linesFor(t, true));
				case 'copyDetails':
					return copy(await linesFor(t, false));
				case 'copyName':
				case 'copyDetail': {
					const nameOnly = id === 'copyName';
					if (t.list === 'shops') {
						return copy(await clipboardClient.shop(n, { ...t.shop, nameOnly }));
					}
					if (t.list === 'monsters' || t.list === 'monsterCompare' || t.list === 'monsterDetail' || t.list === 'monsterCompareLoc') {
						if (nameOnly && t.line) return copy({ text: t.cells?.[0] ?? null, error: null });
						return copy(await clipboardClient.monster(t.lines ?? [], nameOnly));
					}
					return copy(await linesFor(t, nameOnly));
				}
				case 'calcSwings':
					calcPrefill.set('swing', { weapon: n });
					await go('/calc/swing');
					return undefined;
				case 'calcBackstab':
					calcPrefill.set('backstab', { weapon: n });
					await go('/calc/backstab');
					return undefined;
				case 'setBackstabWeapon':
					combatPrefill.set({ backstab: n });
					await go('/character');
					return undefined;
				case 'setCombatSpell':
					combatPrefill.set({ spell: n });
					await go('/character');
					return undefined;
				case 'addToItemManager':
					return addToItemManager(t.numbers?.length ? t.numbers : n > 0 ? [n] : []);
				case 'learn':
					return learn(n);
				case 'setBless':
					return setBless(n);
				case 'lookUpMonster':
					// the monster pane's "Spawns via" lines are what MME's lookup lists
					selection.set('monsters', n);
					await go('/monsters');
					return undefined;
				case 'hitCalc': {
					const side = await dialogs.ask('Hit Calc Pop-Up', 'Mob as attacker [yes] or defender [no]?', [
						{ value: 'attacker', label: 'Yes (attacker)' },
						{ value: 'defender', label: 'No (defender)' }
					]);
					if (!side) return undefined;
					calcPrefill.set('hit', { monster: n, side: side as 'attacker' | 'defender' });
					await go('/calc/hit');
					return undefined;
				}
				case 'setWeaponCalc': {
					vsMob.setWeapons(monsterNumbers(t, true));
					await go('/weapons');
					return undefined;
				}
				case 'setSpellCalc': {
					vsMob.setSpells(monsterNumbers(t, false));
					await go('/spells');
					return undefined;
				}
				default:
					return undefined;
			}
		}
	};
}

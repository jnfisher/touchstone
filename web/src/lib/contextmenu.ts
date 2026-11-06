// MME's right-click menus as data: which items a list's menu shows, in which order and under which
// caption (frmMain `PopUpItemsMenu`, `PopUpSpellsMenu`, `PopUpAuxMenu`). Captions are the menu items'
// design-time ones plus the ones those procedures assign at run time. Nothing here touches the page;
// `menuactions.svelte.ts` runs the chosen item.

/** MME's list names, by the menu each opens. */
export type MenuList =
	| 'weapons'
	| 'weaponCompare'
	| 'armour'
	| 'armourCompare'
	| 'itemManager'
	| 'spells'
	| 'spellCompare'
	| 'spellBook'
	| 'monsters'
	| 'monsterCompare'
	| 'monsterDetail'
	| 'monsterCompareLoc'
	| 'shopDetail'
	| 'otherItems'
	| 'shops'
	| 'classes'
	| 'races'
	| 'weaponLoc'
	| 'armourLoc'
	| 'spellLoc'
	| 'shopLoc'
	| 'mapLoc'
	| 'weaponCompareLoc'
	| 'armourCompareLoc'
	| 'spellCompareLoc'
	| 'itemManagerLoc'
	| 'otherItemLoc';

/** The `lv...` name MME gives each list (`CopyLVLinetoClipboard` reads it). */
export const LV_NAME: Record<MenuList, string> = {
	weapons: 'lvWeapons',
	weaponCompare: 'lvWeaponCompare',
	armour: 'lvArmour',
	armourCompare: 'lvArmourCompare',
	itemManager: 'lvItemManager',
	spells: 'lvSpells',
	spellCompare: 'lvSpellCompare',
	spellBook: 'lvSpellBook',
	monsters: 'lvMonsters',
	monsterCompare: 'lvMonsterCompare',
	monsterDetail: 'lvMonsterDetail',
	monsterCompareLoc: 'lvMonsterCompareLoc',
	shopDetail: 'lvShopDetail',
	otherItems: 'lvOtherItems',
	shops: 'lvShops',
	classes: 'lvClasses',
	races: 'lvRaces',
	weaponLoc: 'lvWeaponLoc',
	armourLoc: 'lvArmourLoc',
	spellLoc: 'lvSpellLoc',
	shopLoc: 'lvShopLoc',
	mapLoc: 'lvMapLoc',
	weaponCompareLoc: 'lvWeaponCompareLoc',
	armourCompareLoc: 'lvArmourCompareLoc',
	spellCompareLoc: 'lvSpellCompareLoc',
	itemManagerLoc: 'lvItemManagerLoc',
	otherItemLoc: 'lvOtherItemLoc'
};

export type MenuKind = 'items' | 'spells' | 'aux';

const ITEMS_LISTS: readonly MenuList[] = ['weapons', 'weaponCompare', 'armour', 'armourCompare', 'itemManager'];
const SPELL_LISTS: readonly MenuList[] = ['spells', 'spellCompare', 'spellBook'];

/** Which of MME's three menus a list opens (`lvWeapons_MouseUp` and the others). */
export function menuKind(list: MenuList): MenuKind {
	if ((ITEMS_LISTS as readonly string[]).includes(list)) return 'items';
	if ((SPELL_LISTS as readonly string[]).includes(list)) return 'spells';
	return 'aux';
}

export type ActionId =
	// the items menu
	| 'equip'
	| 'saveList'
	| 'copyNames'
	| 'copyDetails'
	| 'calcSwings'
	| 'calcBackstab'
	| 'setBackstabWeapon'
	| 'addToItemManager'
	// the spells menu
	| 'whatCasts'
	| 'learn'
	| 'findCounters'
	| 'setCombatSpell'
	| 'setBless'
	// the aux menu
	| 'copyDetail'
	| 'copyName'
	| 'lookUpMonster'
	| 'addShopItems'
	| 'monsterList'
	| 'copyChest'
	| 'recalcDamage'
	| 'hitCalc'
	| 'setWeaponCalc'
	| 'setSpellCalc'
	| 'setShop';

export interface MenuItem {
	id: ActionId;
	label: string;
	/** Present when the item is shown but cannot run yet: the tooltip says why. */
	disabled?: string;
}

/** What the menu depends on besides the list. */
export interface MenuContext {
	list: MenuList;
	/** Monsters: the row is a lair-only group (NMR 1.83+, lair mode, no regen, some lairs), so MME says "LAIR Stats". */
	lairStats?: boolean;
	/** `bAutoCalcMonDamage`: touchstone always calculates, so the re-calculate item stays hidden. */
	autoCalcDamage?: boolean;
	/** The Sundry tab's chest button is enabled (the selected item is a chest). */
	chestsEnabled?: boolean;
}

export const NO_CHEST_HERE = 'MME does nothing for a row of the chest list.';
export const NO_SHOP_ASSIGN = "Assigning a shop to Item Manager rows is not in touchstone yet.";
export const NO_ADD_SHOP_ITEMS = 'MME never shows this item.';

const REMOVE_ONLY: readonly MenuList[] = [
	'weaponCompare',
	'armourCompare',
	'spellCompare',
	'monsterCompare'
];

function itemsMenu(list: MenuList): MenuItem[] {
	const weapon = list === 'weapons' || list === 'weaponCompare';
	const out: MenuItem[] = [
		{ id: 'equip', label: 'Equip / Unequip Item' },
		{
			id: 'saveList',
			label: (REMOVE_ONLY as readonly string[]).includes(list) ? 'Remove From Save List' : 'Add/Remove Save List'
		},
		{ id: 'copyNames', label: 'Copy Name(s) to Clipboard' },
		{ id: 'copyDetails', label: 'Copy Details to Clipboard' }
	];
	if (weapon) {
		out.push(
			{ id: 'calcSwings', label: 'Calculate Swings' },
			{ id: 'calcBackstab', label: 'Calculate Backstab Damage' },
			{ id: 'setBackstabWeapon', label: 'Set Combat Backstab Weapon' }
		);
	}
	// the Item Manager's own menu has no "Add to Item Manager"
	if (list !== 'itemManager') out.push({ id: 'addToItemManager', label: 'Add to Item Manager' });
	return out;
}

function spellsMenu(list: MenuList): MenuItem[] {
	return [
		{
			id: 'saveList',
			label: list === 'spellCompare' ? 'Remove from Save List' : 'Add/Remove Save List'
		},
		{ id: 'copyDetails', label: 'Copy Details to Clipboard' },
		{ id: 'copyNames', label: 'Copy Name(s) to Clipboard' },
		{ id: 'whatCasts', label: 'What casts this spell?' },
		{ id: 'learn', label: 'Mark/Unmark Spell as Learned' },
		{ id: 'findCounters', label: 'Find counters (negates/dispells)' },
		{ id: 'setCombatSpell', label: 'Set as Combat Attack/Heal Spell' },
		{ id: 'setBless', label: 'Set as Bless Spell' }
	];
}

const LOC_LISTS: readonly MenuList[] = [
	'mapLoc',
	'weaponLoc',
	'armourLoc',
	'spellLoc',
	'shopLoc',
	'weaponCompareLoc',
	'armourCompareLoc',
	'spellCompareLoc'
];

function auxMenu(c: MenuContext): MenuItem[] {
	const { list } = c;
	const detail = 'Copy Details to Clipboard';
	switch (list) {
		case 'monsters':
		case 'monsterCompare':
		case 'monsterDetail':
		case 'monsterCompareLoc': {
			const rowList = list === 'monsters' || list === 'monsterCompare';
			const lair = c.lairStats ? "LAIR Stats" : 'Stats';
			const out: MenuItem[] = [
				{ id: 'copyDetail', label: detail },
				{ id: 'copyName', label: 'Copy Name to Clipboard' },
				{ id: 'lookUpMonster', label: 'Where/How is this Monster Summoned?' },
				{
					id: 'monsterList',
					label:
						list === 'monsters'
							? 'Add/Remove on Save List'
							: list === 'monsterCompare'
								? 'Remove from Save List'
								: 'Add to Monster List'
				}
			];
			if (!rowList) {
				out.push({ id: 'findCounters', label: 'Find counters (negates/dispells)' });
			} else if (!c.autoCalcDamage) {
				out.push({ id: 'recalcDamage', label: 'Re-calculate damage based on current character stats' });
			}
			out.push(
				{ id: 'hitCalc', label: 'Hit Calc vs Mob' },
				{ id: 'setWeaponCalc', label: `Calc Weapons vs Mob's ${lair}` },
				{ id: 'setSpellCalc', label: `Calc Spells vs Mob's ${lair}` }
			);
			return out;
		}
		case 'shopDetail':
			return [
				{ id: 'copyDetail', label: detail },
				{ id: 'copyName', label: 'Copy Name to Clipboard' },
				{ id: 'addToItemManager', label: 'Add to Item Manager' }
			];
		case 'otherItemLoc':
			return [
				{ id: 'copyName', label: 'Copy Name to Clipboard' },
				...(c.chestsEnabled ? [{ id: 'copyChest' as const, label: 'Copy Chest to Clipboard', disabled: NO_CHEST_HERE }] : [])
			];
		case 'itemManagerLoc':
			return [
				{ id: 'copyName', label: 'Copy Text to Clipboard' },
				{ id: 'setShop', label: 'Set Row(s) to This Shop', disabled: NO_SHOP_ASSIGN }
			];
		case 'otherItems':
			return [
				{ id: 'copyDetail', label: detail },
				{ id: 'copyName', label: 'Copy Name to Clipboard' },
				{ id: 'copyChest', label: 'Copy Chest to Clipboard' },
				{ id: 'addToItemManager', label: 'Add to Item Manager' }
			];
		default:
			if ((LOC_LISTS as readonly string[]).includes(list)) {
				return [
					{ id: 'copyName', label: 'Copy Text to Clipboard' },
					...(list === 'spellLoc' || list === 'spellCompareLoc'
						? [{ id: 'addToItemManager' as const, label: 'Add to Item Manager' }]
						: [])
				];
			}
			// shops, classes, races, the spell book: the plain pair
			return [
				{ id: 'copyDetail', label: detail },
				{ id: 'copyName', label: 'Copy Name to Clipboard' }
			];
	}
}

/** The items MME's menu shows for a list, in its order. */
export function buildMenu(c: MenuContext): MenuItem[] {
	switch (menuKind(c.list)) {
		case 'items':
			return itemsMenu(c.list);
		case 'spells':
			return spellsMenu(c.list);
		default:
			return auxMenu(c);
	}
}

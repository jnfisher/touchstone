import { describe, expect, it } from 'vitest';
import { buildMenu, menuKind, type MenuList } from './contextmenu';

const labels = (list: MenuList, extra: object = {}) => buildMenu({ list, ...extra }).map((i) => i.label);

describe('items menu (PopUpItemsMenu)', () => {
	it('weapons get all eight items, in MME order', () => {
		// PopUpItemsMenu case lvWeapons; captions from the menu designer
		expect(labels('weapons')).toEqual([
			'Equip / Unequip Item',
			'Add/Remove Save List',
			'Copy Name(s) to Clipboard',
			'Copy Details to Clipboard',
			'Calculate Swings',
			'Calculate Backstab Damage',
			'Set Combat Backstab Weapon',
			'Add to Item Manager'
		]);
	});
	it('armour has no weapon calculators', () => {
		expect(labels('armour')).toEqual([
			'Equip / Unequip Item',
			'Add/Remove Save List',
			'Copy Name(s) to Clipboard',
			'Copy Details to Clipboard',
			'Add to Item Manager'
		]);
	});
	it('compare lists can only remove', () => {
		expect(labels('weaponCompare')[1]).toBe('Remove From Save List');
		expect(labels('armourCompare')[1]).toBe('Remove From Save List');
		expect(labels('weaponCompare')).toContain('Calculate Swings');
		expect(labels('armourCompare')).not.toContain('Calculate Swings');
	});
	it("the Item Manager's menu does not offer Add to Item Manager", () => {
		expect(labels('itemManager')).toEqual([
			'Equip / Unequip Item',
			'Add/Remove Save List',
			'Copy Name(s) to Clipboard',
			'Copy Details to Clipboard'
		]);
	});
});

describe('spells menu (PopUpSpellsMenu)', () => {
	it('shows all eight items; the first depends on the list', () => {
		expect(labels('spells')).toHaveLength(8);
		expect(labels('spells')[0]).toBe('Add/Remove Save List');
		expect(labels('spellBook')[0]).toBe('Add/Remove Save List');
		expect(labels('spellCompare')[0]).toBe('Remove from Save List');
		expect(labels('spells').slice(1)).toEqual([
			'Copy Details to Clipboard',
			'Copy Name(s) to Clipboard',
			'What casts this spell?',
			'Mark/Unmark Spell as Learned',
			'Find counters (negates/dispells)',
			'Set as Combat Attack/Heal Spell',
			'Set as Bless Spell'
		]);
	});
	it('disables what needs the Results window, with a reason', () => {
		const off = buildMenu({ list: 'spells' }).filter((i) => i.disabled);
		expect(off.map((i) => i.id)).toEqual(['whatCasts', 'findCounters']);
		expect(off.every((i) => i.disabled && i.disabled.length > 10)).toBe(true);
	});
});

describe('aux menu (PopUpAuxMenu)', () => {
	it('monster lists', () => {
		expect(labels('monsters', { autoCalcDamage: true })).toEqual([
			'Copy Details to Clipboard',
			'Copy Name to Clipboard',
			'Where/How is this Monster Summoned?',
			'Add/Remove on Save List',
			'Hit Calc vs Mob',
			"Calc Weapons vs Mob's Stats",
			"Calc Spells vs Mob's Stats"
		]);
		expect(labels('monsterCompare', { autoCalcDamage: true })[3]).toBe('Remove from Save List');
	});
	it('offers the re-calculation only when damage is not automatic', () => {
		expect(labels('monsters', { autoCalcDamage: false })).toContain(
			'Re-calculate damage based on current character stats'
		);
		expect(labels('monsters', { autoCalcDamage: true })).not.toContain(
			'Re-calculate damage based on current character stats'
		);
	});
	it('says LAIR Stats for a lair-only group', () => {
		const l = labels('monsters', { lairStats: true, autoCalcDamage: true });
		expect(l).toContain("Calc Weapons vs Mob's LAIR Stats");
		expect(l).toContain("Calc Spells vs Mob's LAIR Stats");
	});
	it('pane lines swap the re-calculation for the counters finder', () => {
		const l = labels('monsterDetail');
		expect(l).toContain('Find counters (negates/dispells)');
		expect(l[3]).toBe('Add to Monster List');
	});
	it('shop stock lines', () => {
		expect(labels('shopDetail')).toEqual([
			'Copy Details to Clipboard',
			'Copy Name to Clipboard',
			'Add to Item Manager'
		]);
	});
	it('reference lists copy text; spell references can add the item', () => {
		expect(labels('weaponLoc')).toEqual(['Copy Text to Clipboard']);
		expect(labels('mapLoc')).toEqual(['Copy Text to Clipboard']);
		expect(labels('spellLoc')).toEqual(['Copy Text to Clipboard', 'Add to Item Manager']);
		expect(labels('spellCompareLoc')).toEqual(['Copy Text to Clipboard', 'Add to Item Manager']);
	});
	it('the Item Manager reference list sets a shop', () => {
		expect(labels('itemManagerLoc')).toEqual(['Copy Text to Clipboard', 'Set Row(s) to This Shop']);
		expect(buildMenu({ list: 'itemManagerLoc' })[1].disabled).toBeTruthy();
	});
	it('sundry items copy the chest and can be added', () => {
		expect(labels('otherItems')).toEqual([
			'Copy Details to Clipboard',
			'Copy Name to Clipboard',
			'Copy Chest to Clipboard',
			'Add to Item Manager'
		]);
		expect(labels('otherItemLoc')).toEqual(['Copy Name to Clipboard']);
		expect(labels('otherItemLoc', { chestsEnabled: true })).toEqual([
			'Copy Name to Clipboard',
			'Copy Chest to Clipboard'
		]);
	});
	it('shops, classes and races get the plain pair', () => {
		for (const l of ['shops', 'classes', 'races'] as const) {
			expect(labels(l)).toEqual(['Copy Details to Clipboard', 'Copy Name to Clipboard']);
		}
	});
});

describe('menuKind', () => {
	it('sends each list to its menu', () => {
		expect(menuKind('weapons')).toBe('items');
		expect(menuKind('itemManager')).toBe('items');
		expect(menuKind('spellCompare')).toBe('spells');
		expect(menuKind('shops')).toBe('aux');
		expect(menuKind('itemManagerLoc')).toBe('aux');
	});
});

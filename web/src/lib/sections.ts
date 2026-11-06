/** The MME tabs, in order; F1..F11 jump to them. */
export const SECTIONS = [
	{ slug: 'weapons', label: 'Weapons' },
	{ slug: 'armour', label: 'Armour' },
	{ slug: 'spells', label: 'Spells' },
	{ slug: 'monsters', label: 'Monsters' },
	{ slug: 'shops', label: 'Shops' },
	{ slug: 'items', label: 'Items' },
	{ slug: 'races', label: 'Races' },
	{ slug: 'classes', label: 'Classes' },
	{ slug: 'rooms', label: 'Rooms' },
	{ slug: 'character', label: 'Character' },
	{ slug: 'compare', label: 'Compare' }
] as const;

export type SectionSlug = (typeof SECTIONS)[number]['slug'];

export function isSection(s: string): s is SectionSlug {
	return SECTIONS.some((x) => x.slug === s);
}

/** "F1".."F11" -> section slug, otherwise undefined. */
export function sectionForKey(key: string): SectionSlug | undefined {
	const m = /^F(\d{1,2})$/.exec(key);
	return m ? SECTIONS[Number(m[1]) - 1]?.slug : undefined;
}

/**
 * MME's F-key for the tab that corresponds to each of ours (docs/inventory.md section 2: F1 Char, F2 the EQ + Char
 * view, F3 EQ, F4 Lists, F5 Weapons, F6 Armour, F7 Spells, F8 Class/Race, F9 Sundry, F10 Monsters, F11 Shops, F12 Rooms).
 */
const MME_KEY: Record<SectionSlug, string> = {
	weapons: 'F5',
	armour: 'F6',
	spells: 'F7',
	monsters: 'F10',
	shops: 'F11',
	items: 'F9',
	races: 'F8',
	classes: 'F8',
	rooms: 'F12',
	character: 'F1',
	compare: 'F4'
};

export interface KeyLine {
	/** The key that opens the tab here. */
	key: string;
	label: string;
	/** MME's key for the same tab. */
	mme: string;
}

/** The keys as bound (`sectionForKey`), for the help popover: ours, and MME's where they differ in order. */
export function keyLines(): KeyLine[] {
	return SECTIONS.map((s, i) => ({ key: `F${i + 1}`, label: s.label, mme: MME_KEY[s.slug] }));
}

/** The MME tabs, in order, with Chests (touchstone's own) beside Items; F1..F11 jump to the others. */
export const SECTIONS = [
	{ slug: 'weapons', label: 'Weapons' },
	{ slug: 'armour', label: 'Armour' },
	{ slug: 'spells', label: 'Spells' },
	{ slug: 'monsters', label: 'Monsters' },
	{ slug: 'shops', label: 'Shops' },
	{ slug: 'items', label: 'Items' },
	{ slug: 'chests', label: 'Chests' },
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

/** Tabs with no F-key: Chests is new here, and the keys of the MME tabs keep their meaning. */
const NO_KEY: readonly SectionSlug[] = ['chests'];
const KEYED = SECTIONS.filter((s) => !NO_KEY.includes(s.slug));

/** "F1".."F11" -> section slug, otherwise undefined. */
export function sectionForKey(key: string): SectionSlug | undefined {
	const m = /^F(\d{1,2})$/.exec(key);
	return m ? KEYED[Number(m[1]) - 1]?.slug : undefined;
}

/** The key that opens a tab here, undefined for a tab with none. */
export function keyForSection(slug: SectionSlug): string | undefined {
	const i = KEYED.findIndex((s) => s.slug === slug);
	return i < 0 ? undefined : `F${i + 1}`;
}

/**
 * MME's F-key for the tab that corresponds to each of ours (docs/inventory.md section 2: F1 Char, F2 the EQ + Char
 * view, F3 EQ, F4 Lists, F5 Weapons, F6 Armour, F7 Spells, F8 Class/Race, F9 Sundry, F10 Monsters, F11 Shops, F12 Rooms).
 */
const MME_KEY: Record<Exclude<SectionSlug, 'chests'>, string> = {
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
	return KEYED.map((s, i) => ({ key: `F${i + 1}`, label: s.label, mme: MME_KEY[s.slug as keyof typeof MME_KEY] }));
}

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

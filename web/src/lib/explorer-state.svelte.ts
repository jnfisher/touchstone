import { DEFAULT_SPELL_FILTERS, type SpellFilterBoxes } from './spelltools';
// Selection per tab, kept for the life of the page so switching tabs and back returns to the same
// record. Plain in-memory state: a reload starts fresh.
const selected = $state<Record<string, string | number | undefined>>({});

export const selection = {
	get: (slug: string) => selected[slug],
	set: (slug: string, id: string | number) => {
		selected[slug] = id;
	}
};

/** The Shops tab's charm box and buy/sell checks. One value for every shop, as in MME (its charm
 * box mirrors the character's Charm); it starts at 0 and with "Show Buying" checked. */
export const shopView = $state({ charm: 0, buying: true, selling: false });

/** The Spells tab's option boxes (`chkSpellOptions`, `txtSpellOptions(0)`): Calc. Combat, the "vs MR" box and "vs Anti-Magic". */
export const spellBar = $state({ calcCombat: false, vsMr: '0', antiMagic: false });
/** The Spells tab's filter boxes (`cmbSpellMagery`, `cmbSpellMageryLevel`, `chkSpellOptions(1)`, `cmbSpellAttackType`, `cmbSpellTarget`, `cmbSpellContainsAbil`). */
export const spellFilters = $state<SpellFilterBoxes>({ ...DEFAULT_SPELL_FILTERS });
/** A room-command row followed from another tab (`ShowRoomCmdRoomList`): the Rooms tab lists its rooms. */
export const roomsView = $state<{ list?: { key: string; rowText: string; percent: string } }>({});

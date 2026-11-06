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

/** A room-command row followed from another tab (`ShowRoomCmdRoomList`): the Rooms tab lists its rooms. */
export const roomsView = $state<{ list?: { key: string; rowText: string; percent: string } }>({});

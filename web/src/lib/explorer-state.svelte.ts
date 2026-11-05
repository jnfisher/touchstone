// Selection per tab, kept for the life of the page so switching tabs and back returns to the same
// record. Plain in-memory state: a reload starts fresh.
const selected = $state<Record<string, string | number | undefined>>({});

export const selection = {
	get: (slug: string) => selected[slug],
	set: (slug: string, id: string | number) => {
		selected[slug] = id;
	}
};

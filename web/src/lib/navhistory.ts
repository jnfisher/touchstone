// MME's navigation history (`NavHistoryBack`, `NavHistory_RecordNavigation`, `NavHistory_Push`) for the
// web: the tab you were on, and here also the record that was selected on it. MME keeps a back stack of
// 50 tabs and drops a push equal to the top of the stack; the web adds a forward stack, so history is one
// list with a cursor. Plain logic: the store below feeds it from the router.

/** The most places kept; MME's `NAV_HISTORY_MAX`. */
export const NAV_HISTORY_MAX = 50;

export interface NavEntry {
	/** The page's path, like `/monsters`. */
	path: string;
	/** The record that was selected there when it was left. */
	id?: string | number;
}

export function createNavHistory(max = NAV_HISTORY_MAX) {
	let entries: NavEntry[] = [];
	let at = -1;

	return {
		get entries(): readonly NavEntry[] {
			return entries;
		},
		get index() {
			return at;
		},
		get current(): NavEntry | undefined {
			return entries[at];
		},
		get canBack() {
			return at > 0;
		},
		get canForward() {
			return at >= 0 && at < entries.length - 1;
		},
		/**
		 * The page shows `path` (reached by the user, not by back/forward). Staying on the same path is not a
		 * move (`nNavCurrentIndex = nNewIndex`). `leavingId` is the record selected on the page being left.
		 */
		visit(path: string, leavingId?: string | number) {
			const here = entries[at];
			if (here?.path === path) return;
			if (here) here.id = leavingId;
			// going somewhere new ends the forward part, as a browser does
			entries = entries.slice(0, at + 1);
			entries.push({ path });
			if (entries.length > max) entries = entries.slice(entries.length - max);
			at = entries.length - 1;
		},
		/** Remembers the selected record of the page being shown. */
		select(id: string | number | undefined) {
			const here = entries[at];
			if (here) here.id = id;
		},
		/** One step back; `leavingId` as in `visit`. Undefined when there is nowhere to go. */
		back(leavingId?: string | number): NavEntry | undefined {
			if (!this.canBack) return undefined;
			entries[at].id = leavingId;
			at -= 1;
			return entries[at];
		},
		forward(leavingId?: string | number): NavEntry | undefined {
			if (!this.canForward) return undefined;
			entries[at].id = leavingId;
			at += 1;
			return entries[at];
		}
	};
}

export type NavHistory = ReturnType<typeof createNavHistory>;

/** The section a path belongs to ("/monsters" -> "monsters"), whose selection a history entry carries. */
export function slugOfPath(path: string): string | undefined {
	const m = /^\/([a-z]+)\/?$/.exec(path);
	return m?.[1];
}

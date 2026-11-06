// The history wired to the page: the router tells it where the user went, back/forward move it and
// restore the record that was selected. Moves replace the browser's history entry, so the browser's own
// back button keeps working on its own list and the two do not fight.
import { goto } from '$app/navigation';
import { selection } from './explorer-state.svelte';
import { createNavHistory, slugOfPath, type NavEntry } from './navhistory';

export interface NavDeps {
	go?: (path: string) => unknown;
	selected?: (slug: string) => string | number | undefined;
	select?: (slug: string, id: string | number) => void;
}

export function createNavStore(deps: NavDeps = {}) {
	const go = deps.go ?? ((p: string) => goto(p, { replaceState: true }));
	const selected = deps.selected ?? ((s: string) => selection.get(s));
	const select = deps.select ?? ((s: string, id: string | number) => selection.set(s, id));
	const hist = createNavHistory();
	// bumped on every change so the buttons re-read the (plain) history
	let rev = $state(0);
	/** The path a back/forward move is heading to; the router's report of arriving there is not a new visit. */
	let expect: string | undefined;

	const selectedOn = (path: string | undefined) => {
		const slug = path ? slugOfPath(path) : undefined;
		return slug ? selected(slug) : undefined;
	};

	function arrive(e: NavEntry) {
		const slug = slugOfPath(e.path);
		if (slug && e.id !== undefined) select(slug, e.id);
		expect = e.path;
		void go(e.path);
		rev++;
	}

	return {
		get canBack() {
			void rev;
			return hist.canBack;
		},
		get canForward() {
			void rev;
			return hist.canForward;
		},
		/** The router reports the page now shown. */
		shown(path: string) {
			if (expect === path) {
				expect = undefined;
				return;
			}
			expect = undefined;
			hist.visit(path, selectedOn(hist.current?.path));
			rev++;
		},
		/** `NavHistoryBack`. True when it moved. */
		back(): boolean {
			const e = hist.back(selectedOn(hist.current?.path));
			if (!e) return false;
			arrive(e);
			return true;
		},
		forward(): boolean {
			const e = hist.forward(selectedOn(hist.current?.path));
			if (!e) return false;
			arrive(e);
			return true;
		}
	};
}

export type NavStore = ReturnType<typeof createNavStore>;

let shared: NavStore | undefined;
export function getNavStore(): NavStore {
	return (shared ??= createNavStore());
}

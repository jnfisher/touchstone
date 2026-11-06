// The Spell Book window's open state and the seed it opens with (the builder's Spellbook button
// opens it at level 999 for a class). One window for the whole page.
import type { BookForm } from './spelltools';

let open = $state(false);
let seed = $state.raw<{ id: number; form: Partial<BookForm> } | undefined>();
let counter = 0;

export const spellBook = {
	get open() {
		return open;
	},
	set open(v: boolean) {
		open = v;
	},
	/** The latest seed; `id` changes on every open so the window applies it again. */
	get seed() {
		return seed;
	},
	show(form?: Partial<BookForm>) {
		seed = form ? { id: ++counter, form } : undefined;
		open = true;
	}
};

// The character a calculator reads: the Character tab's store, mapped to the numbers MME's windows
// copy from the main form. "Use current character" is on by default once a character is loaded;
// switching it off leaves the manual boxes as they were.
import { api, type ClassRec } from '$lib/api';
import { calcCharacter } from '$lib/calc';
import { getCharacterStore, openLastCharacter } from '$lib/character.svelte';

export function useCurrentCharacter() {
	const store = getCharacterStore();
	let classes = $state.raw<ClassRec[]>([]);
	// undefined follows "a character is loaded"; a click on the switch makes it explicit.
	let choice = $state<boolean>();

	$effect(() => {
		let alive = true;
		api
			.table('classes')
			.then((c) => alive && (classes = c))
			.catch(() => undefined);
		void openLastCharacter(store);
		return () => (alive = false);
	});

	const character = $derived(calcCharacter(store.doc, store.evaluated, classes));
	const use = $derived(choice ?? character !== undefined);

	return {
		/** The mapped character, or undefined while none is loaded and evaluated. */
		get character() {
			return character;
		},
		/** The switch's state. */
		get use() {
			return use;
		},
		set use(v: boolean) {
			choice = v;
		},
		/** The character is there and the switch is on: the fields show its values, read-only. */
		get active() {
			return use && character !== undefined;
		},
		get name() {
			return store.doc.name;
		},
		get level() {
			return store.doc.level;
		}
	};
}

export type CurrentCharacter = ReturnType<typeof useCurrentCharacter>;

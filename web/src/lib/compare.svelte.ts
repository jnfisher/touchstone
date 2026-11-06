// The Compare tab's lists as the page holds them: read from, and written back into, the open
// character's `extra` (the .mmec [Compare] keys), so a change autosaves with the character and an
// import or export carries it.
import { getCharacterStore, type CharacterStore } from './character.svelte';
import {
	addEquipped,
	compareClient,
	readLists,
	toggled,
	withoutAt,
	writeLists,
	emptyLists,
	type CompareKind,
	type CompareLists
} from './compare';

export function createCompareStore(chars: CharacterStore = getCharacterStore(), client = compareClient) {
	/** The sub-tab the Compare tab shows (MME's cmdCompareNav 0..3). */
	let active = $state<CompareKind>('weapons');

	const lists = () => readLists(chars.doc.extra);
	function put(next: CompareLists) {
		chars.edit((d) => {
			d.extra = writeLists(d.extra, next);
		});
	}

	return {
		get active() {
			return active;
		},
		setActive(k: CompareKind) {
			active = k;
		},
		get lists(): CompareLists {
			return lists();
		},
		count(k: CompareKind) {
			return lists()[k].length;
		},
		has(k: CompareKind, n: number) {
			return lists()[k].includes(n);
		},
		/** CompareAddItem and its siblings: on the list means off it, otherwise on the end. */
		toggle(k: CompareKind, n: number) {
			const l = lists();
			l[k] = toggled(l[k], n);
			put(l);
		},
		/** The selected entry leaves the list. */
		removeAt(k: CompareKind, index: number) {
			const l = lists();
			l[k] = withoutAt(l[k], index);
			put(l);
		},
		/** cmdCompareClear_Click for one list. */
		clear(k: CompareKind) {
			const l = lists();
			l[k] = [];
			put(l);
		},
		/** cmdCompareClear_Click(4): all four lists (the Item Manager is not one of them). */
		clearAll() {
			put(emptyLists());
		},
		/** "Add all to compare" on the EQ tab. */
		async addEquipped() {
			const gear = $state.snapshot(chars.doc.equipped).filter((n) => n > 0);
			const kinds = new Map((await client.classify([...new Set(gear)])).map((c) => [c.number, c.kind]));
			put(addEquipped(lists(), $state.snapshot(chars.doc.equipped), kinds));
		}
	};
}

export type CompareStore = ReturnType<typeof createCompareStore>;

let shared: CompareStore | undefined;

export function getCompareStore(): CompareStore {
	return (shared ??= createCompareStore());
}

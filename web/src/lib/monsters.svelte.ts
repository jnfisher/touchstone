import {
	freshControls,
	readControls,
	writeControls,
	type ExtraFilters,
	type MonsterControls,
	type PartyBoxes
} from './monsters';

export function createMonsterFilterStore(storage: Pick<Storage, 'getItem' | 'setItem'> | undefined) {
	let controls = $state<MonsterControls>(readControls(storage));
	// Bumped when reachability is (re)computed: the list and the pane ask the server again.
	let reachRev = $state(0);
	const keep = () => writeControls(storage, controls);
	return {
		get controls() {
			return controls;
		},
		get reachRev() {
			return reachRev;
		},
		bumpReach() {
			reachRev += 1;
		},
		set(change: Partial<Omit<MonsterControls, 'party' | 'extras'>>) {
			controls = { ...controls, ...change };
			keep();
		},
		setParty(change: Partial<PartyBoxes>) {
			controls = { ...controls, party: { ...controls.party, ...change } };
			keep();
		},
		setExtras(change: Partial<ExtraFilters>) {
			controls = { ...controls, extras: { ...controls.extras, ...change } };
			keep();
		},
		/** `RemoveFilters`: the boxes back to their defaults, the extras off (their values stay). */
		reset() {
			const d = freshControls();
			controls = { ...d, lair: controls.lair, party: controls.party, extras: { ...controls.extras, enabled: false } };
			keep();
		}
	};
}

export type MonsterFilterStore = ReturnType<typeof createMonsterFilterStore>;

let shared: MonsterFilterStore | undefined;

/** One set of boxes for the page: the Monsters tab and its pane read them. */
export function getMonsterFilterStore(): MonsterFilterStore {
	return (shared ??= createMonsterFilterStore(typeof localStorage === 'undefined' ? undefined : localStorage));
}

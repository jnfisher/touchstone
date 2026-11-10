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
	// Who the party boxes were last filled from (realm characters or a paste); the boxes themselves are numbers.
	let partyNames = $state.raw<string[]>([]);
	const keep = () => writeControls(storage, controls);
	return {
		get controls() {
			return controls;
		},
		get reachRev() {
			return reachRev;
		},
		get partyNames() {
			return partyNames;
		},
		setPartyNames(names: string[]) {
			partyNames = names;
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
		/** The party boxes filled from a party (realm characters or a paste): the numbers, the damage-in box when the party set it, and who they are. */
		applyParty(boxes: Partial<PartyBoxes>, damage: string | null | undefined, names: string[]) {
			controls = { ...controls, ...(damage ? { damage } : {}), party: { ...controls.party, ...boxes } };
			partyNames = names;
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

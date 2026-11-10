// The character a calculator reads: the active character (the header's picker), mapped to the numbers MME's
// windows copy from the main form. "Use current character" is on by default once a character is loaded;
// switching it off leaves the manual boxes as they were.
//
// Each calculator can also borrow another realm character for itself ("use another"): a second, read-only
// store holds it and every number comes from there instead. The override belongs to that calculator alone
// (nothing else reads it) and the character is never written to.
import { api, type ClassRec, type Race } from '$lib/api';
import { calcCharacter } from '$lib/calc';
import {
	createCharacterStore,
	getActiveCharacter,
	openLastCharacter,
	type CharacterStore
} from '$lib/character.svelte';
import type { CharacterDoc } from '$lib/character';
import { getRealmStore, type RealmStore } from '$lib/realm.svelte';
import { fullName, realmClient, type RealmClient, type RealmSummary } from '$lib/realm';
import { untrack } from 'svelte';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Where a calculator's character comes from, worded for the line under the switch. */
export interface CharacterSource {
	/** `active`: the header's character; `override`: this calculator's own pick. */
	via: 'active' | 'override';
	/** The realm character's id; undefined for the Sandbox. */
	realmId?: number;
	name: string;
	/** "Human Warrior" (empty while the tables load). */
	kind: string;
	level: number;
}

export interface CurrentDeps {
	/** The active character (the header's pick): what the rest of the app calls "the character". */
	store: CharacterStore;
	/** The realm character the header picked, if one is. */
	activeRealmId: () => number | undefined;
	realm: Pick<RealmStore, 'characters' | 'available' | 'list' | 'select' | 'setView'>;
	client?: Pick<RealmClient, 'input'>;
	classes?: () => ClassRec[];
	races?: () => Race[];
	/** The read-only store for a borrowed character. */
	makeOverride?: () => CharacterStore;
}

export function createCurrentCharacter(deps: CurrentDeps) {
	const { store, realm } = deps;
	const client = deps.client ?? realmClient;
	const classes = () => deps.classes?.() ?? [];
	const races = () => deps.races?.() ?? [];
	let borrowed: CharacterStore | undefined;
	let overrideId = $state<number>();
	let overrideError = $state<string>();
	let loaded: string | undefined;
	let ticket = 0;
	// undefined follows "a character is loaded"; a click on the switch makes it explicit.
	let choice = $state<boolean>();

	const own = () => (borrowed ??= (deps.makeOverride ?? (() => createCharacterStore(undefined, { readOnly: true })))());
	/** The store the numbers come from. */
	const source = (): CharacterStore => (overrideId === undefined ? store : own());

	const character = $derived.by(() => {
		const s = source();
		return calcCharacter(s.doc, s.evaluated, classes());
	});
	const use = $derived(choice ?? character !== undefined);

	function summary(id: number): RealmSummary | undefined {
		return realm.characters.find((c) => c.id === id);
	}

	const describe = $derived.by((): CharacterSource => {
		if (overrideId !== undefined) {
			const c = summary(overrideId);
			const d = own().doc;
			return c
				? { via: 'override', realmId: c.id, name: fullName(c), kind: `${c.race_name} ${c.class_name}`, level: c.level }
				: { via: 'override', realmId: overrideId, name: d.name || 'Loading...', kind: '', level: d.level };
		}
		const id = deps.activeRealmId();
		const d = store.doc;
		if (id !== undefined) {
			const c = summary(id);
			if (c) return { via: 'active', realmId: id, name: fullName(c), kind: `${c.race_name} ${c.class_name}`, level: c.level };
		}
		const cls = classes().find((x) => x.number === d.class)?.name ?? '';
		const race = races().find((x) => x.number === d.race)?.name ?? '';
		return { via: 'active', realmId: id, name: d.name || 'Unnamed', kind: `${race} ${cls}`.trim(), level: d.level };
	});

	/** Reads the borrowed character's input; unchanged input leaves the open copy alone. */
	async function load() {
		const id = overrideId;
		if (id === undefined) return;
		const mine = ++ticket;
		try {
			const input: CharacterDoc = await client.input(id);
			if (mine !== ticket) return;
			const key = JSON.stringify(input);
			if (key !== loaded) {
				loaded = key;
				const s = own();
				s.startNew();
				s.set(input);
			}
			overrideError = undefined;
		} catch (e) {
			if (mine !== ticket) return;
			overrideError = message(e);
		}
	}

	return {
		/** The mapped character, or undefined while none is loaded and evaluated. */
		get character() {
			return character;
		},
		/** The character's document, whichever character is used (never edit it: a realm character is read-only). */
		get doc(): CharacterDoc {
			return source().doc;
		},
		/** The evaluate options of the character in use. */
		get options() {
			return source().options;
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
			return source().doc.name;
		},
		get level() {
			return source().doc.level;
		},
		/** Which character this calculator uses, for its line of text. */
		get source(): CharacterSource {
			return describe;
		},
		/** The realm character picked for this calculator alone, if any. */
		get override() {
			return overrideId;
		},
		get overrideError() {
			return overrideError;
		},
		/** The realm characters a calculator can borrow. */
		get choices(): RealmSummary[] {
			return realm.characters;
		},
		get realmAvailable() {
			return realm.available;
		},
		/** "Use another": this calculator takes that realm character instead of the active one. */
		async useAnother(id: number): Promise<void> {
			overrideId = id;
			loaded = undefined;
			overrideError = undefined;
			own().startNew();
			await load();
		},
		/** Back to the active character. */
		clearOverride() {
			ticket++;
			overrideId = undefined;
			overrideError = undefined;
			loaded = undefined;
		},
		/** The realm reloaded: the borrowed character is read again, and dropped if it is gone. */
		async reload(): Promise<void> {
			if (overrideId === undefined) return;
			if (realm.available && realm.list && !summary(overrideId)) {
				this.clearOverride();
				return;
			}
			await load();
		},
		/** The link to the character's sheet: the Characters tab opens on it. */
		openSheet(go: (path: string) => void) {
			const id = describe.realmId;
			if (id !== undefined) {
				void realm.select(id);
				realm.setView('realm');
			} else {
				realm.setView('sandbox');
			}
			go('/character');
		}
	};
}

export function useCurrentCharacter() {
	const active = getActiveCharacter();
	const store = active.store;
	const realm = getRealmStore();
	let classes = $state.raw<ClassRec[]>([]);
	let races = $state.raw<Race[]>([]);

	$effect(() => {
		let alive = true;
		api
			.table('classes')
			.then((c) => alive && (classes = c))
			.catch(() => undefined);
		api
			.table('races')
			.then((r) => alive && (races = r))
			.catch(() => undefined);
		void openLastCharacter(store);
		return () => (alive = false);
	});

	const cur = createCurrentCharacter({
		store,
		activeRealmId: () => active.realmId,
		realm,
		classes: () => classes,
		races: () => races
	});
	// the realm reloaded (the roster is read again): a borrowed character follows
	$effect(() => {
		void realm.list;
		untrack(() => void cur.reload());
	});
	return cur;
}

export type CurrentCharacter = ReturnType<typeof createCurrentCharacter>;

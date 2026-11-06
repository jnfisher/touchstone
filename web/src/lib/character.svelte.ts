// The Char tab's state: the character being edited, its last evaluation, and the saved list.
// Edits re-evaluate after a short pause and autosave under the character's name after a longer
// one. Timers are plain `setTimeout`s (no effects) so the store works, and tests, outside a
// component.
import {
	characterClient,
	evalRequest,
	fromServer,
	nameProblem,
	newCharacter,
	type CharacterClient,
	type CharacterDoc,
	type CharacterSummary,
	type EvalOptions,
	type EvalState
} from './character';

export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error';

export interface StoreOptions {
	evalDelay?: number;
	saveDelay?: number;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function createCharacterStore(client: CharacterClient = characterClient, opts: StoreOptions = {}) {
	const evalDelay = opts.evalDelay ?? 150;
	const saveDelay = opts.saveDelay ?? 700;

	let doc = $state<CharacterDoc>(newCharacter());
	let options = $state<EvalOptions>({ hide_class_race: false, min_item_level: 0 });
	let evaluated = $state.raw<EvalState>();
	let evalError = $state<string>();
	let saved = $state.raw<CharacterSummary[]>([]);
	let saveStatus = $state<SaveStatus>('idle');
	let saveError = $state<string>();
	/** The name the character is stored under on the server, if it is stored. */
	let storedName: string | undefined;

	let evalTimer: ReturnType<typeof setTimeout> | undefined;
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	let evalTicket = 0;
	let saveChain: Promise<void> = Promise.resolve();

	function scheduleEval() {
		clearTimeout(evalTimer);
		evalTimer = setTimeout(runEval, evalDelay);
	}

	async function runEval() {
		const req = evalRequest($state.snapshot(doc), $state.snapshot(options));
		const mine = ++evalTicket;
		if (!req) {
			evaluated = undefined;
			evalError = undefined;
			return;
		}
		try {
			const out = await client.evaluate(req);
			if (mine !== evalTicket) return;
			evaluated = out;
			evalError = undefined;
		} catch (e) {
			if (mine !== evalTicket) return;
			evalError = message(e);
		}
	}

	function scheduleSave() {
		clearTimeout(saveTimer);
		if (nameProblem(doc.name)) {
			saveStatus = 'idle';
			return;
		}
		saveTimer = setTimeout(() => void flush(), saveDelay);
	}

	/** Saves now. Saves are serialised so an older PUT never lands after a newer one. */
	function flush(): Promise<void> {
		clearTimeout(saveTimer);
		saveChain = saveChain.then(async () => {
			const snapshot = $state.snapshot(doc);
			if (nameProblem(snapshot.name)) return;
			saveStatus = 'saving';
			try {
				await client.put(snapshot.name, snapshot);
				// A rename leaves the old file behind unless it is removed.
				if (storedName && storedName !== snapshot.name) {
					await client.remove(storedName).catch(() => undefined);
				}
				storedName = snapshot.name;
				saveStatus = 'saved';
				saveError = undefined;
				saved = await client.list();
			} catch (e) {
				saveStatus = 'error';
				saveError = message(e);
			}
		});
		return saveChain;
	}

	function replace(next: CharacterDoc, stored: string | undefined) {
		clearTimeout(saveTimer);
		doc = next;
		storedName = stored;
		saveStatus = 'idle';
		saveError = undefined;
		scheduleEval();
	}

	return {
		get doc() {
			return doc;
		},
		get options() {
			return options;
		},
		get evaluated() {
			return evaluated;
		},
		get evalError() {
			return evalError;
		},
		get saved() {
			return saved;
		},
		get saveStatus() {
			return saveStatus;
		},
		get saveError() {
			return saveError;
		},
		get nameProblem() {
			return nameProblem(doc.name);
		},
		get storedName() {
			return storedName;
		},
		/** Apply an edit to the character; re-evaluates and autosaves. */
		edit(change: (d: CharacterDoc) => void) {
			change(doc);
			scheduleEval();
			scheduleSave();
		},
		/** View toggles are not saved with the character. */
		setOptions(change: (o: EvalOptions) => void) {
			change(options);
			scheduleEval();
		},
		async refreshList() {
			try {
				saved = await client.list();
			} catch (e) {
				saveError = message(e);
			}
		},
		startNew() {
			replace(newCharacter(), undefined);
		},
		async load(name: string) {
			await flush();
			try {
				replace(await client.get(name), name);
			} catch (e) {
				saveError = message(e);
			}
		},
		async remove() {
			clearTimeout(saveTimer);
			const name = storedName;
			if (name) {
				try {
					await client.remove(name);
				} catch (e) {
					saveError = message(e);
					return;
				}
			}
			replace(newCharacter(), undefined);
			saved = await client.list().catch(() => saved);
		},
		/** Replace the document wholesale (tests, imports). */
		set(next: Partial<CharacterDoc>) {
			replace(fromServer(next), undefined);
		},
		flush,
		evaluateNow: runEval
	};
}

export type CharacterStore = ReturnType<typeof createCharacterStore>;

// ---------- the shared store ----------
// The Character tab and the calculators look at one character, as MME's windows all read the main
// form. The store is created on first use and lives for the page.

let shared: CharacterStore | undefined;

export function getCharacterStore(): CharacterStore {
	return (shared ??= createCharacterStore());
}

const LAST_KEY = 'touchstone.character.last';

/** Remembers which saved character was open last, for the next visit. */
export function rememberLastCharacter(name: string | undefined) {
	try {
		if (name) localStorage.setItem(LAST_KEY, name);
	} catch {
		// private window or blocked storage: the app still works, it just starts empty
	}
}

const opened = new WeakMap<CharacterStore, Promise<string | undefined>>();

/**
 * Opens the character that was open last (or the first saved one) unless one is already being
 * edited. Runs once per store; resolves to the name opened.
 */
export function openLastCharacter(store: CharacterStore): Promise<string | undefined> {
	let p = opened.get(store);
	if (!p) {
		p = (async () => {
			await store.refreshList();
			let last: string | null = null;
			try {
				last = localStorage.getItem(LAST_KEY);
			} catch {
				// see rememberLastCharacter()
			}
			const names = store.saved.map((s) => s.name);
			const name = last && names.includes(last) ? last : names[0];
			if (name && store.doc.name === '' && !store.storedName) {
				await store.load(name);
				return name;
			}
			return undefined;
		})();
		opened.set(store, p);
	}
	return p;
}

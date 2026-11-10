// The Characters tab's realm state: the roster, the selected character's sheet, and which of the tab's two
// views is open. `refresh()` is called whenever the server reloads its live database (the info load number
// changes), so the list and the open sheet follow it. A request overtaken by a newer one is dropped.
import { realmClient, type RealmClient, type RealmDetail, type RealmList } from './realm';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function createRealmStore(client: RealmClient = realmClient) {
	let list = $state.raw<RealmList>();
	let error = $state<string>();
	let selected = $state<number>();
	let detail = $state.raw<RealmDetail>();
	let detailError = $state<string>();
	let view = $state<'realm' | 'sandbox'>('realm');
	let listTicket = 0;
	let detailTicket = 0;
	let first: Promise<void> | undefined;

	async function loadDetail() {
		const mine = ++detailTicket;
		if (selected === undefined) {
			detail = undefined;
			return;
		}
		try {
			const d = await client.detail(selected);
			if (mine !== detailTicket) return;
			detail = d;
			detailError = undefined;
		} catch (e) {
			if (mine !== detailTicket) return;
			detail = undefined;
			detailError = message(e);
		}
	}

	async function loadList() {
		const mine = ++listTicket;
		try {
			const l = await client.list();
			if (mine !== listTicket) return;
			list = l;
			error = undefined;
		} catch (e) {
			if (mine !== listTicket) return;
			error = message(e);
		}
	}

	return {
		get list() {
			return list;
		},
		/** True once the server says it has a realm. */
		get available() {
			return list?.available === true;
		},
		get characters() {
			return list?.characters ?? [];
		},
		get error() {
			return error;
		},
		get selected() {
			return selected;
		},
		get detail() {
			return detail;
		},
		get detailError() {
			return detailError;
		},
		get view() {
			return view;
		},
		setView(v: 'realm' | 'sandbox') {
			view = v;
		},
		/** Reads the roster once; later calls share that read. */
		load(): Promise<void> {
			return (first ??= loadList());
		},
		/** Reads the roster again and, if a character is open, its sheet; a character gone from the roster closes. */
		async refresh(): Promise<void> {
			await loadList();
			if (selected !== undefined && list && !list.characters.some((c) => c.id === selected)) {
				selected = undefined;
			}
			await loadDetail();
		},
		async select(id: number | undefined) {
			selected = id;
			detail = undefined;
			detailError = undefined;
			await loadDetail();
		}
	};
}

export type RealmStore = ReturnType<typeof createRealmStore>;

let shared: RealmStore | undefined;

export function getRealmStore(): RealmStore {
	return (shared ??= createRealmStore());
}

// The Chests tab's state: the charm, the chest list, the selected chest and its detail. A request that is
// overtaken by a newer one (a second charm, another chest) is dropped when it lands.
import { chestsClient, DEFAULT_CHARM, parseCharm, type ChestDetail, type ChestList, type ChestsClient } from './chests';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function createChestsStore(client: ChestsClient = chestsClient) {
	let charmText = $state(String(DEFAULT_CHARM));
	let charm = $state(DEFAULT_CHARM);
	let list = $state.raw<ChestList>();
	let detail = $state.raw<ChestDetail>();
	let selected = $state<number>();
	let error = $state<string>();
	let detailError = $state<string>();
	let listTicket = 0;
	let detailTicket = 0;

	async function loadDetail() {
		const mine = ++detailTicket;
		if (selected === undefined) {
			detail = undefined;
			return;
		}
		try {
			const d = await client.detail(selected, charm);
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
			const l = await client.list(charm);
			if (mine !== listTicket) return;
			list = l;
			error = undefined;
		} catch (e) {
			if (mine !== listTicket) return;
			error = message(e);
		}
	}

	return {
		get charmText() {
			return charmText;
		},
		get charm() {
			return charm;
		},
		/** True while the box holds something that is not a charm (the old charm is still in use). */
		get charmInvalid() {
			return parseCharm(charmText) === undefined;
		},
		get list() {
			return list;
		},
		get detail() {
			return detail;
		},
		get selected() {
			return selected;
		},
		get error() {
			return error;
		},
		get detailError() {
			return detailError;
		},
		/** The first load, and a reload: the list and the selected chest's detail at the current charm. */
		async load() {
			await Promise.all([loadList(), loadDetail()]);
		},
		async select(item: number | undefined) {
			if (item === selected && detail) return;
			selected = item;
			await loadDetail();
		},
		/** The charm box changed: a valid charm that differs from the current one reprices everything. */
		async setCharmText(text: string) {
			charmText = text;
			const n = parseCharm(text);
			if (n === undefined || n === charm) return;
			charm = n;
			await Promise.all([loadList(), loadDetail()]);
		}
	};
}

export type ChestsStore = ReturnType<typeof createChestsStore>;

let shared: ChestsStore | undefined;
/** One store for the page, so leaving the tab and coming back keeps the charm and the selected chest. */
export function getChestsStore(): ChestsStore {
	return (shared ??= createChestsStore());
}

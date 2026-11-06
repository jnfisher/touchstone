import { DEFAULT_FILTER, readFilter, writeFilter, type GlobalFilter } from './globalfilter';

export function createGlobalFilterStore(storage: Pick<Storage, 'getItem' | 'setItem'> | undefined) {
	let filter = $state<GlobalFilter>(readFilter(storage));
	return {
		get filter() {
			return filter;
		},
		set(change: Partial<GlobalFilter>) {
			filter = { ...filter, ...change };
			writeFilter(storage, filter);
		},
		reset() {
			filter = { ...DEFAULT_FILTER };
			writeFilter(storage, filter);
		}
	};
}

export type GlobalFilterStore = ReturnType<typeof createGlobalFilterStore>;

let shared: GlobalFilterStore | undefined;

/** One bar for the whole page: the shell shows it, every explorer reads it. */
export function getGlobalFilterStore(): GlobalFilterStore {
	return (shared ??= createGlobalFilterStore(typeof localStorage === 'undefined' ? undefined : localStorage));
}

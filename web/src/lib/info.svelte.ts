// The server's `/api/info` as one shared, live value. The badge, the info popover and the Quicksilver notice read it;
// the server reloads a live database in the background (a new load number and created date), so it is read again
// every 30 seconds while the tab is visible, and as soon as the tab becomes visible again.
import { api, type Info } from './api';

export const INFO_POLL_MS = 30_000;

export interface InfoClient {
	info(): Promise<Info>;
}

/** What makes two reads the same database: the load number, the created date and the profile. */
export function infoKey(i: Info): string {
	return JSON.stringify([i.load_count, i.version.date, i.version.time, i.profile ?? null, i.counts]);
}

export interface InfoStoreOptions {
	pollMs?: number;
	/** Whether the page is in view; the poll does nothing while it is not. */
	visible?: () => boolean;
}

export function createInfoStore(client: InfoClient = api, opts: InfoStoreOptions = {}) {
	const pollMs = opts.pollMs ?? INFO_POLL_MS;
	const visible =
		opts.visible ?? (() => typeof document === 'undefined' || document.visibilityState === 'visible');
	let current = $state.raw<Info>();
	let error = $state<string>();
	let users = 0;
	let timer: ReturnType<typeof setInterval> | undefined;
	let onVisible: (() => void) | undefined;
	let inflight: Promise<void> | undefined;

	function refresh(): Promise<void> {
		inflight ??= (async () => {
			try {
				const next = await client.info();
				// a read that finds the same database keeps the object, so nothing downstream redraws
				if (!current || infoKey(current) !== infoKey(next)) current = next;
				error = undefined;
			} catch (e) {
				error = e instanceof Error ? e.message : String(e);
			} finally {
				inflight = undefined;
			}
		})();
		return inflight;
	}

	function begin() {
		void refresh();
		timer = setInterval(() => {
			if (visible()) void refresh();
		}, pollMs);
		if (typeof document !== 'undefined') {
			onVisible = () => {
				if (visible()) void refresh();
			};
			document.addEventListener('visibilitychange', onVisible);
		}
	}

	function end() {
		if (timer) clearInterval(timer);
		timer = undefined;
		if (onVisible && typeof document !== 'undefined') document.removeEventListener('visibilitychange', onVisible);
		onVisible = undefined;
	}

	return {
		get current() {
			return current;
		},
		get error() {
			return error;
		},
		refresh,
		/** A component that shows the info calls this on mount and the returned function on destroy; one poll serves all. */
		use(): () => void {
			if (users++ === 0) begin();
			return () => {
				if (--users === 0) end();
			};
		}
	};
}

export type InfoStore = ReturnType<typeof createInfoStore>;

let shared: InfoStore | undefined;

export function getInfoStore(): InfoStore {
	return (shared ??= createInfoStore());
}

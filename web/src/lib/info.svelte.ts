// The server's `/api/info` as one shared, live value. The badge, the info popover and the Quicksilver notice read it.
// A live database is read again only when someone presses refresh (`POST /api/reload`); another open tab learns of the
// new load number from the poll every 30 seconds while it is visible, and as soon as it becomes visible again.
import { api, type Info, type ReloadResult } from './api';

export const INFO_POLL_MS = 30_000;

export interface InfoClient {
	info(): Promise<Info>;
	reload?(): Promise<ReloadResult>;
}

/** Whether the server reads a live database that the refresh button can read again. */
export function isLive(i: Info | undefined): boolean {
	return i?.source.kind === 'silvermere';
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
	let reloading = $state(false);
	let reloadError = $state<string>();

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

	/** Ask the server to read the live database again, then read the info so the load number moves for everyone here. */
	async function reload(): Promise<ReloadResult | undefined> {
		if (reloading || !client.reload) return undefined;
		reloading = true;
		try {
			const r = await client.reload();
			reloadError = r.error ?? undefined;
			await refresh();
			return r;
		} catch (e) {
			reloadError = e instanceof Error ? e.message : String(e);
			return undefined;
		} finally {
			reloading = false;
		}
	}

	return {
		get current() {
			return current;
		},
		get reloading() {
			return reloading;
		},
		get reloadError() {
			return reloadError;
		},
		reload,
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

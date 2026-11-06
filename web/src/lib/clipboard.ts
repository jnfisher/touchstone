// The clipboard texts MME's menus copy, built by the server (`POST /api/clipboard/*`), and the
// copying itself: `navigator.clipboard` where the browser allows it, otherwise the caller shows
// the text in a selectable dialog (no secure context on a LAN host, or permission refused).

export interface ClipResult {
	/** What the clipboard should hold; null when MME copies nothing. */
	text: string | null;
	/** The VB6 error that stopped the procedure, if one did. */
	error: string | null;
}

export interface ClipRow {
	text: string;
	subs: string[];
	selected?: boolean;
	detail?: string;
	refs?: ClipRow[];
}

export interface ClipColumn {
	text: string;
	width?: number;
}

export interface LinesRequest {
	list: string;
	columns: ClipColumn[];
	rows: ClipRow[];
	detail?: string;
	locations?: ClipRow[];
	exclude_column?: number;
	name_only?: boolean;
}

type FetchFn = typeof fetch;

export function createClipboardClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
	async function post(path: string, body: unknown): Promise<ClipResult> {
		const res = await fetchFn(`/api/clipboard/${path}`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(body)
		});
		if (!res.ok) {
			const reason = (await res.text().catch(() => '')).trim();
			throw new Error(reason || `HTTP ${res.status}`);
		}
		return (await res.json()) as ClipResult;
	}
	return {
		character: (character: object, statsOnly: boolean) => post('character', { character, stats_only: statsOnly }),
		monster: (lines: ClipRow[], nameOnly: boolean) => post('monster', { lines, name_only: nameOnly }),
		shop: (number: number, opts: { charm?: number; showSell?: boolean; nameOnly?: boolean } = {}) =>
			post('shop', { number, charm: opts.charm ?? 0, show_sell: !!opts.showSell, name_only: !!opts.nameOnly }),
		lines: (req: LinesRequest) => post('lines', req)
	};
}

export const clipboardClient = createClipboardClient();

/** Writes text to the clipboard; false when the browser will not (the page then shows the text). */
export async function writeClipboard(
	text: string,
	nav: { clipboard?: { writeText(t: string): Promise<void> } } | undefined = typeof navigator === 'undefined' ? undefined : navigator
): Promise<boolean> {
	if (!nav?.clipboard) return false;
	try {
		await nav.clipboard.writeText(text);
		return true;
	} catch {
		return false;
	}
}

/**
 * A server answer onto the clipboard. Returns what the page should tell the person: nothing to copy,
 * the VB error MME would have shown, or whether the text needs the fallback dialog.
 */
export async function deliver(
	res: ClipResult,
	write: (t: string) => Promise<boolean> = writeClipboard
): Promise<{ copied: boolean; fallback?: string; error?: string; empty?: boolean }> {
	if (res.text === null) return { copied: false, empty: !res.error, error: res.error ?? undefined };
	const ok = await write(res.text);
	return ok
		? { copied: true, error: res.error ?? undefined }
		: { copied: false, fallback: res.text, error: res.error ?? undefined };
}

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Info } from './api';
import { createInfoStore, INFO_POLL_MS } from './info.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const info = (load: number, date = '1/2/2020'): Info => ({
	counts: { items: 3 },
	data_version: 1.11,
	load_count: load,
	loaded_at: '',
	source: { kind: 'mdb', path: 'x.mdb' },
	version: { custom: '', dat_file_version: 'v1.11', date, nmr_version: '1.83', time: '10:00' },
	profile: { name: 'quicksilver', setting: 'auto', content_version: '1.20.0', balance_overrides: [] }
});

describe('the live info store', () => {
	it('reads once on use and picks up a reload on the next poll', async () => {
		const client = { info: vi.fn(async () => info(1)) };
		const store = createInfoStore(client);
		const stop = store.use();
		await vi.advanceTimersByTimeAsync(0);
		expect(store.current?.load_count).toBe(1);
		client.info.mockResolvedValue(info(2, '3/4/2021'));
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS);
		expect(store.current?.load_count).toBe(2);
		expect(store.current?.version.date).toBe('3/4/2021');
		stop();
	});

	it('keeps the same object when nothing changed, and one poll serves every user', async () => {
		const client = { info: vi.fn(async () => info(1)) };
		const store = createInfoStore(client);
		const stopA = store.use();
		const stopB = store.use();
		await vi.advanceTimersByTimeAsync(0);
		const first = store.current;
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS);
		expect(store.current).toBe(first);
		// the mount read plus one poll, not one per user
		expect(client.info).toHaveBeenCalledTimes(2);
		stopA();
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS);
		expect(client.info).toHaveBeenCalledTimes(3);
		stopB();
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS * 3);
		expect(client.info).toHaveBeenCalledTimes(3);
	});

	it('does not poll while the tab is hidden', async () => {
		let shown = true;
		const client = { info: vi.fn(async () => info(1)) };
		const store = createInfoStore(client, { visible: () => shown });
		const stop = store.use();
		await vi.advanceTimersByTimeAsync(0);
		shown = false;
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS * 4);
		expect(client.info).toHaveBeenCalledTimes(1);
		shown = true;
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS);
		expect(client.info).toHaveBeenCalledTimes(2);
		stop();
	});

	it('keeps the last info when a read fails', async () => {
		const client = { info: vi.fn(async () => info(1)) };
		const store = createInfoStore(client);
		const stop = store.use();
		await vi.advanceTimersByTimeAsync(0);
		client.info.mockRejectedValue(new Error('down'));
		await vi.advanceTimersByTimeAsync(INFO_POLL_MS);
		expect(store.current?.load_count).toBe(1);
		expect(store.error).toBe('down');
		stop();
	});
});

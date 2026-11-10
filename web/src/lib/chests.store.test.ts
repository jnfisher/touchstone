import { describe, expect, it, vi } from 'vitest';
import type { ChestDetail, ChestList, ChestsClient } from './chests';
import { createChestsStore } from './chests.svelte';

const list = (charm: number): ChestList => ({
	charm,
	chests: [
		{ item: 906, name: 'wooden box', contents: 61, average_copper: 1, average: '1 Copper', average_short: '1C', dropped_by: 1 },
		{ item: 909, name: 'iron-banded chest', contents: 68, average_copper: 2, average: '2 Copper', average_short: '2C', dropped_by: 4 }
	]
});
const detail = (item: number, charm: number): ChestDetail => ({
	item,
	name: 'x',
	charm,
	contents: [],
	average_copper: 0,
	average: '(no value)',
	average_short: '0',
	droppers: []
});

function setup() {
	const client = {
		list: vi.fn(async (c: number) => list(c)),
		detail: vi.fn(async (i: number, c: number) => detail(i, c))
	};
	return { client, store: createChestsStore(client as unknown as ChestsClient) };
}

describe('chests store', () => {
	it('starts at charm 50 and loads the list', async () => {
		const { store, client } = setup();
		expect(store.charm).toBe(50);
		await store.load();
		expect(client.list).toHaveBeenCalledWith(50);
		expect(store.list?.chests).toHaveLength(2);
		// nothing selected yet, so no detail request
		expect(client.detail).not.toHaveBeenCalled();
	});

	it('loads a chest when it is selected, and not again for the same one', async () => {
		const { store, client } = setup();
		await store.load();
		await store.select(909);
		expect(client.detail).toHaveBeenCalledWith(909, 50);
		expect(store.detail?.item).toBe(909);
		await store.select(909);
		expect(client.detail).toHaveBeenCalledTimes(1);
	});

	it('reprices the list and the chest when the charm changes', async () => {
		const { store, client } = setup();
		await store.load();
		await store.select(909);
		await store.setCharmText('75');
		expect(store.charm).toBe(75);
		expect(client.list).toHaveBeenLastCalledWith(75);
		expect(client.detail).toHaveBeenLastCalledWith(909, 75);
		expect(store.detail?.charm).toBe(75);
	});

	it('ignores a charm that is not a charm, and an unchanged one', async () => {
		const { store, client } = setup();
		await store.load();
		await store.setCharmText('abc');
		expect(store.charmInvalid).toBe(true);
		expect(store.charm).toBe(50);
		await store.setCharmText('50');
		expect(store.charmInvalid).toBe(false);
		expect(client.list).toHaveBeenCalledTimes(1);
	});

	it('drops an answer that a newer request overtook', async () => {
		let release!: (d: ChestDetail) => void;
		const client = {
			list: vi.fn(async (c: number) => list(c)),
			detail: vi.fn((i: number) =>
				i === 906 ? new Promise<ChestDetail>((r) => (release = r)) : Promise.resolve(detail(i, 50))
			)
		};
		const store = createChestsStore(client as unknown as ChestsClient);
		const slow = store.select(906);
		await store.select(909);
		release(detail(906, 50));
		await slow;
		expect(store.detail?.item).toBe(909);
	});

	it('reports a failed list and a failed chest', async () => {
		const client = {
			list: vi.fn(async () => {
				throw new Error('down');
			}),
			detail: vi.fn(async () => {
				throw new Error('not a chest');
			})
		};
		const store = createChestsStore(client as unknown as ChestsClient);
		await store.load();
		expect(store.error).toBe('down');
		await store.select(1);
		expect(store.detailError).toBe('not a chest');
		expect(store.detail).toBeUndefined();
	});
});

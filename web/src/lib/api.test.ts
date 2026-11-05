import { describe, expect, it, vi } from 'vitest';
import { ApiError, createApi, expPath } from './api';

function res(status: number, body: unknown, etag?: string) {
	return new Response(status === 304 ? null : JSON.stringify(body), {
		status,
		headers: etag ? { etag } : {}
	});
}

describe('api client', () => {
	it('fetches a table once and serves the cache afterwards', async () => {
		const f = vi.fn(async (_p: string, _i?: RequestInit) => res(200, [{ number: 1 }], '"a"'));
		const api = createApi(f as unknown as typeof fetch);
		const a = await api.table('races');
		const b = await api.table('races');
		expect(a).toBe(b);
		expect(f).toHaveBeenCalledTimes(1);
		expect(f.mock.calls[0][0]).toBe('/api/races');
		expect(api.has('races')).toBe(true);
	});

	it('shares one request between concurrent callers', async () => {
		const f = vi.fn(async () => res(200, [], '"a"'));
		const api = createApi(f as unknown as typeof fetch);
		await Promise.all([api.table('rooms'), api.table('rooms')]);
		expect(f).toHaveBeenCalledTimes(1);
	});

	it('revalidates with the stored ETag and keeps the data on 304', async () => {
		const f = vi
			.fn()
			.mockResolvedValueOnce(res(200, [{ number: 1 }], '"a"'))
			.mockResolvedValueOnce(res(304, null));
		const api = createApi(f as unknown as typeof fetch);
		const first = await api.table('classes');
		const again = await api.revalidate('classes');
		expect(again).toBe(first);
		expect(f.mock.calls[1][1]).toEqual({ headers: { 'If-None-Match': '"a"' } });
	});

	it('replaces the data when the ETag changed', async () => {
		const f = vi
			.fn()
			.mockResolvedValueOnce(res(200, [1], '"a"'))
			.mockResolvedValueOnce(res(200, [1, 2], '"b"'));
		const api = createApi(f as unknown as typeof fetch);
		await api.table('shops');
		expect(await api.revalidate('shops')).toEqual([1, 2]);
		expect(await api.table('shops')).toEqual([1, 2]);
	});

	it('throws ApiError on failure and does not cache it', async () => {
		const f = vi.fn().mockResolvedValueOnce(res(500, {})).mockResolvedValueOnce(res(200, [], '"a"'));
		const api = createApi(f as unknown as typeof fetch);
		await expect(api.table('spells')).rejects.toBeInstanceOf(ApiError);
		expect(await api.table('spells')).toEqual([]);
	});
});

describe('computed endpoints', () => {
	it('builds the exp query', () => {
		expect(expPath({ race: 2, class: 5 })).toBe('/api/exp?race=2&class=5');
		expect(expPath({ race: 2, class: 5, from: 1, to: 255, markup: 25 })).toBe(
			'/api/exp?race=2&class=5&from=1&to=255&markup=25'
		);
	});

	it('fetches shop prices fresh each time', async () => {
		const f = vi.fn(async (_p: string) => res(200, { shop: 3, items: [] }));
		const api = createApi(f as unknown as typeof fetch);
		await api.shopPrices(3, 50);
		await api.shopPrices(3, 60);
		expect(f.mock.calls.map((c) => c[0])).toEqual([
			'/api/shops/3/prices?charm=50',
			'/api/shops/3/prices?charm=60'
		]);
	});

	it('reports a failed exp request', async () => {
		const f = vi.fn(async () => res(404, {}));
		const api = createApi(f as unknown as typeof fetch);
		await expect(api.expTable({ race: 9, class: 1 })).rejects.toBeInstanceOf(ApiError);
	});
});

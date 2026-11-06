import { describe, expect, it, vi } from 'vitest';
import { newCharacter } from './character';
import { DEFAULT_FILTER } from './globalfilter';
import { createRouteClient, routeLevel, routeRequest, RouteError } from './routedialog';

describe('route dialog requests', () => {
	it('asks for a monster, its alternate, or a room, at the global level', () => {
		expect(routeRequest({ monster: 45 }, 30)).toEqual({ monster: 45, level: 30 });
		expect(routeRequest({ monster: 45, alt: true }, 30)).toEqual({ monster: 45, alt: true, level: 30 });
		expect(routeRequest({ monster: 45, alt: false }, 30)).toEqual({ monster: 45, level: 30 });
		expect(routeRequest({ map: 1, room: 1403 }, 12)).toEqual({ map: 1, room: 1403, level: 12 });
	});

	it('reads txtGlobalLevel: the character while the bar mirrors it, else the bar', () => {
		const ready = { ...newCharacter('x'), class: 1, race: 2, level: 33 };
		const bar = { ...DEFAULT_FILTER, level: 20, use_character: false };
		expect(routeLevel(bar, ready)).toBe(20);
		expect(routeLevel({ ...bar, use_character: true }, ready)).toBe(33);
		// a half-made character cannot be mirrored
		expect(routeLevel({ ...bar, use_character: true }, newCharacter('x'))).toBe(20);
		expect(routeLevel({ ...bar, use_character: true }, undefined)).toBe(20);
	});

	it('posts the request and returns the dialog', async () => {
		const data = { caption: 'Route to Bank', rows: [{ step: 0, text: 'Room: Bank (1/297)', map: 1, room: 297 }], message: null };
		const fetchFn = vi.fn(async () => new Response(JSON.stringify(data)));
		const out = await createRouteClient(fetchFn as unknown as typeof fetch).route({ map: 1, room: 297, level: 5 });
		expect(out).toEqual(data);
		const [url, init] = fetchFn.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/reachability/route');
		expect(init.method).toBe('POST');
		expect(JSON.parse(init.body as string)).toEqual({ map: 1, room: 297, level: 5 });
	});

	it('reports the server text of a refusal', async () => {
		const fetchFn = vi.fn(async () => new Response('give a monster, or a map and a room', { status: 400 }));
		const err = await createRouteClient(fetchFn as unknown as typeof fetch).route({ level: 1 }).catch((e) => e);
		expect(err).toBeInstanceOf(RouteError);
		expect(err.status).toBe(400);
		expect(err.message).toBe('give a monster, or a map and a room');
	});
});

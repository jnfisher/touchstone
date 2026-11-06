import { describe, expect, it } from 'vitest';
import {
	DEFAULT_ROOM_OPTIONS,
	RoomHistory,
	createRoomsClient,
	exitSearchReady,
	isRoomList,
	parseRoomKey,
	roomQuery
} from './rooms';

describe('room keys', () => {
	it('parses map/room and nothing else', () => {
		expect(parseRoomKey('17/278')).toEqual({ map: 17, room: 278 });
		expect(parseRoomKey(' 1 / 2 ')).toEqual({ map: 1, room: 2 });
		expect(parseRoomKey('17')).toBeUndefined();
		expect(parseRoomKey('17/278|17/3')).toBeUndefined();
		expect(parseRoomKey('1230@4')).toBeUndefined();
	});
	it('tells a list of rooms from one room', () => {
		expect(isRoomList('17/278|17/2073-2096')).toBe(true);
		expect(isRoomList('17/2073-2096')).toBe(true);
		expect(isRoomList('17/278')).toBe(false);
	});
});

describe('requests', () => {
	it('sends only the boxes that are set', () => {
		expect(roomQuery(DEFAULT_ROOM_OPTIONS)).toBe('');
		expect(roomQuery({ ...DEFAULT_ROOM_OPTIONS, hideNpc: true, allExits: true, illu: -20.7 })).toBe(
			'?hide_npc=true&all_exits=true&illu=-20'
		);
	});
	it('asks the finders the way the server reads them', async () => {
		const calls: [string, RequestInit | undefined][] = [];
		const client = createRoomsClient(async (url, init) => {
			calls.push([String(url), init]);
			return new Response(JSON.stringify({ rows: [] }), { status: 200 });
		});
		await client.room(1, 2, { ...DEFAULT_ROOM_OPTIONS, hideNpc: true });
		await client.findText('dirt path', 50);
		await client.findExits('Stone Tunnel', true, ['N', 'S']);
		await client.cmdRooms('1/2|1/3', 'Room cmd "x": a', '3%');
		expect(calls[0][0]).toBe('/api/rooms/1/2?hide_npc=true');
		expect(calls[1][0]).toBe('/api/rooms/find-text?q=dirt+path&limit=50');
		expect(JSON.parse(String(calls[2][1]?.body))).toEqual({ name: 'Stone Tunnel', exact: true, exits: ['N', 'S'] });
		expect(JSON.parse(String(calls[3][1]?.body))).toEqual({
			list: '1/2|1/3',
			row_text: 'Room cmd "x": a',
			percent: '3%'
		});
	});
	it('reports the server reason', async () => {
		const client = createRoomsClient(async () => new Response('limit must be between 1 and 1000', { status: 400 }));
		await expect(client.findText('a', 0)).rejects.toMatchObject({ status: 400, message: 'limit must be between 1 and 1000' });
	});
	it('applies the finder window rules', () => {
		expect(exitSearchReady('ab', ['N'])).toBe(false);
		expect(exitSearchReady(' abc ', [])).toBe(false);
		expect(exitSearchReady(' abc ', ['N'])).toBe(true);
	});
});

describe('Go Back history', () => {
	it('walks back through the rooms visited, newest first', () => {
		const h = new RoomHistory();
		h.visit(1, 1);
		h.visit(1, 2);
		h.visit(1, 3);
		expect(h.back()).toEqual({ map: 1, room: 2 });
		expect(h.back()).toEqual({ map: 1, room: 1 });
	});
	it('wraps to the second slot when the list runs out', () => {
		const h = new RoomHistory();
		h.visit(1, 1);
		h.visit(1, 2);
		expect(h.back()).toEqual({ map: 1, room: 1 });
		// slot 2 is empty: back to slot 1
		expect(h.back()).toEqual({ map: 1, room: 1 });
	});
	it('does not record the same room twice in a row, and going back records nothing', () => {
		const h = new RoomHistory();
		h.visit(1, 1);
		h.visit(1, 1);
		h.visit(2, 5);
		expect(h.back()).toEqual({ map: 1, room: 1 });
		h.visit(2, 5); // a new jump starts again from the newest
		expect(h.back()).toEqual({ map: 1, room: 1 });
	});
	it('has nothing to go back to at first', () => {
		expect(new RoomHistory().back()).toBeUndefined();
		const h = new RoomHistory();
		h.visit(3, 4);
		expect(h.back()).toBeUndefined();
	});
	it('keeps 21 rooms', () => {
		const h = new RoomHistory();
		for (let r = 1; r <= 30; r++) h.visit(1, r);
		const seen: number[] = [];
		for (let k = 0; k < 20; k++) seen.push(h.back()!.room);
		expect(seen).toEqual(Array.from({ length: 20 }, (_, k) => 29 - k));
		expect(h.back()).toEqual({ map: 1, room: 29 }); // past the last slot: wraps to the second
	});
});

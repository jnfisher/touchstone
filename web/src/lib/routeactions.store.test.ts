import { afterEach, describe, expect, it, vi } from 'vitest';
import { dialogs } from './dialogs.svelte';
import { openRoute } from './routeactions.svelte';
import type { RouteClient } from './routedialog';

afterEach(() => dialogs.closeRoute());

describe('opening the route dialog', () => {
	it('shows what the server answered and keeps the row action', async () => {
		const data = {
			caption: 'Route to Temple Chapel (1/525): 9 steps from Bank of Godfrey',
			rows: [{ step: 0, text: 'Room: Bank of Godfrey (1/297)', map: 1, room: 297 }],
			message: null
		};
		const route = vi.fn(async () => data);
		const onRoom = vi.fn();
		await openRoute({ map: 1, room: 525 }, onRoom, { route } as unknown as RouteClient);
		expect(route).toHaveBeenCalledTimes(1);
		const body = (route.mock.calls as unknown as [Record<string, number>][])[0][0];
		expect(body).toMatchObject({ map: 1, room: 525 });
		expect(typeof body.level).toBe('number');
		expect(dialogs.route?.data).toEqual(data);
		dialogs.route?.onRoom?.(1, 297);
		expect(onRoom).toHaveBeenCalledWith(1, 297);
	});

	it('puts a failure in the dialog as its message', async () => {
		const route = vi.fn(async () => {
			throw new Error('HTTP 500');
		});
		await openRoute({ monster: 3, alt: true }, undefined, { route } as unknown as RouteClient);
		expect(dialogs.route?.data.rows).toEqual([]);
		expect(dialogs.route?.data.message).toBe('The route could not be found: HTTP 500');
		const body = (route.mock.calls as unknown as [Record<string, unknown>][])[0][0];
		expect(body).toMatchObject({ monster: 3, alt: true });
	});

	it('shows the message box text MME would show when there is no route', async () => {
		const route = vi.fn(async () => ({ caption: '', rows: [], message: 'No route found.' }));
		await openRoute({ monster: 9 }, undefined, { route } as unknown as RouteClient);
		expect(dialogs.route?.data.message).toBe('No route found.');
		dialogs.closeRoute();
		expect(dialogs.route).toBeUndefined();
	});
});

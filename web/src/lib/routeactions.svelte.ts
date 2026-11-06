// Opening the route dialog from a tab: reads the global level the way MME's `txtGlobalLevel(0)` does and shows the
// server's answer in the dialog.
import { getCharacterStore } from './character.svelte';
import { dialogs } from './dialogs.svelte';
import { getGlobalFilterStore } from './globalfilter.svelte';
import { routeClient, routeLevel, routeRequest, type RouteClient, type RouteTarget } from './routedialog';

/**
 * `ShowRoomRoute` / `ShowMonsterRoute`. `onRoom` is what a click on a row does; without it the Rooms tab shows the room.
 * A failed request is shown in the dialog as its message.
 */
export async function openRoute(
	target: RouteTarget,
	onRoom?: (map: number, room: number) => void,
	client: RouteClient = routeClient
): Promise<void> {
	const level = routeLevel(getGlobalFilterStore().filter, getCharacterStore().doc);
	try {
		dialogs.showRoute(await client.route(routeRequest(target, level)), onRoom);
	} catch (e) {
		const message = e instanceof Error ? e.message : String(e);
		dialogs.showRoute({ caption: '', rows: [], message: `The route could not be found: ${message}` }, onRoom);
	}
}

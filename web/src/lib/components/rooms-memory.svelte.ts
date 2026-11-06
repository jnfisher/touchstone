// What the Rooms tab keeps while another tab is shown: the room, the last results and the Go Back list.
import { RoomHistory, type RoomInfo, type RoomOptions, type RoomRefRow } from '$lib/rooms';

export const roomsMemory: {
	mapText: string;
	roomText: string;
	info: RoomInfo | undefined;
	results: { caption: string; rows: RoomRefRow[] } | undefined;
	options: Partial<RoomOptions>;
	history: RoomHistory;
} = {
	mapText: '',
	roomText: '',
	info: undefined,
	results: undefined,
	options: {},
	history: new RoomHistory()
};

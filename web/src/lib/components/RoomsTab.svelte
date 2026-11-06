<script lang="ts">
	// The Rooms tab (F9 here, F12 in MME), text half: a room by map and room number, the room name finder, the
	// rooms-with-these-exits finder, and the room's references, exits and tooltip text.
	import { goto } from '$app/navigation';
	import { onMount, untrack } from 'svelte';
	import type { ColDef } from 'ag-grid-community';
	import { api } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { cardCls, headingCls, inputCls, labelCls } from '$lib/components/calc/styles';
	import { linkFor, type RefTarget } from '$lib/detail';
	import { roomsView, selection } from '$lib/explorer-state.svelte';
	import { getSettingsStore } from '$lib/settings.svelte';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import {
		DEFAULT_ROOM_OPTIONS,
		EXIT_DIRS,
		RoomHistory,
		exitSearchReady,
		isRoomList,
		parseRoomKey,
		roomsClient,
		rowRoom,
		type ExitDir,
		type RoomInfo,
		type RoomRefRow
	} from '$lib/rooms';
	import { roomsMemory } from './rooms-memory.svelte';
	import { openRoute } from '$lib/routeactions.svelte';
	import MapView from './map/MapView.svelte';
	import { popoutUrl } from '$lib/map';

	const settings = getSettingsStore();
	const mem = roomsMemory;
	let mapText = $state(mem.mapText);
	let roomText = $state(mem.roomText);
	let info = $state.raw<RoomInfo | undefined>(mem.info);
	let error = $state<string>();
	let results = $state.raw<{ caption: string; rows: RoomRefRow[] } | undefined>(mem.results);
	let notice = $state<string>();
	let nameQuery = $state('');
	let exitName = $state('');
	let exact = $state(false);
	let dirs = $state<ExitDir[]>([]);
	let options = $state({ ...DEFAULT_ROOM_OPTIONS, ...mem.options });
	let ticket = 0;
	let busy = $state(false);

	const fail = (e: unknown) => (error = e instanceof Error ? e.message : String(e));

	/** `MapStartMapping`: show the room; `record` is false for Go Back. */
	async function open(map: number, room: number, record = true) {
		const mine = ++ticket;
		busy = true;
		try {
			const r = await roomsClient.room(map, room, options);
			if (mine !== ticket) return;
			error = undefined;
			if (!r.found) {
				notice = r.message ?? `Room ${map}/${room} was not found.`;
				return;
			}
			notice = undefined;
			info = r;
			mapText = String(map);
			roomText = String(room);
			if (record) mem.history.visit(map, room);
			selection.set('rooms', `${map}/${room}`);
			remember();
		} catch (e) {
			if (mine === ticket) fail(e);
		} finally {
			if (mine === ticket) busy = false;
		}
	}

	function remember() {
		mem.mapText = mapText;
		mem.roomText = roomText;
		mem.info = info;
		mem.results = results;
		mem.options = { ...options };
	}

	function go() {
		const m = Number(mapText.trim());
		const r = Number(roomText.trim());
		if (!Number.isInteger(m) || !Number.isInteger(r) || m < 1 || r < 1) {
			error = 'Enter a map number and a room number.';
			return;
		}
		void open(m, r);
	}

	/** `cmdRoomsButtons_Click` case 6 (`ShowRoomRoute`): the route to the shown room, listed in a dialog whose rows open their rooms. */
	async function routeToRoom() {
		if (!info) return;
		busy = true;
		try {
			await openRoute({ map: info.map, room: info.room }, (m, r) => void open(m, r));
		} finally {
			busy = false;
		}
	}

	function back() {
		const to = mem.history.back();
		if (to) void open(to.map, to.room, false);
	}

	async function findText() {
		const q = nameQuery;
		if (!q) return;
		busy = true;
		try {
			const r = await roomsClient.findText(q, 100);
			error = undefined;
			notice = r.message ?? undefined;
			if (r.matches.length) {
				const rows = r.matches.map((m) => ({
					text: `Room: ${m.name || '(no name)'} (${m.map}/${m.room})`,
					tag: `${m.map}/${m.room}`,
					target: null
				}));
				results = {
					caption:
						`Rooms with "${q}" in the name` + (r.truncated ? `, first ${r.matches.length}` : `, ${r.matches.length}`) + ':',
					rows
				};
				// Find jumps to the first hit
				void open(r.matches[0].map, r.matches[0].room);
			} else results = undefined;
			remember();
		} catch (e) {
			fail(e);
		} finally {
			busy = false;
		}
	}

	async function findExits() {
		if (!exitSearchReady(exitName, dirs)) {
			notice = 'Enter at least three letters of the room name and pick at least one exit.';
			return;
		}
		busy = true;
		try {
			const r = await roomsClient.findExits(exitName, exact, dirs);
			error = undefined;
			notice = r.messages.length ? r.messages.join(' ') : undefined;
			results = r.rows.length ? { caption: r.caption, rows: r.rows } : undefined;
			remember();
		} catch (e) {
			fail(e);
		} finally {
			busy = false;
		}
	}

	async function showList(list: { key: string; rowText: string; percent: string }) {
		busy = true;
		try {
			const r = await roomsClient.cmdRooms(list.key, list.rowText, list.percent);
			error = undefined;
			notice = undefined;
			results = { caption: r.caption, rows: r.rows };
			remember();
		} catch (e) {
			fail(e);
		} finally {
			busy = false;
		}
	}

	function toggleDir(d: ExitDir) {
		dirs = dirs.includes(d) ? dirs.filter((x) => x !== d) : [...dirs, d];
	}

	// Rows elsewhere that lead here (GotoLocation): a room, or the rooms behind a room command.
	onMount(() => {
		const list = roomsView.list;
		if (list) {
			roomsView.list = undefined;
			void showList(list);
			return;
		}
		const sel = selection.get('rooms');
		const key = typeof sel === 'string' ? parseRoomKey(sel) : undefined;
		if (key && !(info && info.map === key.map && info.room === key.room)) void open(key.map, key.room);
	});

	// Options change what the server writes: ask again for the shown room.
	$effect(() => {
		// the hide_record_numbers setting is read by the server on every request, so a change asks again too
		void [settings.current.hide_record_numbers, options.hideNpc, options.hideLairs, options.hideCmds, options.allExits, options.illu];
		untrack(() => {
			if (info) void open(info.map, info.room, false);
			else remember();
		});
	});

	let items: ReadonlyMap<number, { item_type: number; worn: number }> | undefined;
	async function follow(t: RefTarget | null) {
		if (!t) return;
		if (t.table === 'rooms' && t.key) {
			const k = parseRoomKey(t.key);
			if (k) void open(k.map, k.room);
			return;
		}
		if (t.table === 'items' && !items) items = new Map((await api.table('items')).map((i) => [i.number, i]));
		const to = linkFor(t, items);
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}

	function pickRow(r: RoomRefRow) {
		const k = rowRoom(r);
		if (k) void open(k.map, k.room);
	}

	const resultCols: ColDef<RoomRefRow>[] = [{ colId: 'text', headerName: 'Result', field: 'text', flex: 1 }];
	const sel = $derived(info ? `${info.map}/${info.room}` : undefined);
</script>

<div class="grid h-full min-h-0 gap-3 lg:grid-cols-[minmax(340px,5fr)_7fr]" data-testid="rooms-tab">
	<div class="flex min-h-0 flex-col gap-3 overflow-auto">
		<form
			class={cardCls + ' flex flex-wrap items-end gap-2'}
			onsubmit={(e) => {
				e.preventDefault();
				go();
			}}
		>
			<label class="w-24"><span class={labelCls}>Map</span>
				<input class={inputCls} inputmode="numeric" aria-label="Map number" bind:value={mapText} /></label>
			<label class="w-24"><span class={labelCls}>Room</span>
				<input class={inputCls} inputmode="numeric" aria-label="Room number" bind:value={roomText} /></label>
			<Button type="submit" size="default">Go</Button>
			<Button type="button" variant="outline" onclick={back} data-testid="back">Go back</Button>
			<Button type="button" variant="outline" disabled={!info} onclick={() => void routeToRoom()} data-testid="route-to-room">Route to room</Button>
		</form>

		<form
			class={cardCls + ' flex flex-wrap items-end gap-2'}
			onsubmit={(e) => {
				e.preventDefault();
				void findText();
			}}
		>
			<label class="min-w-40 flex-1"><span class={labelCls}>Find room by name</span>
				<input class={inputCls} aria-label="Room name text" bind:value={nameQuery} /></label>
			<Button type="submit" variant="outline">Find</Button>
		</form>

		<form
			class={cardCls + ' flex flex-col gap-2'}
			onsubmit={(e) => {
				e.preventDefault();
				void findExits();
			}}
		>
			<h3 class={headingCls + ' mb-0'}>Find room with exits</h3>
			<label><span class={labelCls}>Room name</span>
				<input class={inputCls} aria-label="Room name for the exits search" bind:value={exitName} /></label>
			<div class="flex gap-4 text-sm" role="radiogroup" aria-label="Name match">
				<label class="flex items-center gap-1.5"><input type="radio" name="match" checked={!exact} onchange={() => (exact = false)} /> Contains</label>
				<label class="flex items-center gap-1.5"><input type="radio" name="match" checked={exact} onchange={() => (exact = true)} /> Exact</label>
			</div>
			<div class="flex flex-wrap gap-1" role="group" aria-label="Exits">
				{#each EXIT_DIRS as d (d)}
					<button
						type="button"
						aria-pressed={dirs.includes(d)}
						class="h-8 w-10 rounded-md border text-sm font-medium {dirs.includes(d)
							? 'bg-primary text-primary-foreground border-primary'
							: 'bg-background border-input hover:bg-accent'}"
						onclick={() => toggleDir(d)}>{d}</button>
				{/each}
			</div>
			<div><Button type="submit" variant="outline">Search</Button></div>
		</form>

		{#if error}<p class="text-destructive text-sm" role="alert">{error}</p>{/if}
		{#if notice}<p class="text-muted-foreground text-sm" data-testid="notice">{notice}</p>{/if}
		{#if results}
			<div class="flex min-h-48 flex-1 flex-col gap-1" data-testid="results">
				<h3 class="text-sm font-medium">{results.caption}</h3>
				<div class="min-h-0 flex-1">
					<DataGrid
						rows={results.rows}
						columnDefs={resultCols}
						getRowId={(r: RoomRefRow) => r.tag + r.text}
						selectedId={undefined}
						storageKey="rooms-results"
						rowState={(r: RoomRefRow) => (rowRoom(r) ? undefined : 'muted')}
						onSelect={pickRow}
					/>
				</div>
			</div>
		{/if}
	</div>

	<div class="flex min-h-0 flex-col gap-3 overflow-auto">
	<div class="bg-card shrink-0 rounded-md border p-3" data-testid="map-panel">
		<MapView
			variant="tab"
			start={info ? { map: info.map, room: info.room } : undefined}
			onNavigate={(at) => void open(at.map, at.room)}
			onPopout={(size, o) => info && window.open(popoutUrl({ map: info.map, room: info.room }, size, o), 'touchstone-map', 'popup,width=1100,height=950')}
		/>
	</div>
	<div class="bg-card max-h-[65vh] min-h-48 shrink-0 overflow-auto rounded-md border text-[13px] leading-5" data-testid="room">
		{#if info}
			<h3 class="bg-card sticky top-0 z-10 truncate border-b px-3 py-2 text-sm font-semibold" data-testid="room-title">{info.title.trim()}</h3>
			<div class="flex flex-col gap-4 p-3">
				<fieldset class="flex flex-wrap gap-x-4 gap-y-1 text-xs">
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={options.hideNpc} /> Hide NPC</label>
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={options.hideLairs} /> Hide lairs</label>
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={options.hideCmds} /> Hide commands</label>
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={options.allExits} /> All exits in summary</label>
					<label class="flex items-center gap-1.5">Illumination
						<input class="{inputCls} !w-16" type="number" aria-label="Illumination" bind:value={options.illu} /></label>
				</fieldset>

				<section>
					<h3 class={headingCls}>Summary</h3>
					<pre class="font-sans text-[13px] break-words whitespace-pre-wrap" data-testid="tooltip">{info.tooltip ?? ''}</pre>
				</section>

				{#if info.exits.length}
					<section>
						<h3 class={headingCls}>Exits</h3>
						<table class="w-full text-left" data-testid="exits">
							<tbody>
								{#each info.exits as e (e.dir)}
									<tr class="border-b last:border-0">
										<th class="w-10 py-0.5 pr-2 font-medium">{e.dir}</th>
										<td class="py-0.5 pr-2">
											{#if e.kind !== 'action' && e.room > 0}
												<button type="button" class="text-primary hover:underline" onclick={() => void open(e.map, e.room)}>{e.name} ({e.map}/{e.room})</button>
											{:else}{e.text}{/if}
										</td>
										<td class="text-muted-foreground py-0.5">{e.kind !== 'normal' && e.kind !== 'action' ? e.exit_type || e.kind : e.kind === 'action' ? 'action' : ''}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</section>
				{/if}

				<section>
					<h3 class={headingCls}>{info.refs_header || 'References'}</h3>
					{#if info.refs.length}
						<ul data-testid="refs">
							{#each info.refs as r, i (i)}
								<li class="border-b py-0.5 last:border-0">
									{#if r.target || (r.text.startsWith('Teleport') && parseRoomKey(r.tag))}
										<button type="button" class="text-primary text-left hover:underline" onclick={() => follow(r.target ?? { table: 'rooms', number: 0, key: r.tag })}>{r.text}</button>
									{:else}<span>{r.text}</span>{/if}
								</li>
							{/each}
						</ul>
					{:else}
						<p class="text-muted-foreground">Nothing is listed for this room.</p>
					{/if}
				</section>
			</div>
		{:else}
			<div class="text-muted-foreground p-3" data-testid="room-empty">Enter a map and room number, or find a room by its name, to see its details.</div>
		{/if}
	</div>
</div>
</div>

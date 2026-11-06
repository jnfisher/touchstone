<script lang="ts">
	// The external map window (frmMap): `/map?map=&room=&size=&options=` opened by the Rooms tab's External Map button.
	// It carries its own room; walking and clicking redraw it and keep the address in step.
	import { page } from '$app/state';
	import { untrack } from 'svelte';
	import MapView from '$lib/components/map/MapView.svelte';
	import { selection } from '$lib/explorer-state.svelte';
	import { optionsParam, parsePopout, type MapOptionsState, type MapSize, type RoomRef } from '$lib/map';

	const given = untrack(() => parsePopout(page.url.search));
	let start = $state<RoomRef | undefined>(given.at);
	let size: MapSize | undefined;
	let options: MapOptionsState | undefined;

	function sync() {
		if (!start || !size || !options) return;
		const p = new URLSearchParams({ map: String(start.map), room: String(start.room), size });
		const list = optionsParam(options);
		if (list) p.set('options', list);
		try {
			history.replaceState(history.state, '', `?${p}`);
		} catch {
			// the address just stays as opened
		}
	}
</script>

<svelte:head><title>touchstone map</title></svelte:head>

<div class="mx-auto flex max-w-[1100px] flex-col gap-2" data-testid="popout">
	{#if !start}
		<p class="text-muted-foreground text-sm" data-testid="popout-missing">Open this window from the Rooms tab, or add ?map=1&amp;room=224 to the address.</p>
	{/if}
	<MapView
		{start}
		variant="window"
		initial={{ size: given.size, options: given.options }}
		onNavigate={(at, select) => {
			start = at;
			if (select) selection.set('rooms', `${at.map}/${at.room}`);
			sync();
		}}
		onChange={(s, o) => {
			size = s;
			options = o;
			sync();
		}}
	/>
</div>

<script lang="ts">
	// frmMapLegend: the colour key, drawn with the same tokens as the cells. The mouse hints are the window's own text.
	import type { LegendEntry } from '$lib/map';

	let { entries, onclose }: { entries: LegendEntry[]; onclose: () => void } = $props();
	const groups = $derived(
		[
			['Exit lines', 'line'],
			['Room fill', 'cell'],
			['Markers', 'marker']
		].map(([title, g]) => ({ title, rows: entries.filter((e) => e.group === g) }))
	);
	const hints = [
		'Hover mouse over room to see information.',
		'Left click: view the room and redraw the map from it.',
		'Right click: redraw map from selected room (an up or down exit is followed).',
		'Shift+right click: redraw map using the cell as the centre; this works on unused cells too.'
	];
</script>

<div
	class="bg-popover text-popover-foreground w-80 rounded-md border p-3 text-sm shadow-lg"
	role="dialog"
	aria-label="Map Legend"
	data-testid="map-legend"
>
	<div class="mb-2 flex items-center justify-between">
		<h3 class="font-semibold">Map Legend</h3>
		<button type="button" class="text-muted-foreground hover:text-foreground text-xs" onclick={onclose}>Close</button>
	</div>
	{#each groups as g (g.title)}
		<h4 class="text-muted-foreground mt-2 mb-1 text-[11px] font-medium tracking-wide uppercase">{g.title}</h4>
		<ul class="flex flex-col gap-1">
			{#each g.rows as e (e.id)}
				<li class="flex items-center gap-2" data-testid="legend-{e.id}">
					<svg width="22" height="14" viewBox="0 0 22 14" aria-hidden="true">
						{#if e.group === 'line'}
							<line x1="1" y1="7" x2="21" y2="7" stroke="var(--map-line-{e.id.toLowerCase()})" stroke-width="2.5" stroke-linecap="round" />
						{:else if e.group === 'cell'}
							<rect x="4" y="1" width="14" height="12" rx="1.5" fill="var(--map-cell-{e.id.toLowerCase()})" stroke="var(--border)" />
						{:else if e.id === 'Npc'}
							<circle cx="11" cy="7" r="4.5" fill="var(--map-mark-npc)" />
						{:else if e.id === 'Lair'}
							<circle cx="11" cy="7" r="5" fill="none" stroke="var(--map-mark-lair)" stroke-width="2" />
						{:else if e.id === 'Command'}
							<rect x="6" y="2" width="10" height="10" fill="none" stroke="var(--map-mark-command)" stroke-width="2" />
						{:else}
							<rect x="5" y="1.5" width="12" height="11" fill="none" stroke="var(--map-mark-{e.id.toLowerCase()})" stroke-width="2" />
						{/if}
					</svg>
					<span>{e.label}</span>
				</li>
			{/each}
		</ul>
	{/each}
	<ul class="text-muted-foreground mt-3 flex list-disc flex-col gap-0.5 pl-4 text-xs">
		{#each hints as h (h)}<li>{h}</li>{/each}
	</ul>
</div>

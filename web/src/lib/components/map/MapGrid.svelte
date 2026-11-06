<script lang="ts">
	// The map as SVG: a rect per placed cell filled by its up/down colour, a line per exit segment, the markers on
	// top. Every colour is a `--map-*` token (app.css), so dark and light need no code here. Events are delegated to
	// the <svg> so a 50x50 map carries no per-cell handlers.
	import { DIR_STEP, cellNumber, type MapCell, type MapResponse, type MapSize, type Marker } from '$lib/map';

	let {
		grid,
		size,
		showBlocks = false,
		onHover,
		onPick,
		onContext
	}: {
		grid: MapResponse;
		size: MapSize;
		/** `cmdMapShowUnused`: outline every block of the grid, used or not. */
		showBlocks?: boolean;
		onHover?: (cell: MapCell | undefined, at: { x: number; y: number }) => void;
		onPick?: (cell: MapCell) => void;
		/** Right click: the cell under the pointer (undefined on an empty block) and its number. */
		onContext?: (cell: MapCell | undefined, number: number, e: MouseEvent) => void;
	} = $props();

	const P = 10;
	const byCell = $derived(new Map(grid.cells.map((c) => [c.cell, c])));
	const blocks = $derived(
		showBlocks ? Array.from({ length: grid.cols * grid.rows }, (_, i) => ({ x: i % grid.cols, y: Math.floor(i / grid.cols) })) : []
	);
	const lines = $derived(
		grid.segments.flatMap((s) => {
			const c = byCell.get(s.cell);
			if (!c) return [];
			const [dx, dy] = DIR_STEP[s.dir];
			const cx = c.x * P + P / 2;
			const cy = c.y * P + P / 2;
			return [{ x1: cx, y1: cy, x2: cx + (dx * P) / 2, y2: cy + (dy * P) / 2, kind: s.kind.toLowerCase() }];
		})
	);
	const has = (c: MapCell, m: Marker) => c.markers.includes(m);

	function cellOf(e: Event): MapCell | undefined {
		const el = (e.target as Element | null)?.closest?.('[data-cell]');
		const n = el ? Number(el.getAttribute('data-cell')) : NaN;
		return byCell.get(n);
	}

	/** The block under the pointer, whether or not it holds a room (shift+right click re-centres on empty ones). */
	function numberAt(e: MouseEvent): number {
		const svg = e.currentTarget as SVGSVGElement;
		const m = svg.getScreenCTM();
		if (!m) return -1;
		const pt = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
		const x = Math.floor(pt.x / P);
		const y = Math.floor(pt.y / P);
		return x < 0 || y < 0 || x >= grid.cols || y >= grid.rows ? -1 : cellNumber(size, x, y);
	}

	function star(cx: number, cy: number, r: number): string {
		return Array.from({ length: 10 }, (_, i) => {
			const a = (Math.PI / 5) * i - Math.PI / 2;
			const rr = i % 2 === 0 ? r : r / 2.3;
			return `${(cx + rr * Math.cos(a)).toFixed(2)},${(cy + rr * Math.sin(a)).toFixed(2)}`;
		}).join(' ');
	}
</script>

<!-- the keyboard walks from the surrounding map element; pointer handling here is the mouse half of the same controls -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<svg
	viewBox="0 0 {grid.cols * P} {grid.rows * P}"
	class="bg-card block h-auto max-h-full w-full rounded-md border"
	style="aspect-ratio: {grid.cols} / {grid.rows}"
	role="img"
	aria-label={grid.title}
	data-testid="map-svg"
	onpointermove={(e) => onHover?.(cellOf(e), { x: e.clientX, y: e.clientY })}
	onpointerleave={() => onHover?.(undefined, { x: 0, y: 0 })}
	onclick={(e) => {
		const c = cellOf(e);
		if (c) onPick?.(c);
	}}
	oncontextmenu={(e) => {
		e.preventDefault();
		onContext?.(cellOf(e), numberAt(e), e);
	}}
>
	{#each blocks as b (b.y * 1000 + b.x)}
		<rect x={b.x * P + 1} y={b.y * P + 1} width={P - 2} height={P - 2} fill="none" stroke="var(--map-block)" stroke-width="0.4" data-testid="map-block" />
	{/each}
	{#each lines as l, i (i)}
		<line x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} stroke="var(--map-line-{l.kind})" stroke-width="0.8" stroke-linecap="round" data-testid="map-segment" data-kind={l.kind} />
	{/each}
	{#each grid.cells as c (c.cell)}
		{@const x = c.x * P}
		{@const y = c.y * P}
		<g data-cell={c.cell} data-testid="map-cell" data-key={c.key} data-color={c.color.toLowerCase()}>
			<rect x={x + 2} y={y + 2} width="6" height="6" rx="0.6" fill="var(--map-cell-{c.color.toLowerCase()})" />
			{#if has(c, 'Overwritten')}
				<rect x={x + 2.4} y={y + 2.4} width="1.8" height="1.8" fill="var(--map-mark-overwritten)" />
			{/if}
			{#if has(c, 'Command')}
				<rect x={x + 3} y={y + 3} width="4" height="4" fill="none" stroke="var(--map-mark-command)" stroke-width="0.7" />
			{/if}
			{#if has(c, 'Lair')}
				<circle cx={x + 5} cy={y + 5} r="2.2" fill="none" stroke="var(--map-mark-lair)" stroke-width="0.7" />
			{/if}
			{#if has(c, 'Npc')}
				<circle cx={x + 5} cy={y + 5} r="1.3" fill="var(--map-mark-npc)" />
			{/if}
			{#if has(c, 'Marked')}
				<polygon points={star(x + 5, y + 5, 2.4)} fill="var(--map-mark-marked)" stroke="var(--map-line-general)" stroke-width="0.25" />
			{/if}
			{#if has(c, 'Missing')}
				<rect x={x + 1.2} y={y + 1.2} width="7.6" height="7.6" fill="none" stroke="var(--map-mark-missing)" stroke-width="0.7" />
			{/if}
			{#if has(c, 'Start')}
				<rect x={x + 0.7} y={y + 0.7} width="8.6" height="8.6" fill="none" stroke="var(--map-mark-start)" stroke-width="1" data-testid="map-start" />
			{/if}
		</g>
	{/each}
</svg>

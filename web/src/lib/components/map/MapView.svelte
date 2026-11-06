<script lang="ts">
	// MME's graphical map (frmMain's map frame and the external frmMap window) as one component. The host owns the
	// start room; this asks `/api/map` for it, draws it, and reports every move through `onNavigate`.
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { inputCls, labelCls } from '$lib/components/calc/styles';
	import { linkFor } from '$lib/detail';
	import { selection } from '$lib/explorer-state.svelte';
	import {
		ALSO_MARK,
		COMPASS,
		OPTION_BOXES,
		SEGMENT_DIRS,
		SIZES,
		WINDOW_SIZES,
		directionForKey,
		loadPrefs,
		mapClient,
		presetPage,
		savePrefs,
		tooltipRoomOptions,
		withPreset,
		type Direction,
		type LeadRow,
		type LeadsHere,
		type LegendEntry,
		type MapCell,
		type MapClient,
		type MapOptionsState,
		type MapResponse,
		type MapSize,
		type Preset,
		type PresetDraft,
		type RoomRef
	} from '$lib/map';
	import { roomsClient } from '$lib/rooms';
	import { getSettingsStore } from '$lib/settings.svelte';
	import LeadsHereDialog from './LeadsHereDialog.svelte';
	import MapGrid from './MapGrid.svelte';
	import MapLegend from './MapLegend.svelte';
	import PresetEditDialog from './PresetEditDialog.svelte';

	let {
		start,
		variant,
		initial,
		client = mapClient,
		onNavigate,
		onPopout,
		onChange
	}: {
		start: RoomRef | undefined;
		/** `tab` is frmMain's fixed grid, `window` frmMap's. */
		variant: 'tab' | 'window';
		/** Size and options the pop-out was opened with; they win over what the browser remembers. */
		initial?: { size?: MapSize; options?: MapOptionsState };
		client?: MapClient;
		/** A new room to chart; `select` also selects it in the Rooms explorer (a click on a cell). */
		onNavigate: (at: RoomRef, select: boolean) => void;
		onPopout?: (size: MapSize, options: MapOptionsState) => void;
		onChange?: (size: MapSize, options: MapOptionsState) => void;
	} = $props();

	const settings = getSettingsStore();
	const remembered = untrack(() => loadPrefs(variant));
	let size = $state<MapSize>(untrack(() => initial?.size) ?? remembered.size);
	let options = $state<MapOptionsState>({ ...(untrack(() => initial?.options) ?? remembered.options) });
	let grid = $state.raw<MapResponse>();
	let error = $state<string>();
	let notice = $state<string>();
	let busy = $state(false);
	let showBlocks = $state(false);
	let legend = $state.raw<LegendEntry[]>();
	let leads = $state.raw<{ result: LeadsHere | null } | undefined>();
	let presets = $state.raw<Preset[]>([]);
	let page = $state(0);
	let editing = $state(false);
	let editPreset = $state.raw<Preset>();
	let confirmReset = $state(false);
	let centerFor = $state<{ key: string; cell: number }>();
	let upDownAsk = $state<MapCell>();
	let redraw = $state(0);

	const sizes = $derived(variant === 'tab' ? SIZES : WINDOW_SIZES);
	const startKey = $derived(start ? `${start.map}/${start.room}/${size}` : '');
	const center = $derived(centerFor && centerFor.key === startKey ? centerFor.cell : undefined);
	const here = $derived(grid ? grid.start : start);
	const hereName = $derived(grid?.title.replace(/^Rooms -- /, '').replace(/\s*\(\d+\/\d+\)\s*$/, '').trim() ?? '');

	$effect(() => {
		savePrefs(variant, { size, options: { ...options } });
		onChange?.(size, options);
	});

	// Draw: any change of room, size, option or centre asks the server again; a late answer is dropped.
	let ticket = 0;
	$effect(() => {
		const at = start;
		const o = { ...options };
		const sz = size;
		const c = center;
		void redraw;
		if (!at) return;
		const mine = ++ticket;
		const ctl = new AbortController();
		busy = true;
		client
			.map(at, sz, o, c, ctl.signal)
			.then((g) => {
				if (mine !== ticket) return;
				grid = g;
				error = undefined;
			})
			.catch((e: unknown) => {
				if (mine !== ticket || ctl.signal.aborted) return;
				error = e instanceof Error ? e.message : String(e);
			})
			.finally(() => {
				if (mine === ticket) busy = false;
			});
		return () => ctl.abort();
	});

	async function loadPresets() {
		try {
			presets = (await client.presets()).presets;
		} catch {
			presets = [];
		}
	}
	$effect(() => {
		void settings.load();
		// saved edits come back through the settings store: ask the endpoint for the merged list again
		void settings.current.map_presets;
		untrack(() => void loadPresets());
	});

	// ---- walking ----
	async function walk(dir: Direction, from: RoomRef | undefined = here) {
		if (!from) return;
		try {
			const w = await client.go(from, dir, variant === 'window');
			notice = undefined;
			if (w === 'Stay') return;
			if ('Go' in w) onNavigate(w.Go, false);
			else notice = w.Message;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function onKey(e: KeyboardEvent) {
		const t = e.target as HTMLElement | null;
		if (t && ['INPUT', 'SELECT', 'TEXTAREA'].includes(t.tagName)) return;
		const d = directionForKey(e);
		if (!d) return;
		e.preventDefault();
		void walk(d);
	}

	// ---- the grid's mouse ----
	let tip = $state<{ cell: MapCell; x: number; y: number; name?: string; text?: string }>();
	let tipTimer: ReturnType<typeof setTimeout> | undefined;
	const tipCache = new Map<string, { name: string; text: string }>();

	function hover(cell: MapCell | undefined, at: { x: number; y: number }) {
		clearTimeout(tipTimer);
		if (!cell || options.noTips) {
			tip = undefined;
			return;
		}
		if (tip && tip.cell.cell === cell.cell) {
			tip = { ...tip, x: at.x, y: at.y };
			return;
		}
		const o = tooltipRoomOptions(options);
		const ck = `${cell.key}|${o.hideNpc}|${o.hideLairs}|${o.hideCmds}`;
		const cached = tipCache.get(ck);
		tip = { cell, x: at.x, y: at.y, ...cached };
		if (cached) return;
		tipTimer = setTimeout(async () => {
			try {
				const r = await roomsClient.room(cell.map, cell.room, o);
				const v = { name: r.name, text: r.tooltip ?? '' };
				tipCache.set(ck, v);
				if (tip && tip.cell.cell === cell.cell) tip = { ...tip, ...v };
			} catch {
				// the hover just stays with the room number
			}
		}, 120);
	}

	function pick(c: MapCell) {
		tip = undefined;
		onNavigate({ map: c.map, room: c.room }, true);
	}

	/** lblRoomCell_MouseDown, right button: shift re-centres, an up or down cell follows it, a cell with both asks. */
	function context(cell: MapCell | undefined, number: number, e: MouseEvent) {
		upDownAsk = undefined;
		if (e.shiftKey) {
			if (number > 0 && start) centerFor = { key: startKey, cell: number };
			return;
		}
		if (!cell) return;
		if (cell.color === 'Up') void walk('U', cell);
		else if (cell.color === 'Down') void walk('D', cell);
		else if (cell.color === 'UpDown') upDownAsk = cell;
		else onNavigate({ map: cell.map, room: cell.room }, false);
	}

	// ---- leads here, legend, presets ----
	async function showLeads() {
		if (!here) return;
		try {
			leads = { result: await client.leadsHere(here) };
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function openLead(r: LeadRow) {
		leads = undefined;
		if (r.kind === 'Room') {
			const [m, rm] = r.tag.split('/').map(Number);
			if (m > 0 && rm > 0) onNavigate({ map: m, room: rm }, true);
			return;
		}
		const to = linkFor({ table: r.kind === 'Spell' ? 'spells' : 'monsters', number: Number(r.tag) }, undefined);
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}

	async function toggleLegend() {
		if (legend) {
			legend = undefined;
			return;
		}
		try {
			legend = await client.legend();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function savePreset(d: PresetDraft) {
		const p = editPreset;
		editPreset = undefined;
		if (!p) return;
		if (!(await settings.setMapPresets(withPreset(settings.current.map_presets, p.index, d)))) error = settings.error;
	}

	async function resetPresets() {
		confirmReset = false;
		if (!(await settings.setMapPresets([]))) error = settings.error;
	}

	const caption = (b: (typeof OPTION_BOXES)[number]) => (variant === 'tab' ? b.main : b.window);
	const unplaced = $derived(
		grid
			? grid.unplaced.map((u) => ({
					...u,
					text: `${u.map}/${u.room}`,
					how: `${SEGMENT_DIRS[u.dir]} of ${grid!.cells.find((c) => c.cell === u.from_cell)?.key ?? '?'}`,
					why: u.reason === 'OffGrid' ? 'off the grid' : 'already shown'
				}))
			: []
	);
	const btn = 'border-input hover:bg-accent h-8 min-w-8 rounded-md border px-2 text-xs font-medium';
</script>

<!-- the map is a keyboard target: arrows and the number pad walk, as frmMap's Form_KeyPress does -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
	class="flex min-h-0 flex-col gap-2 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
	role="group"
	aria-label="Map"
	tabindex="0"
	onkeydown={onKey}
	data-testid="map-view"
	data-variant={variant}
>
	<div class="flex flex-wrap items-end gap-2">
		<label class="w-52"><span class={labelCls}>Size</span>
			<select class={inputCls} aria-label="Map size" bind:value={size} data-testid="map-size">
				{#each sizes as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
			</select></label>
		<Button type="button" variant="outline" size="sm" disabled={!start} onclick={() => { centerFor = undefined; redraw++; }} data-testid="map-redraw">Redraw</Button>
		<Button type="button" variant="outline" size="sm" aria-pressed={showBlocks} onclick={() => (showBlocks = !showBlocks)} data-testid="map-blocks">{showBlocks ? 'Hide Blocks' : 'Show Blocks'}</Button>
		<Button type="button" variant="outline" size="sm" disabled={!here} onclick={() => void showLeads()} data-testid="map-leads">What Leads Here?</Button>
		<div class="relative">
			<Button type="button" variant="outline" size="sm" aria-expanded={!!legend} onclick={() => void toggleLegend()} data-testid="map-legend-button">View Legend</Button>
			{#if legend}
				<div class="absolute top-9 left-0 z-30"><MapLegend entries={legend} onclose={() => (legend = undefined)} /></div>
			{/if}
		</div>
		{#if onPopout}
			<Button type="button" variant="outline" size="sm" disabled={!here} onclick={() => onPopout(size, options)} data-testid="map-popout">External Map</Button>
		{/if}
		{#if busy}<span class="text-muted-foreground text-xs">Drawing...</span>{/if}
	</div>

	{#if error}<p class="text-destructive text-sm" role="alert" data-testid="map-error">{error}</p>{/if}
	{#if notice}<p class="text-sm" data-testid="map-notice">{notice}</p>{/if}

	<div class="flex min-h-0 flex-wrap gap-3">
		<div class="relative min-h-0 min-w-[280px] flex-1" data-testid="map-grid-box">
			{#if grid}
				<MapGrid {grid} {size} {showBlocks} onHover={hover} onPick={pick} onContext={context} />
				{#if upDownAsk}
					<div class="bg-popover absolute top-2 left-2 z-20 flex items-center gap-1 rounded-md border p-2 text-xs shadow" role="menu" data-testid="map-updown">
						<span>Room {upDownAsk.key}:</span>
						<button type="button" class={btn} role="menuitem" onclick={() => { const c = upDownAsk!; upDownAsk = undefined; void walk('U', c); }}>Up</button>
						<button type="button" class={btn} role="menuitem" onclick={() => { const c = upDownAsk!; upDownAsk = undefined; void walk('D', c); }}>Down</button>
					</div>
				{/if}
			{:else if !start}
				<p class="text-muted-foreground text-sm" data-testid="map-empty">Show a room to draw its map.</p>
			{/if}
		</div>

		<div class="flex w-52 flex-col gap-3 text-xs">
			<div class="grid w-fit grid-cols-3 gap-1" role="group" aria-label="Walk">
				{#each COMPASS as d, i (i)}
					{#if d}
						<button type="button" class={btn} disabled={!here} onclick={() => void walk(d)} data-testid="go-{d}" aria-label="Go {d}">{d}</button>
					{:else}
						<span></span>
					{/if}
				{/each}
				<button type="button" class={btn} disabled={!here} onclick={() => void walk('U')} data-testid="go-U" aria-label="Go U">U</button>
				<span></span>
				<button type="button" class={btn} disabled={!here} onclick={() => void walk('D')} data-testid="go-D" aria-label="Go D">D</button>
			</div>

			<fieldset class="flex flex-col gap-1" data-testid="map-options">
				<legend class={labelCls}>Options</legend>
				{#each OPTION_BOXES as b (b.key)}
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={options[b.key]} /> {caption(b)}</label>
				{/each}
			</fieldset>
			<fieldset class="flex flex-wrap gap-x-3 gap-y-1" role="radiogroup" aria-label="Also mark">
				<legend class={labelCls}>Also Mark:</legend>
				{#each ALSO_MARK as a (a.value)}
					<label class="flex items-center gap-1.5"><input type="radio" name="also-mark-{variant}" value={a.value} bind:group={options.alsoMark} /> {a.label}</label>
				{/each}
			</fieldset>
		</div>
	</div>

	{#if unplaced.length}
		<details class="text-xs" data-testid="map-unplaced">
			<summary class="text-muted-foreground cursor-pointer">Rooms not shown ({unplaced.length})</summary>
			<ul class="mt-1 max-h-32 overflow-auto">
				{#each unplaced as u, i (i)}
					<li>
						<button type="button" class="text-primary hover:underline" onclick={() => onNavigate({ map: u.map, room: u.room }, true)}>{u.text}</button>
						<span class="text-muted-foreground">{u.how}, {u.why}</span>
					</li>
				{/each}
			</ul>
		</details>
	{/if}

	<section class="flex flex-col gap-1" data-testid="map-presets">
		<div class="flex flex-wrap items-center gap-1">
			<span class={labelCls}>Presets</span>
			{#each [0, 1, 2, 3, 4] as pg (pg)}
				<button type="button" class={btn + (pg === page ? ' bg-accent' : '')} aria-pressed={pg === page} onclick={() => (page = pg)} data-testid="preset-page-{pg}">{pg * 10 + 1}-{pg * 10 + 10}</button>
			{/each}
			<button type="button" class={btn + (editing ? ' bg-accent' : '')} aria-pressed={editing} onclick={() => (editing = !editing)} data-testid="preset-edit-toggle">{editing ? 'Done editing' : 'Edit'}</button>
			{#if editing}
				{#if confirmReset}
					<span>Reset the presets to the default set?</span>
					<button type="button" class={btn} onclick={() => void resetPresets()} data-testid="preset-reset-yes">Yes</button>
					<button type="button" class={btn} onclick={() => (confirmReset = false)}>No</button>
				{:else}
					<button type="button" class={btn} onclick={() => (confirmReset = true)} data-testid="preset-reset">Reset</button>
				{/if}
			{/if}
		</div>
		<div class="flex flex-wrap gap-1">
			{#each presetPage(presets, page) as p (p.index)}
				<button
					type="button"
					class={btn + ' max-w-40 truncate' + (editing ? ' border-dashed' : '')}
					title="{p.name} ({p.map}/{p.room})"
					onclick={() => (editing ? (editPreset = p) : onNavigate({ map: p.map, room: p.room }, false))}
					data-testid="preset-{p.index}">{p.name}</button>
			{/each}
		</div>
	</section>

	{#if tip}
		<div
			class="bg-popover text-popover-foreground pointer-events-none fixed z-50 max-w-xs rounded-md border px-2 py-1 text-xs shadow-md"
			style="left: {Math.min(tip.x + 14, (typeof window === 'undefined' ? 9999 : window.innerWidth) - 300)}px; top: {tip.y + 14}px"
			data-testid="map-tip"
		>
			<div class="font-medium">{tip.name ?? ''} ({tip.cell.key})</div>
			{#if tip.text}<pre class="font-sans whitespace-pre-wrap">{tip.text}</pre>{/if}
		</div>
	{/if}
</div>

{#if leads}
	<LeadsHereDialog result={leads.result} onpick={openLead} onclose={() => (leads = undefined)} />
{/if}
{#if editPreset}
	<PresetEditDialog
		preset={editPreset}
		current={here ? { map: here.map, room: here.room, name: hereName } : undefined}
		onsave={(d) => void savePreset(d)}
		oncancel={() => (editPreset = undefined)}
	/>
{/if}

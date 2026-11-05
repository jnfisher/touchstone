<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type TableName, type TableTypes } from '$lib/api';
	import DetailPane from '$lib/components/DetailPane.svelte';
	import ExpTable from '$lib/components/ExpTable.svelte';
	import ShopPrices from '$lib/components/ShopPrices.svelte';
	import SplitPane from '$lib/components/SplitPane.svelte';
	import { selection } from '$lib/explorer-state.svelte';
	import { buildLookups, listRows, parseNmr, type AnyExplorer, type Lookups } from '$lib/explorers';
	import { toColDefs } from '$lib/grid/columns';
	import DataGrid from '$lib/grid/DataGrid.svelte';

	type Row = TableTypes[TableName];
	let { explorer, title }: { explorer: AnyExplorer; title: string } = $props();

	let table = $state.raw<Row[]>();
	let nmr = $state(0);
	let lookups = $state.raw<Lookups>();
	let error = $state<string>();
	let search = $state('');
	let shown = $state(0);

	// Each table is fetched when its tab first opens (rooms alone is several MB) and cached by the
	// API client, so coming back to a tab is instant.
	onMount(async () => {
		try {
			const info = await api.info();
			nmr = parseNmr(info.version.nmr_version);
			table = await api.table(explorer.table);
			const extra = await Promise.all(
				explorer.needs.map(async (n: TableName) => [n, await api.table(n)] as const)
			);
			lookups = buildLookups(Object.fromEntries(extra), nmr);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	});

	const rows = $derived(table ? listRows(explorer, table, { nmr }) : []);
	const columnDefs = $derived(toColDefs(explorer.columns, explorer.defaultSort));
	const slug = $derived(explorer.slug);
	const selectedId = $derived(selection.get(slug));
	const selected = $derived(
		selectedId === undefined ? undefined : rows.find((r) => String(explorer.rowId(r)) === String(selectedId))
	);
	const groups = $derived(selected ? explorer.detail(selected, lookups ?? { nmr }) : []);
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<h2 class="text-base font-semibold">{title}</h2>
		<span class="text-muted-foreground text-sm tabular-nums" data-testid="count">
			{#if table}{shown.toLocaleString('en-US')} of {rows.length.toLocaleString('en-US')}{/if}
		</span>
		<input
			type="search"
			placeholder="Search..."
			aria-label="Search {title}"
			bind:value={search}
			class="bg-background border-input placeholder:text-muted-foreground focus-visible:ring-ring h-8 w-56 rounded-md border px-3 text-sm outline-none focus-visible:ring-2"
		/>
		{#if explorer.pending.length}
			<span class="text-muted-foreground text-xs" data-testid="pending">
				Needs a character, coming later: {explorer.pending.join(', ')}
			</span>
		{/if}
	</div>

	<div class="min-h-0 flex-1">
		{#if error}
			<p class="text-destructive" role="alert">Could not load {title.toLowerCase()}: {error}</p>
		{:else if !table}
			<p class="text-muted-foreground" data-testid="loading">Loading {title.toLowerCase()}...</p>
		{:else}
			<SplitPane storageKey="touchstone.split.detail">
				{#snippet left()}
					<DataGrid
						{rows}
						{columnDefs}
						getRowId={(r) => String(explorer.rowId(r))}
						selectedId={selectedId === undefined ? undefined : String(selectedId)}
						quickFilter={search}
						storageKey={slug}
						onSelect={(r) => selection.set(slug, explorer.rowId(r))}
						onCount={(n) => (shown = n)}
					/>
				{/snippet}
				{#snippet right()}
					{#if selected}
						<DetailPane {groups}>
							{#if slug === 'shops'}
								<ShopPrices shop={Number(explorer.rowId(selected))} />
							{:else if slug === 'races' || slug === 'classes'}
								<ExpTable kind={slug === 'races' ? 'race' : 'class'} number={Number(explorer.rowId(selected))} />
							{/if}
						</DetailPane>
					{:else}
						<div class="text-muted-foreground bg-card h-full rounded-md border p-4 text-sm">
							Select a record to see its details.
						</div>
					{/if}
				{/snippet}
			</SplitPane>
		{/if}
	</div>
</div>

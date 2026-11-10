<script lang="ts">
	// The realm list on the left, the selected character's sheet on the right (the explorers' split layout).
	import { onMount } from 'svelte';
	import { getActiveCharacter, getSandboxStore, copyToSandbox } from '$lib/character.svelte';
	import { getRealmStore } from '$lib/realm.svelte';
	import { realmClient, NO_FILTERS, facets, filterRealm, realmColumns, type RealmFilters, type RealmSummary } from '$lib/realm';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import SplitPane from '../SplitPane.svelte';
	import RealmSheet from './RealmSheet.svelte';

	const realm = getRealmStore();
	const active = getActiveCharacter();
	const sandbox = getSandboxStore();
	const columns = realmColumns();

	let filters = $state<RealmFilters>({ ...NO_FILTERS });
	let gridWidth = $state<number>();
	let collapsed = $state(false);
	let copyError = $state<string>();

	onMount(() => void realm.load());

	const rows = $derived(filterRealm(realm.characters, filters));
	const options = $derived(facets(realm.characters));
	const sel = 'bg-background border-input h-8 rounded-md border px-2 text-sm';

	async function copy(id: number) {
		copyError = undefined;
		try {
			await copyToSandbox(sandbox, await realmClient.input(id));
			realm.setView('sandbox');
		} catch (e) {
			copyError = e instanceof Error ? e.message : String(e);
		}
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-2" data-testid="realm-view">
	{#if realm.error}
		<p class="text-destructive" role="alert">Could not load the realm: {realm.error}</p>
	{:else if !realm.list}
		<p class="text-muted-foreground" data-testid="loading">Loading characters...</p>
	{:else if !realm.available}
		<div class="bg-card rounded-lg border p-4" data-testid="realm-unavailable">
			<p class="text-sm">This data source has no realm, so there are no characters to list.</p>
			<p class="text-muted-foreground mt-1 text-sm">
				Only a live Quicksilver or silvermere database has players. The Sandbox still works on any source.
			</p>
			<button type="button" class="bg-secondary text-secondary-foreground mt-3 h-8 rounded-md px-3 text-sm" onclick={() => realm.setView('sandbox')}>Open the Sandbox</button>
		</div>
	{:else}
		<div class="flex flex-wrap items-center gap-2">
			<input
				type="search"
				placeholder="Search..."
				aria-label="Search characters"
				data-testid="realm-search"
				bind:value={filters.search}
				class="bg-background border-input placeholder:text-muted-foreground focus-visible:ring-ring h-8 w-48 rounded-md border px-3 text-sm outline-none focus-visible:ring-2"
			/>
			<select class={sel} aria-label="Race" data-testid="realm-race" bind:value={filters.race}>
				<option value={null}>Any race</option>
				{#each options.races as r (r.id)}<option value={r.id}>{r.name}</option>{/each}
			</select>
			<select class={sel} aria-label="Class" data-testid="realm-class" bind:value={filters.class}>
				<option value={null}>Any class</option>
				{#each options.classes as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
			</select>
			<label class="text-muted-foreground flex items-center gap-1 text-sm">
				Level
				<input class="{sel} w-16 tabular-nums" inputmode="numeric" placeholder="min" aria-label="Minimum level" data-testid="realm-level-min" bind:value={filters.levelMin} />
				to
				<input class="{sel} w-16 tabular-nums" inputmode="numeric" placeholder="max" aria-label="Maximum level" data-testid="realm-level-max" bind:value={filters.levelMax} />
			</label>
			<button type="button" class="border-input hover:bg-accent h-8 rounded-md border px-3 text-sm" onclick={() => (filters = { ...NO_FILTERS })}>Clear</button>
			<span class="text-muted-foreground text-sm tabular-nums" data-testid="realm-count">{rows.length} of {realm.characters.length}</span>
			<button type="button" class="bg-background border-input hover:bg-accent ml-auto h-8 rounded-md border px-3 text-sm" aria-pressed={!collapsed} onclick={() => (collapsed = !collapsed)}>
				{collapsed ? 'Show sheet' : 'Hide sheet'}
			</button>
		</div>
		{#if copyError}<p class="text-destructive text-sm" role="alert">{copyError}</p>{/if}
		<div class="relative min-h-0 flex-1">
			<SplitPane storageKey="touchstone.split.realm" autoWidth={gridWidth === undefined ? undefined : Math.min(gridWidth, 800)} bind:collapsed>
				{#snippet left()}
					<DataGrid
						{rows}
						columnDefs={columns}
						getRowId={(r: RealmSummary) => String(r.id)}
						selectedId={realm.selected === undefined ? undefined : String(realm.selected)}
						storageKey="realm"
						onSelect={(r: RealmSummary) => void realm.select(r.id)}
						onWidth={(w) => (gridWidth = w)}
					/>
				{/snippet}
				{#snippet right()}
					{#if realm.detailError}
						<p class="text-destructive" role="alert">Could not load the character: {realm.detailError}</p>
					{:else if realm.detail}
						<RealmSheet
							detail={realm.detail}
							isActive={active.realmId === realm.detail.id}
							onActive={() => active.select({ kind: 'realm', id: realm.detail!.id })}
							onCopy={() => void copy(realm.detail!.id)}
						/>
					{:else}
						<p class="text-muted-foreground text-sm" data-testid="pick-hint">Pick a character to see its sheet.</p>
					{/if}
				{/snippet}
			</SplitPane>
		</div>
	{/if}
</div>

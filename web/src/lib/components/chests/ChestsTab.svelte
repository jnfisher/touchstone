<script lang="ts">
	// The Chests tab: the chest list on the left; on the right the selected chest's contents (chance, sell price and
	// expected value at the charm in the box) and who drops it. The sale values are touchstone's own, not MME's
	// (docs/differences.md, "Chests tab").
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import { linkFor, type RefTarget } from '$lib/detail';
	import { dropperColumns, dropperRows, contentColumns, summaryColumns, type ChestContent, type ChestSummary, type DropperRow } from '$lib/chests';
	import { getChestsStore } from '$lib/chests.svelte';
	import { roomsView, selection } from '$lib/explorer-state.svelte';
	import { isRoomList } from '$lib/rooms';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import SplitPane from '../SplitPane.svelte';

	const store = getChestsStore();
	const summaryDefs = summaryColumns();
	const contentDefs = contentColumns();
	const dropperDefs = dropperColumns();

	let search = $state('');
	let shown = $state(0);
	let gridWidth = $state<number>();
	let collapsed = $state(false);

	onMount(() => {
		void store.load().then(() => {
			// a chest picked earlier, else the first in the list
			const want = selection.get('chests');
			const first = store.list?.chests[0]?.item;
			const pick = typeof want === 'number' && store.list?.chests.some((c) => c.item === want) ? want : first;
			if (pick !== undefined) void store.select(pick);
		});
	});

	const droppers = $derived(store.detail ? dropperRows(store.detail.droppers) : []);
	const monsterCount = $derived(droppers.filter((d) => d.kind === 'monster').length);

	let items: ReadonlyMap<number, { item_type: number; worn: number }> | undefined;
	async function follow(t: RefTarget | null) {
		if (!t) return;
		if (t.table === 'rooms' && t.key && isRoomList(t.key)) {
			roomsView.list = { key: t.key, rowText: '', percent: '' };
			selection.set('rooms', t.key);
			void goto('/rooms');
			return;
		}
		if (t.table === 'items' && !items) items = new Map((await api.table('items')).map((i) => [i.number, i]));
		const to = linkFor(t, items);
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-2" data-testid="chests-tab">
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<h2 class="text-base font-semibold" data-testid="title">Chests</h2>
		<span class="text-muted-foreground text-sm tabular-nums" data-testid="count">
			{#if store.list}{shown.toLocaleString('en-US')} of {store.list.chests.length.toLocaleString('en-US')}{/if}
		</span>
		<input
			type="search"
			placeholder="Search..."
			aria-label="Search Chests"
			bind:value={search}
			class="bg-background border-input placeholder:text-muted-foreground focus-visible:ring-ring h-8 w-56 rounded-md border px-3 text-sm outline-none focus-visible:ring-2"
		/>
		<label class="flex items-center gap-1 text-sm" title="Sell prices and the average sale use this charm (50 by default)">
			Charm
			<input
				class="bg-background border-input focus-visible:ring-ring h-8 w-16 rounded-md border px-2 text-sm tabular-nums outline-none focus-visible:ring-2 {store.charmInvalid ? 'border-destructive' : ''}"
				inputmode="numeric"
				aria-label="Charm"
				aria-invalid={store.charmInvalid}
				data-testid="chest-charm"
				value={store.charmText}
				oninput={(e) => void store.setCharmText(e.currentTarget.value)}
			/>
		</label>
		<button
			type="button"
			class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm"
			aria-pressed={!collapsed}
			data-testid="toggle-detail"
			onclick={() => (collapsed = !collapsed)}
		>
			{collapsed ? 'Show details' : 'Hide details'}
		</button>
	</div>

	<div class="relative min-h-0 flex-1">
		{#if store.error}
			<p class="text-destructive" role="alert">Could not load chests: {store.error}</p>
		{:else if !store.list}
			<p class="text-muted-foreground" data-testid="loading">Loading chests...</p>
		{:else}
			<SplitPane storageKey="touchstone.split.chests" autoWidth={gridWidth === undefined ? undefined : Math.min(gridWidth, 750)} bind:collapsed>
				{#snippet left()}
					<DataGrid
						rows={store.list!.chests}
						columnDefs={summaryDefs}
						getRowId={(r: ChestSummary) => String(r.item)}
						selectedId={store.selected === undefined ? undefined : String(store.selected)}
						quickFilter={search}
						storageKey="chests"
						onSelect={(r: ChestSummary) => {
							selection.set('chests', r.item);
							void store.select(r.item);
						}}
						onCount={(n) => (shown = n)}
						onWidth={(w) => (gridWidth = w)}
					/>
				{/snippet}
				{#snippet right()}
					{#if store.detailError}
						<p class="text-destructive" role="alert">Could not load the chest: {store.detailError}</p>
					{:else if store.detail}
						{@const d = store.detail}
						<div class="flex h-full min-h-0 flex-col gap-2" data-testid="chest-detail">
							<div class="bg-card rounded-md border px-3 py-2">
								<div class="text-sm font-semibold" data-testid="chest-name">{d.name} <span class="text-muted-foreground font-normal">({d.item})</span></div>
								<div class="text-muted-foreground text-[13px]" data-testid="chest-average">
									{#if d.contents.length}
										Average sale of one opening at charm {d.charm}:
										<span class="text-foreground" title={d.average}>{d.average_short}</span>
										<span class="tabular-nums">({d.average})</span>
									{:else}
										This chest gives nothing.
									{/if}
								</div>
							</div>
							<section class="flex min-h-0 flex-[3] flex-col gap-1">
								<h3 class="text-muted-foreground text-[11px] font-medium tracking-wide uppercase">Contents ({d.contents.length})</h3>
								<div class="min-h-0 flex-1" data-testid="chest-contents">
									<DataGrid
										rows={d.contents}
										columnDefs={contentDefs}
										getRowId={(r: ChestContent) => String(r.item)}
										storageKey="chest-contents"
										onRowClick={(r: ChestContent) => void follow({ table: 'items', number: r.item })}
									/>
								</div>
							</section>
							<section class="flex min-h-0 flex-[2] flex-col gap-1">
								<h3 class="text-muted-foreground text-[11px] font-medium tracking-wide uppercase">
									Dropped by ({monsterCount} {monsterCount === 1 ? 'monster' : 'monsters'}{droppers.length > monsterCount ? `, ${droppers.length - monsterCount} other` : ''})
								</h3>
								<div class="min-h-0 flex-1" data-testid="chest-droppers">
									{#if droppers.length}
										<DataGrid
											rows={droppers}
											columnDefs={dropperDefs}
											getRowId={(r: DropperRow) => r.id}
											storageKey="chest-droppers"
											onRowClick={(r: DropperRow) => void follow(r.target)}
										/>
									{:else}
										<p class="text-muted-foreground text-[13px]">Nothing in the data says where this chest comes from.</p>
									{/if}
								</div>
							</section>
						</div>
					{:else}
						<div class="text-muted-foreground bg-card h-full rounded-md border p-3 text-[13px]">Select a chest to see what it gives.</div>
					{/if}
				{/snippet}
			</SplitPane>
		{/if}
	</div>
</div>

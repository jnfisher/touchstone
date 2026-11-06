<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount, untrack } from 'svelte';
	import { api, type Item, type Spell, type TableName, type TableTypes } from '$lib/api';
	import DetailPane from '$lib/components/DetailPane.svelte';
	import MonsterFilters from '$lib/components/MonsterFilters.svelte';
	import ExpTable from '$lib/components/ExpTable.svelte';
	import ShopControls from '$lib/components/ShopControls.svelte';
	import SpellEffect from '$lib/components/SpellEffect.svelte';
	import SplitPane from '$lib/components/SplitPane.svelte';
	import { getCharacterStore, openLastCharacter } from '$lib/character.svelte';
	import { linkFor, paneBody, paneClient, paneKind, type Pane, type RefTarget } from '$lib/detail';
	import { evaluateBody, evaluateClient, toColDefs as evalColDefs, type EvalRow, type EvalTable } from '$lib/evaluate';
	import { getGlobalFilterStore } from '$lib/globalfilter.svelte';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { roomsView, selection, shopView } from '$lib/explorer-state.svelte';
	import {
		buildLookups,
		listRows,
		parseNmr,
		type AnyExplorer,
		type Lookups
	} from '$lib/explorers';
	import { isRoomList } from '$lib/rooms';
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
	let gridWidth = $state<number>();
	let detailHidden = $state(false);

	// Each table is fetched when its tab first opens (rooms alone is several MB) and cached by the
	// API client, so coming back to a tab is instant.
	const store = getCharacterStore();
	onMount(async () => {
		void openLastCharacter(store);
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

	const computedTab = $derived(explorer.computed);
	const rows = $derived(table && !computedTab ? listRows(explorer, table, { nmr }) : []);
	const columnDefs = $derived(toColDefs(explorer.columns, explorer.defaultSort));
	const slug = $derived(explorer.slug);
	const selectedId = $derived(selection.get(slug));

	// The server builds these lists (MME's columns and cells for the current character). The
	// character store is shared with the Character tab; edits re-fetch after a short pause.
	const bar = getGlobalFilterStore();
	const monsterBoxes = getMonsterFilterStore();
	let evaluated = $state.raw<{ tab: string; table: EvalTable }>();
	let evalError = $state<string>();
	const payload = $derived(evaluated && evaluated.tab === computedTab ? evaluated.table : undefined);
	const evalRows = $derived(payload?.rows ?? []);
	const evalDefs = $derived(payload ? evalColDefs(payload.columns, explorer.defaultSort) : []);
	$effect(() => {
		const tab = computedTab;
		if (!tab) return;
		// The Monsters list also shows damage by the character's attack choice with the bar off.
		const isMonsters = tab === 'monsters';
		const body = evaluateBody(
			$state.snapshot(store.doc),
			$state.snapshot(bar.filter),
			isMonsters ? { monsters: $state.snapshot(monsterBoxes.controls) } : {},
			isMonsters
		);
		const ctl = new AbortController();
		const timer = setTimeout(
			async () => {
				try {
					const t = await evaluateClient.evaluate(tab, body, ctl.signal);
					evaluated = { tab, table: t };
					evalError = undefined;
				} catch (e) {
					if (!ctl.signal.aborted) evalError = e instanceof Error ? e.message : String(e);
				}
			},
			untrack(() => payload) ? 300 : 0
		);
		return () => {
			clearTimeout(timer);
			ctl.abort();
		};
	});

	const total = $derived(computedTab ? evalRows.length : rows.length);
	const selected = $derived.by(() => {
		if (selectedId === undefined) return undefined;
		if (computedTab) return table?.find((r) => String(explorer.rowId(r)) === String(selectedId));
		return rows.find((r) => String(explorer.rowId(r)) === String(selectedId));
	});
	const groups = $derived(selected ? explorer.detail(selected, lookups ?? { nmr }) : []);

	// The record's pane as MME builds it (PullItemDetail and friends) for the current character and
	// global filter. Shop prices follow the charm box; edits re-fetch after a short pause.
	const kind = $derived(paneKind(slug));
	const isShops = $derived(slug === 'shops');
	let pane = $state.raw<Pane>();
	let paneError = $state<string>();
	let compare = $state(true);
	$effect(() => {
		const k = kind;
		const id = selectedId === undefined ? undefined : Number(selectedId);
		if (!k || id === undefined) {
			pane = undefined;
			return;
		}
		const showSell = isShops && shopView.selling;
		const charm = isShops ? (shopView.buying || shopView.selling ? shopView.charm : 0) : undefined;
		const body = paneBody($state.snapshot(store.doc), $state.snapshot(bar.filter), {
			charm,
			showSell,
			options: k === 'monsters' ? { monsters: $state.snapshot(monsterBoxes.controls) } : undefined,
			keepCharacter: k === 'monsters'
		});
		const ctl = new AbortController();
		const timer = setTimeout(
			async () => {
				try {
					const p = await paneClient.pane(k, id, body, ctl.signal);
					pane = p;
					paneError = undefined;
				} catch (e) {
					if (!ctl.signal.aborted) paneError = e instanceof Error ? e.message : String(e);
				}
			},
			untrack(() => pane) ? 150 : 0
		);
		return () => {
			clearTimeout(timer);
			ctl.abort();
		};
	});
	// A pane for another record would flash while the new one loads.
	const livePane = $derived(pane && pane.kind === kind && pane.number === Number(selectedId) ? pane : undefined);

	let itemIndex = $state.raw<Map<number, Item>>();
	async function follow(r: { target: RefTarget | null; text?: string; label?: string; cells?: { text: string }[] }) {
		if (!r.target) return;
		if (r.target.table === 'rooms' && r.target.key && isRoomList(r.target.key)) {
			// GotoLocation -> ShowRoomCmdRoomList: the row's text (its first sub-item's when it has one) and percent
			const withCells = (r.cells?.length ?? 0) > 0;
			roomsView.list = { key: r.target.key, rowText: withCells ? r.cells![0].text : (r.text ?? ''), percent: withCells ? (r.text ?? '').trim() : '' };
			selection.set('rooms', r.target.key);
			void goto('/rooms');
			return;
		}
		let items = itemIndex;
		if (r.target.table === 'items' && !items) {
			items = itemIndex = new Map((await api.table('items')).map((i) => [i.number, i]));
		}
		const to = linkFor(r.target, items);
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}
	const shopNote = $derived(
		!isShops || !livePane?.charm_label ? '' : livePane.charm_label
	);
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<h2 class="text-base font-semibold" data-testid="title">{payload?.filtered ? `*${title}*` : title}</h2>
		<span class="text-muted-foreground text-sm tabular-nums" data-testid="count">
			{#if table && (!computedTab || payload)}{shown.toLocaleString('en-US')} of {total.toLocaleString('en-US')}{/if}
		</span>
		<input
			type="search"
			placeholder="Search..."
			aria-label="Search {title}"
			bind:value={search}
			class="bg-background border-input placeholder:text-muted-foreground focus-visible:ring-ring h-8 w-56 rounded-md border px-3 text-sm outline-none focus-visible:ring-2"
		/>
		<button
			type="button"
			class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm"
			aria-pressed={!detailHidden}
			data-testid="toggle-detail"
			onclick={() => (detailHidden = !detailHidden)}
		>
			{detailHidden ? 'Show details' : 'Hide details'}
		</button>
		{#if explorer.pending.length}
			<span class="text-muted-foreground text-xs" data-testid="pending">
				Needs a character, coming later: {explorer.pending.join(', ')}
			</span>
		{/if}
	</div>

	{#if slug === 'monsters'}
		<MonsterFilters />
	{/if}

	<div class="min-h-0 flex-1">
		{#if error}
			<p class="text-destructive" role="alert">Could not load {title.toLowerCase()}: {error}</p>
		{:else if evalError && !payload}
			<p class="text-destructive" role="alert">Could not build {title.toLowerCase()}: {evalError}</p>
		{:else if !table || (computedTab && !payload)}
			<p class="text-muted-foreground" data-testid="loading">Loading {title.toLowerCase()}...</p>
		{:else}
			<SplitPane storageKey="touchstone.split.{slug}" autoWidth={gridWidth} bind:collapsed={detailHidden}>
				{#snippet left()}
					{#if computedTab}
						<DataGrid
							rows={evalRows}
							columnDefs={evalDefs}
							getRowId={(r: EvalRow) => String(r.number)}
							selectedId={selectedId === undefined ? undefined : String(selectedId)}
							quickFilter={search}
							storageKey={slug}
							onSelect={(r: EvalRow) => selection.set(slug, r.number)}
							onCount={(n) => (shown = n)}
							onWidth={(w) => (gridWidth = w)}
						/>
					{:else}
					<DataGrid
						{rows}
						{columnDefs}
						getRowId={(r) => String(explorer.rowId(r))}
						selectedId={selectedId === undefined ? undefined : String(selectedId)}
						quickFilter={search}
						storageKey={slug}
						onSelect={(r) => selection.set(slug, explorer.rowId(r))}
						onCount={(n) => (shown = n)}
						onWidth={(w) => (gridWidth = w)}
					/>
					{/if}
				{/snippet}
				{#snippet right()}
					{#if selected}
						<DetailPane
							{groups}
							pane={livePane}
							error={paneError}
							bind:compare
							canCompare={slug === 'weapons' || slug === 'armour'}
							onLink={follow}
						>
							{#snippet before()}
								{#if isShops}<ShopControls note={shopNote} />{/if}
							{/snippet}
							{#if slug === 'spells'}
								{#key selectedId}
									<SpellEffect number={Number(explorer.rowId(selected))} reqLevel={(selected as Spell).req_level} />
								{/key}
							{:else if slug === 'races' || slug === 'classes'}
								<ExpTable kind={slug === 'races' ? 'race' : 'class'} number={Number(explorer.rowId(selected))} />
							{/if}
						</DetailPane>
					{:else}
						<div class="text-muted-foreground bg-card h-full rounded-md border p-3 text-[13px]">
							Select a record to see its details.
						</div>
					{/if}
				{/snippet}
			</SplitPane>
		{/if}
	</div>
</div>

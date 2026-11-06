<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount, untrack } from 'svelte';
	import { api, type Item, type Shop, type Spell, type TableName, type TableTypes } from '$lib/api';
	import ClassRaceBuilder from '$lib/components/ClassRaceBuilder.svelte';
	import DetailPane from '$lib/components/DetailPane.svelte';
	import DamageQuestion from '$lib/components/DamageQuestion.svelte';
	import MonsterStatus from '$lib/components/MonsterStatus.svelte';
	import MonsterFilters from '$lib/components/MonsterFilters.svelte';
	import SpellFilters from '$lib/components/SpellFilters.svelte';
	import ExpTable from '$lib/components/ExpTable.svelte';
	import ShopControls from '$lib/components/ShopControls.svelte';
	import SpellEffect from '$lib/components/SpellEffect.svelte';
	import RowContextMenu from '$lib/components/RowContextMenu.svelte';
	import SplitPane from '$lib/components/SplitPane.svelte';
	import { buildMenu, type ActionId, type MenuList } from '$lib/contextmenu';
	import { vsMob, vsMobOptions } from '$lib/handoff.svelte';
	import { createMenuActions, type MenuTarget } from '$lib/menuactions.svelte';
	import { getCharacterStore, openLastCharacter } from '$lib/character.svelte';
	import { getSettingsStore } from '$lib/settings.svelte';
	import { getCompareStore } from '$lib/compare.svelte';
	import { COMPARE_KINDS, type CompareKind } from '$lib/compare';
	import { linkFor, paneBody, paneClient, paneKind, type Pane, type RefTarget } from '$lib/detail';
	import { evaluateBody, evaluateClient, toColDefs as evalColDefs, type EvalRow, type EvalTable } from '$lib/evaluate';
	import { getGlobalFilterStore } from '$lib/globalfilter.svelte';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { filterReset, roomsView, selection, shopView, spellBar, spellFilters } from '$lib/explorer-state.svelte';
	import { dialogs } from '$lib/dialogs.svelte';
	import { chestRows, spellBarOptions, spellFilterOptions, spellOptionsEnabled } from '$lib/spelltools';
	import { getDamageStore } from '$lib/monsterdamage.svelte';
	import type { DamagePrompt } from '$lib/monsterdamage';
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
	let {
		explorer,
		title,
		list
	}: {
		explorer: AnyExplorer;
		title: string;
		/**
		 * The Compare tab's list: show exactly these records in this order (the explorer's rows for them, no
		 * filter), with Remove and Clear in place of "Add to compare".
		 */
		list?: { numbers: number[]; onRemove: (index: number) => void; onClear: () => void };
	} = $props();

	let table = $state.raw<Row[]>();
	let nmr = $state(0);
	let lookups = $state.raw<Lookups>();
	let error = $state<string>();
	let search = $state('');
	$effect(() => {
		if (filterReset.epoch > 0) search = '';
	});
	let shown = $state(0);
	let gridWidth = $state<number>();
	let detailHidden = $state(false);

	// Each table is fetched when its tab first opens (rooms alone is several MB) and cached by the
	// API client, so coming back to a tab is instant.
	const store = getCharacterStore();
	const settings = getSettingsStore();
	onMount(async () => {
		void settings.load();
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
	const selKey = $derived(list ? `compare:${slug}` : slug);
	const selectedRaw = $derived(selection.get(selKey));
	// A compare list always has a row selected, as MME clicks the first one after a change.
	const selectedId = $derived(
		list && !list.numbers.includes(Number(selectedRaw)) ? list.numbers[0] : selectedRaw
	);
	const compareKind = $derived(
		(COMPARE_KINDS as readonly string[]).includes(slug) ? (slug as CompareKind) : undefined
	);
	const cmp = getCompareStore();

	// The server builds these lists (MME's columns and cells for the current character). The
	// character store is shared with the Character tab; edits re-fetch after a short pause.
	const bar = getGlobalFilterStore();
	const monsterBoxes = getMonsterFilterStore();
	const damage = getDamageStore();
	// MME clears the calculated monster damage when a character is loaded or pasted
	$effect(() => {
		const token = store.replaceEpoch;
		if (slug === 'monsters' && !list) void damage.characterChanged(token);
	});
	// FilterMonsters' "calculate the monster damage first?" question, while the list waits for the answer
	let question = $state<DamagePrompt>();
	async function answer(a: 'yes' | 'no' | 'cancel') {
		const q = question;
		question = undefined;
		if (!q || a === 'cancel') return;
		if (a === 'yes' && (await damage.calculate(store.doc, q.mode, monsterBoxes.controls.party))) return;
		await damage.dismiss(q);
	}
	let evaluated = $state.raw<{ tab: string; table: EvalTable }>();
	let evalError = $state<string>();
	const payload = $derived(evaluated && evaluated.tab === computedTab ? evaluated.table : undefined);
	type KeyedRow = EvalRow & { key?: string };
	// A saved list can hold a record twice; the grid needs a distinct id for each row.
	const evalRows = $derived.by((): KeyedRow[] => {
		const rows = payload?.rows ?? [];
		if (!list) return rows;
		const seen = new Map<number, number>();
		return rows.map((r) => {
			const k = seen.get(r.number) ?? 0;
			seen.set(r.number, k + 1);
			return { ...r, key: k === 0 ? String(r.number) : `${r.number}#${k}` };
		});
	});
	const evalDefs = $derived(payload ? evalColDefs(payload.columns, explorer.defaultSort) : []);
	$effect(() => {
		const tab = computedTab;
		void monsterBoxes.reachRev;
		// record numbers in the cells follow the Settings window
		void settings.current.hide_record_numbers;
		if (!tab) return;
		// The Monsters list also shows damage by the character's attack choice with the bar off.
		const isMonsters = tab === 'monsters';
		// a calculation or clear changes what the list shows
		if (isMonsters) void damage.version;
		const body = evaluateBody(
			$state.snapshot(store.doc),
			$state.snapshot(bar.filter),
			{
				...(isMonsters
					? { monsters: { ...$state.snapshot(monsterBoxes.controls), ...(list ? {} : { ask_damage: true }) } }
					: {}),
				...(list ? { numbers: $state.snapshot(list.numbers) } : {}),
				...(slug === 'spells' && !list ? spellBarOptions($state.snapshot(spellBar)) : {}),
				...(slug === 'spells' && !list ? spellFilterOptions($state.snapshot(spellFilters)) : {}),
				// "Set Weapon/Spell Combat Calc. to Mob" from the monsters' menu
				...vsMobOptions(slug)
			},
			isMonsters
		);
		const ctl = new AbortController();
		const timer = setTimeout(
			async () => {
				try {
					const t = await evaluateClient.evaluate(tab, body, ctl.signal);
					evalError = undefined;
					if (t.prompt) {
						// MME stops here and leaves the list as it was until the question is answered
						question = t.prompt;
						return;
					}
					question = undefined;
					evaluated = { tab, table: t };
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
		void monsterBoxes.reachRev;
		void settings.current.hide_record_numbers;
		const id = selectedId === undefined ? undefined : Number(selectedId);
		if (!k || id === undefined) {
			pane = undefined;
			return;
		}
		if (k === 'monsters') void damage.version;
		const showSell = isShops && shopView.selling;
		const charm = isShops ? (shopView.buying || shopView.selling ? shopView.charm : 0) : undefined;
		const body = paneBody($state.snapshot(store.doc), $state.snapshot(bar.filter), {
			charm,
			showSell,
			shopsFirst: k === 'items' && settings.current.shops_first,
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

	// ---- the right-click menu (PopUp*Menu) ----
	const actions = createMenuActions();
	let ctx = $state.raw<MenuTarget>();
	const LIST_OF: Record<string, [MenuList, MenuList]> = {
		weapons: ['weapons', 'weaponCompare'],
		armour: ['armour', 'armourCompare'],
		spells: ['spells', 'spellCompare'],
		monsters: ['monsters', 'monsterCompare'],
		shops: ['shops', 'shops'],
		items: ['otherItems', 'otherItems'],
		races: ['races', 'races'],
		classes: ['classes', 'classes']
	};
	const LOC_OF: Record<string, [MenuList, MenuList]> = {
		weapons: ['weaponLoc', 'weaponCompareLoc'],
		armour: ['armourLoc', 'armourCompareLoc'],
		spells: ['spellLoc', 'spellCompareLoc'],
		shops: ['shopLoc', 'shopLoc'],
		items: ['otherItemLoc', 'otherItemLoc'],
		monsters: ['monsterDetail', 'monsterCompareLoc']
	};
	const pick = (m: Record<string, [MenuList, MenuList]>) => m[slug]?.[list ? 1 : 0];
	const val = (t: string | undefined) => Number.parseFloat(t ?? '') || 0;

	function rowMenu(row: EvalRow) {
		const l = pick(LIST_OF);
		if (!l) return;
		const cells = row.cells.map((c) => c.text);
		ctx = {
			list: l,
			number: row.number,
			columns: payload?.columns.map((c) => c.header),
			cells,
			shop: isShops ? { charm: shopView.buying || shopView.selling ? shopView.charm : 0, showSell: shopView.selling } : undefined,
			lairStats: slug === 'monsters' && nmr >= 1.83 && monsterBoxes.controls.lair && val(cells[2]) === 0 && val(cells[12]) > 0
		};
	}

	function paneMenu(e: { part: 'line' | 'shopLine' | 'ref'; index: number }) {
		const p = livePane;
		if (!p) return;
		const base = { columns: undefined, number: 0 };
		if (e.part === 'ref') {
			const r = p.references[e.index];
			const l = pick(LOC_OF);
			if (!r || !l) return;
			ctx = {
				...base,
				list: l,
				number: r.target?.table === 'items' ? r.target.number : 0,
				columns: ['Where', ...r.cells.map(() => 'What')],
				cells: [r.text, ...r.cells.map((c) => c.text)]
			};
		} else if (e.part === 'line') {
			const ln = p.lines?.[e.index];
			if (!ln) return;
			ctx = {
				...base,
				list: list ? 'monsterCompareLoc' : 'monsterDetail',
				number: Number(selectedId),
				cells: [ln.label, ...ln.cells.map((c) => c.text)],
				lines: p.lines?.map((x) => ({ text: x.label, subs: x.cells.map((c) => c.text) })),
				line: ln.target ? { kind: ln.target.table === 'rooms' ? 'room' : ln.target.table.replace(/s$/, ''), number: ln.target.number, tag: ln.cells[0]?.tag } : undefined
			};
		} else {
			const stock = (selected as Shop | undefined)?.stock?.filter((x) => x.item !== 0) ?? [];
			const row = p.rows.filter((x) => x.kind === 'shop_line')[e.index];
			const slot = stock[e.index];
			if (!row) return;
			ctx = {
				...base,
				list: 'shopDetail',
				number: slot?.item ?? 0,
				columns: ['#', 'Name', 'Max', 'Regen', 'Cost'],
				cells: [String(slot?.item ?? ''), row.label, String(slot?.max ?? ''), '', row.value]
			};
		}
	}

	const menuItems = $derived(ctx ? buildMenu({ list: ctx.list, lairStats: ctx.lairStats, autoCalcDamage: true }) : []);

	async function runMenu(id: ActionId) {
		const t = ctx;
		if (!t) return undefined;
		const p = livePane;
		// the pane belongs to the selected record: use it when it is the row's
		const mine = p && t.number === p.number && t.list === (pick(LIST_OF) ?? t.list);
		const full: MenuTarget = mine
			? {
					...t,
					detail: t.detail ?? p.text,
					references: p.references.map((r) => ({ text: r.text, subs: r.cells.map((c) => c.text) })),
					lines: t.lines ?? p.lines?.map((x) => ({ text: x.label, subs: x.cells.map((c) => c.text) }))
				}
			: t;
		return actions.run(id, full);
	}
	const vsNote = $derived(
		slug === 'weapons' ? vsMob.weapons : slug === 'spells' ? vsMob.spells : undefined
	);

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
	// The Items tab's chest button (cmdSundryChests): enabled for a container; the contents list opens as a dialog.
	const isChest = $derived(slug === 'items' && !list && selected !== undefined && (selected as Item).item_type === 8);
	async function chests() {
		const n = Number(selectedId);
		const out = await api.itemChest(n);
		if (out.message) chestNote = out.message;
		else if (out.rows.length === 0) chestNote = 'Nothing.';
		else {
			chestNote = '';
			dialogs.showResults({
				title: `Chest contents: ${(selected as Item).name} (${n})`,
				columns: ['Chance', 'Item'],
				rows: chestRows(out)
			});
		}
	}
	let chestNote = $state('');
	$effect(() => {
		void selectedId;
		chestNote = '';
	});
	const enabledBoxes = $derived(spellOptionsEnabled(spellBar.calcCombat));
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
		{#if list}
			<button
				type="button"
				class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm disabled:opacity-50"
				disabled={selectedId === undefined}
				data-testid="compare-remove"
				onclick={() => {
					const at = list.numbers.indexOf(Number(selectedId));
					if (at >= 0) list.onRemove(at);
				}}
			>
				Remove
			</button>
			<button
				type="button"
				class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm"
				data-testid="compare-clear"
				onclick={() => {
					// cmdCompareClear_Click asks "Are you sure?" with No as the default
					if (confirm(`Clear the saved ${title.toLowerCase()} list?`)) list.onClear();
				}}
			>
				Clear
			</button>
		{:else if compareKind}
			{@const on = selectedId !== undefined && cmp.has(compareKind, Number(selectedId))}
			<button
				type="button"
				class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm disabled:opacity-50"
				disabled={selectedId === undefined || !(Number(selectedId) > 0)}
				aria-pressed={on}
				data-testid="compare-add"
				onclick={() => void actions.addToCompare(compareKind, Number(selectedId))}
			>
				{on ? 'Remove from compare' : 'Add to compare'}
			</button>
		{/if}
		{#if isChest}
			<button
				type="button"
				class="bg-background border-input hover:bg-accent h-8 rounded-md border px-3 text-sm"
				data-testid="chests"
				onclick={chests}
			>
				Chests
			</button>
			{#if chestNote}<span class="text-muted-foreground text-sm" role="status" data-testid="chest-note">{chestNote}</span>{/if}
		{/if}
		{#if slug === 'spells' && !list}
			<label class="flex items-center gap-1 text-sm" title="Calculate the combat numbers of each spell">
				<input type="checkbox" bind:checked={spellBar.calcCombat} data-testid="spell-calc-combat" />
				Calc. Combat
			</label>
			<label class="flex items-center gap-1 text-sm {enabledBoxes.vsMr ? '' : 'opacity-50'}">
				vs MR
				<input
					class="bg-background border-input h-7 w-14 rounded-md border px-2 disabled:opacity-60"
					bind:value={spellBar.vsMr}
					disabled={!enabledBoxes.vsMr}
					inputmode="numeric"
					data-testid="spell-vs-mr"
				/>
			</label>
			<label class="flex items-center gap-1 text-sm {enabledBoxes.antiMagic ? '' : 'opacity-50'}">
				<input type="checkbox" bind:checked={spellBar.antiMagic} disabled={!enabledBoxes.antiMagic} data-testid="spell-anti-magic" />
				vs Anti-Magic
			</label>
		{/if}
		{#if slug === 'monsters' && !list}
			<MonsterStatus />
		{/if}
		{#if explorer.pending.length}
			<span class="text-muted-foreground text-xs" data-testid="pending">
				Needs a character, coming later: {explorer.pending.join(', ')}
			</span>
		{/if}
	</div>

	{#if vsNote}
		<div class="bg-card flex items-center gap-2 rounded-md border px-3 py-1 text-sm" data-testid="vs-mob">
			<span>
				Calculated against {vsNote.name}{slug === 'weapons' ? ` (AC ${vsNote.ac}, DR ${vsNote.dr}, dodge ${vsNote.dodge})` : ` (MR ${vsNote.mr})`}
			</span>
			<button
				type="button"
				class="border-input hover:bg-accent h-6 rounded-md border px-2 text-xs"
				data-testid="vs-mob-clear"
				onclick={() => (slug === 'weapons' ? vsMob.setWeapons(undefined) : vsMob.setSpells(undefined))}>Clear</button
			>
		</div>
	{/if}

	{#if slug === 'monsters' && !list}
		<MonsterFilters />
	{/if}
	{#if slug === 'spells' && !list}
		<SpellFilters />
	{/if}

	<div class="relative min-h-0 flex-1">
		{#if question}
			<DamageQuestion prompt={question} onanswer={answer} />
		{/if}
		{#if error}
			<p class="text-destructive" role="alert">Could not load {title.toLowerCase()}: {error}</p>
		{:else if evalError && !payload}
			<p class="text-destructive" role="alert">Could not build {title.toLowerCase()}: {evalError}</p>
		{:else if !table || (computedTab && !payload)}
			<p class="text-muted-foreground" data-testid="loading">Loading {title.toLowerCase()}...</p>
		{:else}
			<RowContextMenu items={menuItems} onAction={runMenu} onBefore={() => (ctx = undefined)}>
			<SplitPane storageKey="touchstone.split.{slug}" autoWidth={gridWidth} bind:collapsed={detailHidden}>
				{#snippet left()}
					{#if computedTab}
						<DataGrid
							rows={evalRows}
							columnDefs={evalDefs}
							getRowId={(r: KeyedRow) => r.key ?? String(r.number)}
							selectedId={selectedId === undefined ? undefined : String(selectedId)}
							quickFilter={search}
							storageKey={list ? `compare-${slug}` : slug}
							onSelect={(r: EvalRow) => selection.set(selKey, r.number)}
							onRowContext={(r: EvalRow) => rowMenu(r)}
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
						onSelect={(r) => selection.set(selKey, explorer.rowId(r))}
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
							onMenu={paneMenu}
						>
							{#snippet before()}
								{#if isShops}<ShopControls note={shopNote} />{/if}
							{/snippet}
							{#if slug === 'spells'}
								{#key selectedId}
									<SpellEffect number={Number(explorer.rowId(selected))} reqLevel={(selected as Spell).req_level} />
								{/key}
							{:else if slug === 'races' || slug === 'classes'}
								<ClassRaceBuilder kind={slug === 'races' ? 'race' : 'class'} number={Number(explorer.rowId(selected))} />
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
			</RowContextMenu>
		{/if}
	</div>
</div>

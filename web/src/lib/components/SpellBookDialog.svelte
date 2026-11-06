<script lang="ts">
	// MME's Spell Book window (frmSpellBook): the spells a class and level may learn or cast, with the
	// global filter's character. The server decides the list (`cmdListSpells_Click`) and builds the rows
	// (`AddSpell2LV`); this shows them. Double-click a spell to open it in the Spells tab.
	import { goto } from '$app/navigation';
	import { untrack } from 'svelte';
	import { api, type ClassRec } from '$lib/api';
	import { buildMenu, type ActionId } from '$lib/contextmenu';
	import { evaluateBody, evaluateClient, toColDefs, type EvalRow, type EvalTable } from '$lib/evaluate';
	import { getCharacterStore } from '$lib/character.svelte';
	import { getGlobalFilterStore } from '$lib/globalfilter.svelte';
	import { selection } from '$lib/explorer-state.svelte';
	import { createMenuActions, type MenuTarget } from '$lib/menuactions.svelte';
	import { spellBook } from '$lib/spellbook.svelte';
	import { ALIGNMENTS, MAGERY_LEVELS, MAGERY_NAMES, bookRequest, barIsOn, type BookForm } from '$lib/spelltools';
	import RowContextMenu from '$lib/components/RowContextMenu.svelte';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import { Button } from '$lib/components/ui/button';

	const store = getCharacterStore();
	const bar = getGlobalFilterStore();
	const actions = createMenuActions();

	let classes = $state.raw<ClassRec[]>([]);
	let form = $state<BookForm>({ class: 0, level: '', alignment: 0, magery: 0, magery_level: 0 });
	let table = $state.raw<EvalTable>();
	let sortAscending = $state(true);
	let error = $state<string>();
	let busy = $state(false);
	let ctx = $state.raw<MenuTarget>();
	const select = 'bg-background border-input h-8 rounded-md border px-2 text-sm';

	$effect(() => {
		if (!spellBook.open || classes.length) return;
		api.table('classes').then((t) => (classes = t.filter((c) => c.name !== '')));
	});

	// The builder's Spellbook button (and anything else) opens the window with a form to fill.
	let seenSeed = 0;
	$effect(() => {
		const seed = spellBook.seed;
		if (!spellBook.open || !seed || seed.id === seenSeed) return;
		seenSeed = seed.id;
		untrack(() => {
			form = { ...form, ...seed.form };
			void list();
		});
	});

	/** `cmbClass_Click`: the class sets the magery boxes. */
	async function classChanged() {
		try {
			const out = await api.spellBookClass(form.class);
			if (out.magery_level !== null) form.magery_level = out.magery_level;
			if (out.magery !== null) form.magery = out.magery;
		} catch {
			// a class that is not there: MME says so; the boxes stay
		}
	}

	/** `CopyGlobalChar`: the global filter bar fills the window. */
	async function copyGlobal() {
		const f = bar.filter;
		const out = await api.spellBookCopy({
			filter_on: barIsOn(f),
			alignment: f.alignment,
			level_text: String(f.level),
			class_index: f.class > 0 ? 1 : 0,
			class: f.class
		});
		if (out.alignment !== null) form.alignment = out.alignment;
		if (out.level !== null) form.level = out.level;
		if (out.class !== null) {
			form.class = out.class;
			await classChanged();
		}
	}

	function characterMagery() {
		const c = classes.find((x) => x.number === bar.filter.class);
		return { type: c?.magery_type ?? 0, level: c?.magery_lvl ?? 0 };
	}

	/** `cmdListSpells_Click`. */
	async function list() {
		busy = true;
		error = undefined;
		try {
			const req = bookRequest($state.snapshot(form), $state.snapshot(bar.filter), characterMagery());
			const out = await api.spellBookList(req);
			sortAscending = out.sort_ascending;
			if (out.spells.length === 0) {
				table = { columns: [], rows: [] };
				return;
			}
			// the loaded character builds the rows only when the window matches it
			const doc = out.uses_character ? $state.snapshot(store.doc) : undefined;
			table = await evaluateClient.evaluate(
				'spells',
				evaluateBody(doc, $state.snapshot(bar.filter), { numbers: out.spells })
			);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	// MME sorts by the level column (5th), highest first while the global filter is on
	const defs = $derived(
		table ? toColDefs(table.columns, { col: table.columns[4]?.id ?? '', dir: sortAscending ? 'asc' : 'desc' }) : []
	);

	function rowMenu(r: EvalRow) {
		ctx = {
			list: 'spellBook',
			number: r.number,
			columns: table?.columns.map((c) => c.header),
			cells: r.cells.map((c) => c.text)
		};
	}
	const menuItems = $derived(ctx ? buildMenu({ list: 'spellBook' }) : []);
	async function runMenu(id: ActionId) {
		return ctx ? actions.run(id, ctx) : undefined;
	}

	function open(r: EvalRow) {
		selection.set('spells', r.number);
		spellBook.open = false;
		void goto('/spells');
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && spellBook.open && (spellBook.open = false)} />

{#if spellBook.open}
	<div class="bg-background/80 fixed inset-0 z-50 flex items-center justify-center p-4" role="presentation">
		<div
			class="bg-card flex h-[min(720px,90vh)] w-[min(1100px,96vw)] flex-col gap-2 rounded-lg border p-3 shadow-xl"
			role="dialog"
			aria-modal="true"
			aria-label="Spell Book"
			data-testid="spell-book"
		>
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="mr-2 text-base font-semibold">Spell Book</h2>
				<label class="flex items-center gap-1 text-sm">
					Class
					<select class={select} bind:value={form.class} onchange={classChanged} data-testid="book-class">
						<option value={0}>none</option>
						{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
					</select>
				</label>
				<label class="flex items-center gap-1 text-sm">
					Level
					<input class="{select} w-16" bind:value={form.level} inputmode="numeric" data-testid="book-level" />
				</label>
				<label class="flex items-center gap-1 text-sm">
					Alignment
					<select class={select} bind:value={form.alignment} data-testid="book-alignment">
						{#each ALIGNMENTS as a, i (a)}<option value={i}>{a}</option>{/each}
					</select>
				</label>
				<label class="flex items-center gap-1 text-sm">
					Magery
					<select class={select} bind:value={form.magery} data-testid="book-magery">
						{#each MAGERY_NAMES as m, i (m)}<option value={i}>{m}</option>{/each}
					</select>
				</label>
				<label class="flex items-center gap-1 text-sm">
					Magery level
					<select class={select} bind:value={form.magery_level} data-testid="book-magery-level">
						{#each MAGERY_LEVELS as m, i (m)}<option value={i}>{m}</option>{/each}
					</select>
				</label>
				<Button size="sm" variant="outline" onclick={copyGlobal} data-testid="book-copy-filter">Use filter bar</Button>
				<Button size="sm" onclick={list} disabled={busy} data-testid="book-list">List Spells</Button>
				<span class="flex-1"></span>
				<Button size="sm" variant="outline" onclick={() => (spellBook.open = false)} data-testid="book-close">Close</Button>
			</div>
			{#if error}<p class="text-destructive text-sm" role="alert">{error}</p>{/if}
			<div class="min-h-0 flex-1">
				{#if table && table.rows.length}
					<RowContextMenu items={menuItems} onAction={runMenu} onBefore={() => (ctx = undefined)}>
						<DataGrid
							rows={table.rows}
							columnDefs={defs}
							getRowId={(r: EvalRow) => String(r.number)}
							storageKey="spell-book"
							onRowContext={(r: EvalRow) => rowMenu(r)}
							onRowDoubleClick={open}
						/>
					</RowContextMenu>
				{:else}
					<p class="text-muted-foreground text-sm" data-testid="book-empty">
						{table ? 'No spells for this class and level.' : 'Pick a class and level, then List Spells.'}
					</p>
				{/if}
			</div>
			<p class="text-muted-foreground text-xs" data-testid="book-count">
				{table ? `${table.rows.length.toLocaleString('en-US')} spells` : ''}
			</p>
		</div>
	</div>
{/if}

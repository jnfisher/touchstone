<script lang="ts">
	// The Char tab (F10): the saved-character bar on top, then the panels.
	import { api, type ClassRec, type Item, type Race } from '$lib/api';
	import { createCharacterStore } from '$lib/character.svelte';
	import { cardCls } from '../calc/styles';
	import BlessPanel from './BlessPanel.svelte';
	import DerivedPanel from './DerivedPanel.svelte';
	import EquipPanel from './EquipPanel.svelte';
	import IdentityPanel from './IdentityPanel.svelte';
	import QuestPanel from './QuestPanel.svelte';
	import StatsPanel from './StatsPanel.svelte';
	import VitalsPanel from './VitalsPanel.svelte';

	const store = createCharacterStore();
	let classes = $state.raw<ClassRec[]>([]);
	let races = $state.raw<Race[]>([]);
	let items = $state.raw<Item[]>([]);
	let loadError = $state<string>();
	let picked = $state('');

	$effect(() => {
		let alive = true;
		Promise.all([api.table('classes'), api.table('races'), api.table('items')])
			.then(([c, r, i]) => {
				if (!alive) return;
				classes = c.filter((x) => x.name !== '');
				races = r.filter((x) => x.name !== '');
				items = i;
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		void openLast();
		// Unsent edits are written when the tab closes.
		return () => {
			alive = false;
			void store.flush();
		};
	});

	const LAST_KEY = 'touchstone.character.last';
	const remember = (name: string | undefined) => {
		try {
			if (name) localStorage.setItem(LAST_KEY, name);
		} catch {
			// private window or blocked storage: the tab still works, it just starts empty
		}
	};
	async function openLast() {
		await store.refreshList();
		let last: string | null = null;
		try {
			last = localStorage.getItem(LAST_KEY);
		} catch {
			// see remember()
		}
		const names = store.saved.map((s) => s.name);
		const name = last && names.includes(last) ? last : names[0];
		if (name && store.doc.name === '' && !store.storedName) {
			picked = name;
			await store.load(name);
		}
	}
	$effect(() => remember(store.storedName));

	const statusText = $derived(
		store.nameProblem && store.doc.name !== ''
			? `Not saved: ${store.nameProblem}`
			: store.doc.name === ''
				? 'Name the character to save it'
				: { idle: 'Saved characters autosave', saving: 'Saving...', saved: 'Saved', error: 'Save failed' }[store.saveStatus]
	);
</script>

<div class="h-full overflow-auto" data-testid="character-tab">
	<div class="mb-3 flex flex-wrap items-center gap-2">
		<h2 class="mr-2 text-base font-semibold">Character</h2>
		<select
			bind:value={picked}
			onchange={() => {
				if (picked) void store.load(picked);
			}}
			aria-label="Saved characters"
			data-testid="char-picker"
			class="bg-background border-input h-8 rounded-md border px-2 text-sm"
		>
			<option value="">Saved characters ({store.saved.length})</option>
			{#each store.saved as s (s.name)}<option value={s.name}>{s.name} (L{s.level})</option>{/each}
		</select>
		<button
			type="button"
			class="bg-secondary text-secondary-foreground h-8 rounded-md px-3 text-sm"
			onclick={() => {
				picked = '';
				store.startNew();
			}}
			data-testid="char-new">New</button
		>
		<button
			type="button"
			class="border-input h-8 rounded-md border px-3 text-sm disabled:opacity-50"
			disabled={!store.storedName}
			onclick={() => {
				if (confirm(`Delete ${store.storedName}?`)) {
					picked = '';
					void store.remove();
				}
			}}
			data-testid="char-delete">Delete</button
		>
		<span class="text-muted-foreground ml-auto text-xs" data-testid="char-status">{statusText}</span>
	</div>

	{#if loadError}<p class="text-destructive mb-2 text-sm">{loadError}</p>{/if}
	{#if store.saveError}<p class="text-destructive mb-2 text-sm" data-testid="char-save-error">{store.saveError}</p>{/if}
	{#if store.evalError}<p class="text-destructive mb-2 text-sm" data-testid="char-eval-error">{store.evalError}</p>{/if}

	<div class="grid items-start gap-4 lg:grid-cols-2 2xl:grid-cols-3">
		<section class={cardCls} aria-label="Identity and stats">
			<IdentityPanel {store} {classes} {races} />
			<div class="mt-4"><StatsPanel {store} /></div>
		</section>
		<section class={cardCls} aria-label="Vitals"><VitalsPanel {store} /></section>
		<section class="{cardCls} flex flex-col gap-5" aria-label="Bless and quests">
			<BlessPanel {store} />
			<QuestPanel {store} />
		</section>
	</div>
	<section class="{cardCls} mt-4" aria-label="Equipment"><EquipPanel {store} {items} /></section>
	<section class="{cardCls} mt-4" aria-label="Derived stats"><DerivedPanel {store} /></section>
</div>

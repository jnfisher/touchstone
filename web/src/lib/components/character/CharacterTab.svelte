<script lang="ts">
	// The Char tab (F10): the saved-character bar on top, then the panels.
	import { api, type ClassRec, type Item, type Race } from '$lib/api';
	import { exportUrl, isMmecName } from '$lib/character';
	import { getCharacterStore, openLastCharacter, rememberLastCharacter } from '$lib/character.svelte';
	import { cardCls } from '../calc/styles';
	import PasteDialog from '../paste/PasteDialog.svelte';
	import CombatDialog from './CombatDialog.svelte';
	import BlessPanel from './BlessPanel.svelte';
	import DerivedPanel from './DerivedPanel.svelte';
	import EquipPanel from './EquipPanel.svelte';
	import IdentityPanel from './IdentityPanel.svelte';
	import QuestPanel from './QuestPanel.svelte';
	import StatsPanel from './StatsPanel.svelte';
	import VitalsPanel from './VitalsPanel.svelte';

	const store = getCharacterStore();
	let classes = $state.raw<ClassRec[]>([]);
	let races = $state.raw<Race[]>([]);
	let items = $state.raw<Item[]>([]);
	let loadError = $state<string>();
	let picked = $state(store.storedName ?? '');

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

	async function openLast() {
		const name = await openLastCharacter(store);
		if (name) picked = name;
	}
	$effect(() => rememberLastCharacter(store.storedName));

	let pasting = $state(false);
	let combat = $state(false);
	let fileInput = $state<HTMLInputElement>();
	let importNote = $state<string>();

	async function importFile(file: File | undefined) {
		importNote = undefined;
		if (!file) return;
		if (!isMmecName(file.name)) {
			importNote = 'Pick an MMUD Explorer character file (.mmec).';
			return;
		}
		let result = await store.importMmec(file);
		if (result === 'exists' && confirm('A saved character with that name exists. Replace it?')) {
			result = await store.importMmec(file, true);
		}
		if (result === 'ok') picked = store.storedName ?? '';
		else if (result === 'exists') importNote = 'Not imported: that name is taken.';
	}

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
		<input
			bind:this={fileInput}
			type="file"
			accept=".mmec"
			class="hidden"
			data-testid="char-import-input"
			onchange={(e) => {
				void importFile(e.currentTarget.files?.[0]);
				e.currentTarget.value = '';
			}}
		/>
		<button
			type="button"
			class="border-input h-8 rounded-md border px-3 text-sm"
			onclick={() => fileInput?.click()}
			data-testid="char-import">Import .mmec</button
		>
		{#if store.storedName}
			<a
				href={exportUrl(store.storedName)}
				download
				class="border-input inline-flex h-8 items-center rounded-md border px-3 text-sm"
				data-testid="char-export">Export .mmec</a
			>
		{:else}
			<span
				class="border-input text-muted-foreground inline-flex h-8 items-center rounded-md border px-3 text-sm opacity-50"
				title="Save the character first"
				data-testid="char-export-disabled">Export .mmec</span
			>
		{/if}
		<button
			type="button"
			class="border-input h-8 rounded-md border px-3 text-sm"
			onclick={() => (pasting = true)}
			data-testid="char-paste">Paste character</button
		>
		<button
			type="button"
			class="border-input h-8 rounded-md border px-3 text-sm disabled:opacity-50"
			disabled={store.doc.class === null || store.doc.race === null}
			title="Choose the attack, backstab and healing"
			onclick={() => (combat = true)}
			data-testid="char-combat"
			>Combat{store.evaluated?.attack_name ? `: ${store.evaluated.attack_name}` : ''}</button
		>
		<span class="text-muted-foreground ml-auto text-xs" data-testid="char-status">{statusText}</span>
	</div>

	{#if importNote}<p class="text-destructive mb-2 text-sm" data-testid="char-import-note">{importNote}</p>{/if}
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

{#if pasting}
	<PasteDialog {store} onclose={() => (pasting = false)} />
{/if}

{#if combat}
	<CombatDialog {store} onclose={() => (combat = false)} />
{/if}

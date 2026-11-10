<script lang="ts">
	// The header's active-character picker: a searchable list of the realm's characters plus the Sandbox. What
	// the rest of the app calls "the character" is whichever is picked here.
	import { getActiveCharacter } from '$lib/character.svelte';
	import { getRealmStore } from '$lib/realm.svelte';
	import { fullName, pickerLabel, searchPicker } from '$lib/realm';

	const active = getActiveCharacter();
	const realm = getRealmStore();

	let open = $state(false);
	let text = $state('');
	let box = $state<HTMLDivElement>();
	let input = $state<HTMLInputElement>();

	const matches = $derived(searchPicker(realm.characters, text));
	const current = $derived(active.choice.kind === 'realm' ? realm.characters.find((c) => c.id === (active.choice as { id: number }).id) : undefined);
	const sandboxName = $derived(active.store.doc.name);
	const sandboxLabel = $derived('Sandbox');
	const sandboxShown = $derived(text.trim() === '' || 'sandbox'.includes(text.trim().toLowerCase()));

	function pick(c: { kind: 'sandbox' } | { kind: 'realm'; id: number }) {
		active.select(c);
		open = false;
		text = '';
	}
	function onWindowClick(e: MouseEvent) {
		if (open && box && !box.contains(e.target as Node)) open = false;
	}
	$effect(() => {
		if (open) input?.focus();
	});
</script>

<svelte:window onclick={onWindowClick} />

<div class="relative" bind:this={box} data-testid="active-picker">
	<button
		type="button"
		class="bg-background border-input hover:bg-accent flex h-8 max-w-80 items-center gap-2 rounded-md border px-3 text-sm"
		aria-haspopup="listbox"
		aria-expanded={open}
		data-testid="active-picker-button"
		onclick={() => (open = !open)}
	>
		<span class="text-muted-foreground text-xs">Active</span>
		<span class="truncate font-medium" data-testid="active-label">
			{#if current}{fullName(current)} <span class="text-muted-foreground font-normal">{current.race_name} {current.class_name} L{current.level}</span>{:else if active.choice.kind === 'realm'}Loading...{:else}{sandboxLabel}{sandboxName ? `: ${sandboxName}` : ''}{/if}
		</span>
		<span aria-hidden="true" class="text-muted-foreground">&#9662;</span>
	</button>
	{#if open}
		<div class="bg-popover text-popover-foreground absolute top-full left-0 z-50 mt-1 w-96 max-w-[90vw] rounded-md border p-2 shadow-md" data-testid="active-picker-panel">
			<input
				bind:this={input}
				bind:value={text}
				type="search"
				placeholder="Search characters..."
				aria-label="Search characters"
				data-testid="active-picker-search"
				class="bg-background border-input mb-2 h-8 w-full rounded-md border px-2 text-sm outline-none"
				onkeydown={(e) => {
					if (e.key === 'Escape') open = false;
					if (e.key === 'Enter') {
						if (sandboxShown && text.trim() !== '' && !matches.length) pick({ kind: 'sandbox' });
						else if (matches[0]) pick({ kind: 'realm', id: matches[0].id });
						else if (sandboxShown) pick({ kind: 'sandbox' });
					}
				}}
			/>
			<ul role="listbox" class="max-h-72 overflow-auto text-sm">
				{#if sandboxShown}
					<li role="option" aria-selected={active.choice.kind === 'sandbox'}>
						<button type="button" class="hover:bg-accent flex w-full items-center justify-between rounded px-2 py-1 text-left {active.choice.kind === 'sandbox' ? 'bg-accent' : ''}" data-testid="pick-sandbox" onclick={() => pick({ kind: 'sandbox' })}>
							<span>Sandbox{sandboxName ? `: ${sandboxName}` : ''}</span><span class="text-muted-foreground text-xs">what-if editor</span>
						</button>
					</li>
				{/if}
				{#each matches as c (c.id)}
					<li role="option" aria-selected={active.realmId === c.id}>
						<button type="button" class="hover:bg-accent flex w-full items-center justify-between gap-2 rounded px-2 py-1 text-left {active.realmId === c.id ? 'bg-accent' : ''}" data-testid="pick-{c.id}" onclick={() => pick({ kind: 'realm', id: c.id })}>
							<span class="truncate">{fullName(c)}</span><span class="text-muted-foreground shrink-0 text-xs">{c.race_name} {c.class_name} L{c.level}</span>
						</button>
					</li>
				{/each}
				{#if !matches.length && !sandboxShown}<li class="text-muted-foreground px-2 py-1">No match.</li>{/if}
			</ul>
			{#if !realm.available}<p class="text-muted-foreground mt-2 text-xs">This data source has no realm characters.</p>{/if}
		</div>
	{/if}
</div>

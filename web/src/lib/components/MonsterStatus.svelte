<script lang="ts">
	// What the Options menu has done for the Monsters tab: the damage calculation's progress and result, and
	// whether reachability is computed. The actions themselves are in the header's Options menu.
	import { getCharacterStore } from '$lib/character.svelte';
	import { getDamageStore } from '$lib/monsterdamage.svelte';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { getOptionsStore } from '$lib/options.svelte';
	import { canCalculate, progressText } from '$lib/monsterdamage';
	import { onMount } from 'svelte';

	const damage = getDamageStore();
	const character = getCharacterStore();
	const boxes = getMonsterFilterStore();
	const opts = getOptionsStore();

	const running = $derived(damage.status.running);
	const label = $derived(
		damage.busy === 'clear'
			? 'Clearing calculated monster damage...'
			: damage.busy
				? running
					? progressText(running)
					: 'Calculating monster damage...'
				: opts.reachBusy
					? 'Calculating reachability...'
					: undefined
	);
	const state = $derived(
		[damage.status.char.calculated ? 'character' : '', damage.status.party.calculated ? 'party' : '']
			.filter(Boolean)
			.join(' and ')
	);

	onMount(() => void damage.refresh());
	$effect(() => {
		void boxes.reachRev;
		void opts.refreshReach();
	});
</script>

<div class="flex flex-wrap items-center gap-2" data-testid="monster-status">
	{#if label}
		<span class="text-sm tabular-nums" role="status" aria-live="polite" data-testid="damage-progress">{label}</span>
	{:else if damage.error}
		<span class="text-destructive text-sm" role="alert" data-testid="damage-error">{damage.error}</span>
	{/if}
	{#if state}
		<span class="bg-muted text-muted-foreground rounded-md px-2 py-0.5 text-xs" data-testid="damage-state">Damage calculated vs {state}</span>
	{:else if !label && !damage.error && !canCalculate(character.doc)}
		<span class="text-muted-foreground text-xs">Pick a class and race to calculate monster damage (Options menu)</span>
	{/if}
	{#if opts.reach?.computed}
		<span class="bg-muted text-muted-foreground rounded-md px-2 py-0.5 text-xs" data-testid="reach-state">Reachability computed</span>
	{/if}
</div>

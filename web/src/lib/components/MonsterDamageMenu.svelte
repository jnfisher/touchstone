<script lang="ts">
	// MME's Options menu entries for the monster damage: Calculate vs Character, Calculate vs Party and Clear. The
	// server runs the simulations (a few seconds to a minute); this shows the progress and refreshes the list and pane.
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { getCharacterStore } from '$lib/character.svelte';
	import { getDamageStore } from '$lib/monsterdamage.svelte';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { canCalculate, progressText } from '$lib/monsterdamage';
	import { onMount } from 'svelte';

	const damage = getDamageStore();
	const character = getCharacterStore();
	const boxes = getMonsterFilterStore();

	const ready = $derived(canCalculate(character.doc));
	const running = $derived(damage.status.running);
	const label = $derived(
		damage.busy === 'clear'
			? 'Clearing calculated monster damage...'
			: damage.busy
				? running
					? progressText(running)
					: 'Calculating monster damage...'
				: undefined
	);
	const state = $derived(
		[damage.status.char.calculated ? 'character' : '', damage.status.party.calculated ? 'party' : '']
			.filter(Boolean)
			.join(' and ')
	);

	onMount(() => void damage.refresh());
</script>

<div class="flex flex-wrap items-center gap-2" data-testid="damage-menu">
	<DropdownMenu.Root>
		<DropdownMenu.Trigger data-testid="options-menu" disabled={!!damage.busy}>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm" disabled={!!damage.busy}>Options</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="start">
			<DropdownMenu.Item
				data-testid="damage-calc-char"
				disabled={!ready}
				onSelect={() => void damage.calculate(character.doc, 'char', boxes.controls.party)}
			>
				Calculate Monster Damage vs Character
			</DropdownMenu.Item>
			<DropdownMenu.Item
				data-testid="damage-calc-party"
				disabled={!ready}
				onSelect={() => void damage.calculate(character.doc, 'party', boxes.controls.party)}
			>
				Calculate Monster Damage vs Party
			</DropdownMenu.Item>
			<DropdownMenu.Separator />
			<DropdownMenu.Item data-testid="damage-clear" onSelect={() => void damage.clear()}>
				Clear Calculated Monster Damage
			</DropdownMenu.Item>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
	{#if label}
		<span class="text-sm tabular-nums" role="status" aria-live="polite" data-testid="damage-progress">{label}</span>
	{:else if damage.error}
		<span class="text-destructive text-sm" role="alert" data-testid="damage-error">{damage.error}</span>
	{:else if state}
		<span class="text-muted-foreground text-xs" data-testid="damage-state">Damage calculated vs {state}</span>
	{:else if !ready}
		<span class="text-muted-foreground text-xs">Pick a class and race to calculate monster damage</span>
	{/if}
</div>

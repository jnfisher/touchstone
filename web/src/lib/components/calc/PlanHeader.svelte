<script lang="ts">
	// The heading of a planning tool (Weapon Simulator, Gear Planner): its title, which character it is working
	// for, and the "use another" dropdown that borrows a realm character for this tool only.
	import { goto } from '$app/navigation';
	import CharacterDropdown from './CharacterDropdown.svelte';
	import type { CurrentCharacter } from './current.svelte';

	let {
		title,
		current,
		ready,
		testid
	}: {
		title: string;
		current: CurrentCharacter;
		/** The character has a class and a race, so the tool can run. */
		ready: boolean;
		/** Prefix of the test ids (`wsim`, `gear`). */
		testid: string;
	} = $props();
	const src = $derived(current.source);
</script>

<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
	<h2 class="text-base font-semibold">{title}</h2>
	<span class="text-sm" data-testid="{testid}-who">
		{#if ready}
			<span class="font-medium">{src.name}</span><span class="text-muted-foreground">{src.kind ? `, ${src.kind}` : ''}, level {src.level}{src.via === 'override' ? ' (chosen here)' : ''}</span>
			<button type="button" class="text-primary ml-1 underline" onclick={() => current.openSheet((p) => void goto(p))}>Open sheet</button>
		{:else if current.override !== undefined}
			<span class="text-muted-foreground">Loading {src.name}...</span>
		{/if}
	</span>
	{#if current.realmAvailable}
		<CharacterDropdown
			characters={current.choices}
			selected={current.override === undefined ? [] : [current.override]}
			label="Use another character"
			testid="{testid}-use-another"
			onpick={([id]) => void current.useAnother(id)}
		/>
		{#if current.override !== undefined}
			<button type="button" class="border-input hover:bg-accent h-7 rounded-md border px-2 text-xs" onclick={() => current.clearOverride()}>
				Back to the active character
			</button>
		{/if}
		{#if current.overrideError}<span class="text-destructive text-xs" role="alert">{current.overrideError}</span>{/if}
	{/if}
</div>
